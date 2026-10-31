//! Shared `RenderingServer` plumbing for the bench node ([`crate::node`])
//! and the match node ([`crate::match_node`]): the batched MultiMesh
//! [`Layer`], `ArrayMesh` building from [`MeshData`] and the skinned soldier
//! material.

use godot::classes::image::Format as ImageFormat;
use godot::classes::mesh::{ArrayCustomFormat, ArrayFormat, ArrayType, PrimitiveType};
use godot::classes::rendering_server::{MultimeshTransformFormat, ShadowCastingSetting};
use godot::classes::{
    ArrayMesh, Image, ImageTexture, Material, RenderingServer, Shader, ShaderMaterial,
};
use godot::obj::{EngineBitfield, EngineEnum};
use godot::prelude::*;

use crate::meshgen::MeshData;
use crate::present::{Bounds, STRIDE, next_capacity};
use crate::rig;

// ---------------------------------------------------------------- layers --

/// One `RenderingServer` MultiMesh + instance. Instances are written into a
/// Rust scratch buffer during the frame; `commit` sizes the MultiMesh to
/// what was written (with hysteresis) and uploads it in one call.
pub(crate) struct Layer {
    pub(crate) mm: Rid,
    pub(crate) inst: Rid,
    /// Upload buffer, `cap * STRIDE` floats (Godot requires the full size).
    buf: PackedFloat32Array,
    scratch: Vec<f32>,
    pub(crate) cap: usize,
    min_cap: usize,
    max_cap: usize,
    pub(crate) count: usize,
    bounds: Bounds,
    pub(crate) reallocs: u32,
}

impl Layer {
    pub(crate) fn new(
        rs: &mut Gd<RenderingServer>,
        mesh: Rid,
        scenario: Rid,
        min_cap: usize,
        max_cap: usize,
        shadows: ShadowCastingSetting,
        aabb: Aabb,
    ) -> Layer {
        let max_cap = max_cap.max(1);
        let min_cap = min_cap.clamp(1, max_cap);
        let mm = rs.multimesh_create();
        Self::allocate(rs, mm, min_cap);
        rs.multimesh_set_mesh(mm, mesh);
        rs.multimesh_set_custom_aabb(mm, aabb);
        rs.multimesh_set_visible_instances(mm, 0);
        let inst = rs.instance_create2(mm, scenario);
        rs.instance_geometry_set_cast_shadows_setting(inst, shadows);
        let mut buf = PackedFloat32Array::new();
        buf.resize(min_cap * STRIDE);
        Layer {
            mm,
            inst,
            buf,
            scratch: vec![0.0; max_cap * STRIDE],
            cap: min_cap,
            min_cap,
            max_cap,
            count: 0,
            bounds: Bounds::default(),
            reallocs: 0,
        }
    }

    fn allocate(rs: &mut Gd<RenderingServer>, mm: Rid, cap: usize) {
        rs.multimesh_allocate_data_ex(mm, cap as i32, MultimeshTransformFormat::TRANSFORM_3D)
            .color_format(false)
            .custom_data_format(true)
            .done();
    }

    pub(crate) fn begin(&mut self) {
        self.count = 0;
        self.bounds = Bounds::default();
    }

    /// Reserves the next instance (bounding sphere `center`, `r`) and
    /// returns its slot, or None when the layer is full.
    #[inline]
    pub(crate) fn push(&mut self, center: [f32; 3], r: f32) -> Option<&mut [f32]> {
        let k = self.count;
        if k >= self.max_cap {
            return None;
        }
        self.count = k + 1;
        self.bounds.add(center, r);
        Some(&mut self.scratch[k * STRIDE..(k + 1) * STRIDE])
    }

    /// The single batched upload for this MultiMesh this frame. Returns the
    /// bytes uploaded.
    pub(crate) fn commit(&mut self, rs: &mut Gd<RenderingServer>) -> usize {
        let want = next_capacity(self.cap, self.count, self.min_cap, self.max_cap);
        if want != self.cap {
            // Reallocation clears the data; the buffer is set right after,
            // in the same frame.
            Self::allocate(rs, self.mm, want);
            self.cap = want;
            self.buf.resize(want * STRIDE);
            self.reallocs += 1;
        }
        let mut bytes = 0;
        if self.count > 0 {
            let n = self.count * STRIDE;
            self.buf.as_mut_slice()[..n].copy_from_slice(&self.scratch[..n]);
            rs.multimesh_set_buffer(self.mm, &self.buf);
            bytes = self.cap * STRIDE * 4;
            let (lo, hi) = (self.bounds.lo, self.bounds.hi);
            rs.multimesh_set_custom_aabb(
                self.mm,
                Aabb::new(
                    Vector3::new(lo[0], lo[1], lo[2]),
                    Vector3::new(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]),
                ),
            );
        }
        rs.multimesh_set_visible_instances(self.mm, self.count as i32);
        bytes
    }

    pub(crate) fn free(&mut self, rs: &mut Gd<RenderingServer>) {
        rs.free_rid(self.inst);
        rs.free_rid(self.mm);
    }
}

/// The skinned soldier material: the baked bone-matrix texture (every clip
/// of [`rig`]) on `shader`. Returns the texture too, so the caller keeps it
/// alive while the material's RIDs are in use.
pub(crate) fn soldier_material(shader: &Gd<Shader>) -> (Gd<ShaderMaterial>, Gd<ImageTexture>) {
    let tex_data = rig::bake_animation_texture();
    let bytes: PackedByteArray = tex_data.iter().flat_map(|f| f.to_le_bytes()).collect();
    let image = Image::create_from_data(
        rig::TEX_WIDTH as i32,
        rig::TEX_HEIGHT as i32,
        false,
        ImageFormat::RGBAF,
        &bytes,
    )
    .expect("animation image");
    let anim_tex = ImageTexture::create_from_image(&image).expect("animation texture");
    let mut mat = ShaderMaterial::new_gd();
    mat.set_shader(shader);
    mat.set_shader_parameter("anim_tex", &anim_tex.to_variant());
    mat.set_shader_parameter(
        "frames_per_clip",
        &(rig::FRAMES_PER_CLIP as i32).to_variant(),
    );
    mat.set_shader_parameter("walk_row0", &(rig::WALK_ROW0 as i32).to_variant());
    mat.set_shader_parameter("idle_row0", &(rig::IDLE_ROW0 as i32).to_variant());
    (mat, anim_tex)
}

/// Converts plain mesh data to an `ArrayMesh` (one surface). Skinned meshes
/// carry `[bone0, bone1, w0, w1]` in CUSTOM0 as RGBA float.
pub(crate) fn build_mesh(md: &MeshData, material: Option<Gd<Material>>) -> Gd<ArrayMesh> {
    let mut arrays = VarArray::new();
    arrays.resize(ArrayType::MAX.ord() as usize, &Variant::nil());
    let v3 = |a: &[[f32; 3]]| {
        a.iter()
            .map(|p| Vector3::new(p[0], p[1], p[2]))
            .collect::<PackedVector3Array>()
    };
    arrays.set(
        ArrayType::VERTEX.ord() as usize,
        &v3(&md.positions).to_variant(),
    );
    arrays.set(
        ArrayType::NORMAL.ord() as usize,
        &v3(&md.normals).to_variant(),
    );
    // Mesh colours are authored in sRGB; shaders and the vertex-colour
    // albedo path expect linear values.
    let lin = |v: f32| v.max(0.0).powf(2.2);
    let colors: PackedColorArray = md
        .colors
        .iter()
        .map(|c| Color::from_rgba(lin(c[0]), lin(c[1]), lin(c[2]), c[3]))
        .collect();
    arrays.set(ArrayType::COLOR.ord() as usize, &colors.to_variant());
    let mut flags: u64 = 0;
    if !md.custom0.is_empty() {
        let custom: PackedFloat32Array = md.custom0.iter().flatten().copied().collect();
        arrays.set(ArrayType::CUSTOM0.ord() as usize, &custom.to_variant());
        flags |= (ArrayCustomFormat::RGBA_FLOAT.ord() as u64) << ArrayFormat::CUSTOM0_SHIFT.ord();
    }
    let indices = PackedInt32Array::from(md.indices.as_slice());
    arrays.set(ArrayType::INDEX.ord() as usize, &indices.to_variant());
    let mut mesh = ArrayMesh::new_gd();
    mesh.add_surface_from_arrays_ex(PrimitiveType::TRIANGLES, &arrays.upcast_any_array())
        .flags(ArrayFormat::from_ord(flags))
        .done();
    if let Some(m) = material {
        mesh.surface_set_material(0, &m);
    }
    mesh
}
