//! `FrontlineSim`: the Godot node that runs the sim and draws it (§4.1).
//!
//! * The sim runs on its own thread ([`SimHandle`]); `_process` only swaps in
//!   the newest published frame (lock-free) and never waits for a tick.
//! * Soldiers, placeholder vehicles and tracers are drawn through
//!   `RenderingServer` MultiMeshes owned by this node. There is no Node per
//!   unit. Each MultiMesh gets at most one `multimesh_set_buffer` per frame
//!   (12 transform + 4 custom floats per instance) plus
//!   `multimesh_set_visible_instances` and a tight `custom_aabb`. Capacities
//!   follow what is drawn (with hysteresis), so the upload is about the
//!   visible set, not the caps.
//! * Frustum, distance and shadow-caster culling happen here in Rust; each
//!   soldier LOD has its own MultiMesh and draws without shadows. Shadows
//!   come from separate shadow-only caster MultiMeshes holding the instances
//!   whose shadow can land in view within the sun's shadow distance.
//!   Soldiers are skinned in the shader from a baked bone-matrix texture (no
//!   `Skeleton3D`).
//! * Motion is interpolated between two sim snapshots on their *scheduled*
//!   tick times, drawn a fixed delay (one tick + `interp_margin_ms`) behind
//!   the wall clock. The drawn state is a function of time only; it is never
//!   fed back into the next interpolation.
//! * The 300-ish tanks are **placeholders that are not in the sim**: the
//!   bridge drives them on circles purely so draw counts resemble the
//!   Regiment cap (§2.2). They have no gameplay meaning.
//!
//! The node must sit at the world origin with an identity transform: the map
//! is centred on the origin (see `present.rs` for the axis mapping).

use std::time::Instant;

use frontline_sim::terrain::SPACING_M;
use frontline_sim::{SimConfig, TickRate, World};
use godot::classes::base_material_3d::Flags as MatFlags;
use godot::classes::light_3d::Param as LightParam;
use godot::classes::notify::Node3DNotification;
use godot::classes::rendering_server::ShadowCastingSetting;
use godot::classes::{
    DirectionalLight3D, INode3D, Node3D, RenderingServer, Shader, ShaderMaterial,
    StandardMaterial3D,
};
use godot::prelude::*;

use crate::gfx::{Layer, build_mesh, soldier_material};
use crate::handoff::{Frame, SimHandle};
use crate::meshgen;
use crate::present::{
    CasterCull, Frustum, HeightSampler, carry_over, hash01, interp_pair, lerp, lerp_angle,
    model_yaw, sim_to_godot, write_streak_instance, write_yaw_instance,
};
use crate::rig;

const SOLDIER_SHADER: &str = "res://shaders/soldier_skinned.gdshader";
const VEHICLE_SHADER: &str = "res://shaders/vehicle.gdshader";
const TRACER_SHADER: &str = "res://shaders/tracer.gdshader";

/// A soldier whose planar position jumps more than this between the two
/// interpolated snapshots is snapped (treated as a new entity).
const MAX_JUMP_M: f32 = 10.0;

/// Names of the values `get_frame_counters()` returns, in order.
const FRAME_COUNTERS: [&str; 13] = [
    "presenter_us",
    "snapshot_age_ms",
    "interp_alpha",
    "instances",
    "soldiers_lod0",
    "soldiers_lod1",
    "soldiers_lod2",
    "soldier_casters",
    "vehicles",
    "vehicle_casters",
    "tracers",
    "upload_kb",
    "sim_alive",
];

#[derive(GodotClass)]
#[class(base = Node3D)]
pub struct FrontlineSim {
    base: Base<Node3D>,
    /// Soldiers spawned at tick 0 (sim entities).
    #[export]
    soldiers: i32,
    /// Placeholder tanks drawn by the bridge only (NOT simulated).
    #[export]
    vehicles: i32,
    /// Projectile pool size (also the tracer MultiMesh capacity limit).
    #[export]
    projectile_cap: i32,
    /// Square map side in metres; rounded to a multiple of 64.
    #[export]
    map_size_m: i32,
    /// Side of the centred square soldiers spawn in (0 = whole map).
    #[export]
    spawn_area_m: i32,
    #[export]
    seed: i64,
    /// Workers in the sim's own pool (§4.1 rule 7). The pacing thread is
    /// extra but only sleeps or waits while a worker runs the tick.
    #[export]
    sim_threads: i32,
    /// Match tick rate: 30 (default) or 20.
    #[export]
    tick_hz: i32,
    #[export]
    players: i32,
    /// LOD switch distances (m); beyond `draw_distance` nothing is drawn.
    #[export]
    lod0_distance: f32,
    #[export]
    lod1_distance: f32,
    #[export]
    draw_distance: f32,
    /// Seconds of flight a tracer streak shows.
    #[export]
    tracer_seconds: f32,
    /// Render delay beyond one tick (ms): covers the sim step time so the
    /// newer snapshot has normally landed before it is needed.
    #[export]
    interp_margin_ms: f32,
    /// Shadowed directional light used for shadow-caster culling. Without
    /// one, units cast no shadows.
    #[export]
    sun: Option<Gd<DirectionalLight3D>>,
    #[export]
    soldier_shader: Option<Gd<Shader>>,
    #[export]
    vehicle_shader: Option<Gd<Shader>>,
    #[export]
    tracer_shader: Option<Gd<Shader>>,
    rt: Option<Runtime>,
}

#[godot_api]
impl INode3D for FrontlineSim {
    fn init(base: Base<Node3D>) -> Self {
        FrontlineSim {
            base,
            soldiers: 2000,
            vehicles: 300,
            projectile_cap: 5000,
            map_size_m: 1024,
            spawn_area_m: 640,
            seed: 1,
            sim_threads: 3,
            tick_hz: 30,
            players: 2,
            lod0_distance: 25.0,
            lod1_distance: 80.0,
            draw_distance: 1500.0,
            tracer_seconds: 0.015,
            interp_margin_ms: 12.0,
            sun: None,
            soldier_shader: None,
            vehicle_shader: None,
            tracer_shader: None,
            rt: None,
        }
    }

    fn ready(&mut self) {
        if godot::classes::Engine::singleton().is_editor_hint() {
            return;
        }
        self.start();
    }

    fn process(&mut self, delta: f64) {
        let t0 = Instant::now();
        let cam = self.base().get_viewport().and_then(|vp| vp.get_camera_3d());
        let (cam_pos, cam_fwd, frustum) = match cam {
            Some(cam) => {
                let xf = cam.get_global_transform();
                let p = xf.origin;
                let f = -xf.basis.col_c().normalized();
                let planes = cam
                    .get_frustum()
                    .iter_shared()
                    .map(|pl| [pl.normal.x, pl.normal.y, pl.normal.z, pl.d])
                    .collect();
                ([p.x, p.y, p.z], [f.x, f.y, f.z], Frustum { planes })
            }
            None => ([0.0, 100.0, 0.0], [0.0, -1.0, 0.0], Frustum::all()),
        };
        let mut cull = CasterCull {
            frustum: frustum.clone(),
            cam: cam_pos,
            cam_fwd,
            ..CasterCull::default()
        };
        if let Some(sun) = &self.sun
            && sun.is_visible_in_tree()
            && sun.has_shadow()
        {
            let l = -sun.get_global_transform().basis.col_c().normalized();
            cull.light = [l.x, l.y, l.z];
            cull.max_depth = sun.get_param(LightParam::SHADOW_MAX_DISTANCE);
            cull.enabled = true;
        }
        let lods = [
            self.lod0_distance,
            self.lod1_distance,
            self.draw_distance,
            self.tracer_seconds,
        ];
        let margin_s = (self.interp_margin_ms.max(0.0) / 1000.0) as f64;
        if let Some(rt) = self.rt.as_mut() {
            rt.delay = rt.period as f64 + margin_s;
            rt.update(&frustum, &cull, delta as f32, lods);
            rt.present_us = t0.elapsed().as_secs_f32() * 1e6;
            if !rt.sim.is_alive() && !rt.sim_dead_reported {
                rt.sim_dead_reported = true;
                godot_error!(
                    "FrontlineSim: the sim thread panicked and stopped: {}",
                    rt.sim.panic_message().unwrap_or_default()
                );
            }
        }
    }

    fn exit_tree(&mut self) {
        self.stop();
        // `_ready` runs once per node unless re-requested; re-entering the
        // tree must start the sim again.
        self.base_mut().request_ready();
    }

    fn on_notification(&mut self, what: Node3DNotification) {
        if what == Node3DNotification::VISIBILITY_CHANGED {
            let visible = self.base().is_visible_in_tree();
            if let Some(rt) = self.rt.as_mut() {
                rt.set_visible(visible);
            }
        }
    }
}

#[godot_api]
impl FrontlineSim {
    /// Builds the world and render resources and starts the sim thread.
    /// Called from `_ready`; set properties before adding the node.
    #[func]
    fn start(&mut self) {
        self.stop();
        let map = ((self.map_size_m.clamp(64, 8192) + 32) / 64 * 64).clamp(64, 8192);
        if map != self.map_size_m {
            godot_warn!(
                "FrontlineSim: map_size_m {} rounded to {}",
                self.map_size_m,
                map
            );
            self.map_size_m = map;
        }
        let tick_rate = match self.tick_hz {
            20 => TickRate::Hz20,
            30 => TickRate::Hz30,
            other => {
                godot_warn!("FrontlineSim: tick_hz {other} unsupported, using 30");
                self.tick_hz = 30;
                TickRate::Hz30
            }
        };
        let cfg = SimConfig {
            seed: self.seed as u64,
            tick_rate,
            map_size_m: map,
            soldiers: self.soldiers.max(0) as u32,
            spawn_area_m: self.spawn_area_m.clamp(0, map),
            players: self.players.clamp(1, 255) as u8,
            projectile_cap: self.projectile_cap.max(0) as u32,
            ..SimConfig::default()
        };
        let world = match World::new(cfg, self.sim_threads.max(1) as usize) {
            Ok(w) => w,
            Err(e) => {
                godot_error!("FrontlineSim: invalid sim config: {e}");
                return;
            }
        };
        let Some(world3d) = self.base().get_world_3d() else {
            godot_error!("FrontlineSim: node is not in a 3D world");
            return;
        };
        let scenario = world3d.get_scenario();
        let shaders = [
            shader_or_load(&self.soldier_shader, SOLDIER_SHADER),
            shader_or_load(&self.vehicle_shader, VEHICLE_SHADER),
            shader_or_load(&self.tracer_shader, TRACER_SHADER),
        ];
        let [Some(soldier_sh), Some(vehicle_sh), Some(tracer_sh)] = shaders else {
            godot_error!("FrontlineSim: missing shaders");
            return;
        };
        if self.sun.is_none() {
            godot_warn!("FrontlineSim: no `sun` set; units will cast no shadows");
        }
        match Runtime::new(
            world,
            self.tick_hz as u32,
            scenario,
            self.vehicles.max(0) as usize,
            self.spawn_area_m,
            [soldier_sh, vehicle_sh, tracer_sh],
        ) {
            Ok(mut rt) => {
                rt.set_visible(self.base().is_visible_in_tree());
                self.rt = Some(rt);
            }
            Err(e) => {
                godot_error!("FrontlineSim: could not start sim thread: {e}");
            }
        }
    }

    /// Stops the sim thread and frees render resources.
    #[func]
    fn stop(&mut self) {
        if let Some(mut rt) = self.rt.take() {
            rt.free();
        }
    }

    #[func]
    fn is_running(&self) -> bool {
        self.rt.is_some()
    }

    /// False if the sim thread died (panicked); the picture is then frozen.
    #[func]
    fn is_sim_alive(&self) -> bool {
        self.rt.as_ref().is_some_and(|rt| rt.sim.is_alive())
    }

    /// Wall time (ms) of the last `World::step`.
    #[func]
    fn get_sim_tick_ms(&self) -> f32 {
        self.rt.as_ref().map_or(0.0, |rt| rt.newest().step_ms)
    }

    /// Age (ms) of the newest snapshot received.
    #[func]
    fn get_snapshot_age_ms(&self) -> f32 {
        self.rt.as_ref().map_or(0.0, |rt| rt.snapshot_age_ms())
    }

    /// Instances written this frame across all visible MultiMeshes (casters
    /// not included).
    #[func]
    fn get_instances_drawn(&self) -> i32 {
        self.rt.as_ref().map_or(0, |rt| rt.instances_drawn() as i32)
    }

    /// Terrain height (Godot Y) at Godot (x, z).
    #[func]
    fn get_height_at(&self, x: f32, z: f32) -> f32 {
        self.rt.as_ref().map_or(0.0, |rt| rt.heights.at_godot(x, z))
    }

    /// Clears the sim step-time window and late-tick counters.
    #[func]
    fn reset_sim_stats(&self) {
        if let Some(rt) = &self.rt {
            rt.sim.reset_window();
        }
    }

    /// Cheap per-frame counters (no locks, no sorting), in the order of
    /// `get_frame_counter_names()`. For benchmarks that sample every frame.
    #[func]
    fn get_frame_counters(&self) -> PackedFloat32Array {
        let Some(rt) = &self.rt else {
            return PackedFloat32Array::new();
        };
        let l = &rt.soldier_layers;
        let v = [
            rt.present_us,
            rt.snapshot_age_ms(),
            rt.last_alpha,
            rt.instances_drawn() as f32,
            l[0].count as f32,
            l[1].count as f32,
            l[2].count as f32,
            rt.soldier_casters.count as f32,
            rt.vehicle_layer.count as f32,
            rt.vehicle_casters.count as f32,
            rt.tracer_layer.count as f32,
            rt.upload_bytes as f32 / 1024.0,
            if rt.sim.is_alive() { 1.0 } else { 0.0 },
        ];
        PackedFloat32Array::from(v.as_slice())
    }

    #[func]
    fn get_frame_counter_names(&self) -> PackedStringArray {
        FRAME_COUNTERS.iter().map(|s| GString::from(*s)).collect()
    }

    /// Presenter and sim statistics for HUD and benchmarks. Copies and sorts
    /// the sim step-time window: call it a few times a second at most.
    #[func]
    fn get_stats(&self) -> VarDictionary {
        use std::sync::atomic::Ordering::Relaxed;
        let Some(rt) = &self.rt else {
            return VarDictionary::new();
        };
        let (n, mean, p50, p95, p99, max) = rt.sim.step_summary();
        let c = &rt.sim.counters;
        let cur = rt.newest();
        let caps: PackedInt64Array = rt
            .soldier_layers
            .iter()
            .chain([
                &rt.soldier_casters,
                &rt.vehicle_layer,
                &rt.vehicle_casters,
                &rt.tracer_layer,
            ])
            .map(|l| l.cap as i64)
            .collect();
        vdict! {
            "sim_tick" => cur.tick as i64,
            "sim_ticks_run" => c.ticks.load(Relaxed) as i64,
            "sim_tick_hz" => rt.sim.tick_hz as i64,
            "sim_threads" => self.sim_threads as i64,
            "sim_alive" => rt.sim.is_alive(),
            "sim_panic" => rt.sim.panic_message().unwrap_or_default(),
            "sim_step_ms_last" => cur.step_ms,
            "sim_step_samples" => n as i64,
            "sim_step_samples_dropped" => c.dropped_samples.load(Relaxed) as i64,
            "sim_step_ms_mean" => mean,
            "sim_step_ms_p50" => p50,
            "sim_step_ms_p95" => p95,
            "sim_step_ms_p99" => p99,
            "sim_step_ms_max" => max,
            "sim_late_ticks" => c.late_ticks.load(Relaxed) as i64,
            "sim_resyncs" => c.resyncs.load(Relaxed) as i64,
            "snapshot_age_ms" => rt.snapshot_age_ms(),
            "render_delay_ms" => (rt.delay * 1000.0) as f32,
            "soldiers_live" => cur.soldiers.len() as i64,
            "projectiles_live" => cur.projectiles.len() as i64,
            "soldiers_lod0" => rt.soldier_layers[0].count as i64,
            "soldiers_lod1" => rt.soldier_layers[1].count as i64,
            "soldiers_lod2" => rt.soldier_layers[2].count as i64,
            "soldier_casters" => rt.soldier_casters.count as i64,
            "vehicles_drawn" => rt.vehicle_layer.count as i64,
            "vehicle_casters" => rt.vehicle_casters.count as i64,
            "vehicles_placeholder_total" => rt.vehicles.len() as i64,
            "tracers_drawn" => rt.tracer_layer.count as i64,
            "instances_drawn" => rt.instances_drawn() as i64,
            "upload_bytes" => rt.upload_bytes as i64,
            "layer_capacities" => &caps,
            "layer_reallocs" => rt.reallocs() as i64,
            "presenter_cpu_us" => rt.present_us,
            "soldier_tris_lod" => &rt.soldier_tris.iter().map(|&t| t as i64).collect::<PackedInt64Array>(),
            "terrain_tris" => rt.terrain_tris as i64,
            "terrain_casts_shadows" => false,
        }
    }
}

fn shader_or_load(sh: &Option<Gd<Shader>>, path: &str) -> Option<Gd<Shader>> {
    sh.clone()
        .or_else(|| match godot::tools::try_load::<Shader>(path) {
            Ok(s) => Some(s),
            Err(e) => {
                godot_error!("FrontlineSim: cannot load {path}: {e}");
                None
            }
        })
}

/// A placeholder tank on a circular path. Not a sim entity.
struct PlaceholderVehicle {
    center: [f32; 2],
    radius: f32,
    omega: f32,
    phase: f32,
    team: f32,
}

// --------------------------------------------------------------- runtime --

/// Frames kept for interpolation (oldest first).
const HIST: usize = 3;

struct Runtime {
    sim: SimHandle,
    period: f32,
    /// Render delay (s): drawn time = now − delay.
    delay: f64,
    half: f32,
    heights: HeightSampler,
    epoch: Instant,
    /// Newest frames, oldest first; the last `nhist` are valid.
    hist: [Frame; HIST],
    nhist: usize,
    /// Slot the next frame is swapped into.
    spare: Frame,
    /// (tick a, tick b) the cached `start` was built for.
    pair: Option<(u64, u64)>,
    /// Interpolation start per soldier of frame b (a's value by id).
    start: Vec<[f32; 4]>,
    snapped: Vec<u32>,
    /// Per sim id: (walk phase, idle phase).
    phases: Vec<[f32; 2]>,
    soldier_layers: [Layer; 3],
    soldier_casters: Layer,
    vehicle_layer: Layer,
    vehicle_casters: Layer,
    tracer_layer: Layer,
    terrain_inst: Rid,
    terrain_tris: usize,
    vehicles: Vec<PlaceholderVehicle>,
    clock: f64,
    present_us: f32,
    last_alpha: f32,
    upload_bytes: usize,
    soldier_tris: [usize; 3],
    sim_dead_reported: bool,
    // Keep engine resources alive while the RIDs are in use.
    _resources: Vec<Gd<Resource>>,
}

impl Runtime {
    fn new(
        world: World,
        tick_hz: u32,
        scenario: Rid,
        vehicle_count: usize,
        spawn_area_m: i32,
        [soldier_sh, vehicle_sh, tracer_sh]: [Gd<Shader>; 3],
    ) -> std::io::Result<Runtime> {
        let mut rs = RenderingServer::singleton();
        let cfg = world.config().clone();
        let map = cfg.map_size_m as f32;
        let half = map * 0.5;
        let terrain = world.terrain();
        let dim = terrain.dim() as usize;
        let heights = HeightSampler {
            heights: terrain.heights().iter().map(|h| h.to_f32()).collect(),
            dim,
            spacing: SPACING_M as f32,
            half,
        };
        let (hmin, hmax) = heights
            .heights
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), &h| (a.min(h), b.max(h)));
        // Initial box only; each commit sets a tight one.
        let aabb = Aabb::new(
            Vector3::new(-half - 10.0, hmin - 10.0, -half - 10.0),
            Vector3::new(map + 20.0, (hmax - hmin) + 220.0, map + 20.0),
        );
        let mut resources: Vec<Gd<Resource>> = Vec::new();

        // Terrain (static; one RS instance). It does not cast shadows: one
        // un-chunked mesh would go whole into every shadow split (until the
        // CDLOD terrain exists, §4.1 rule 6).
        let stride = (dim - 1).div_ceil(512).max(1);
        let terrain_md = meshgen::terrain_mesh(&heights.heights, dim, SPACING_M as f32, stride);
        let terrain_tris = terrain_md.tri_count();
        let mut terrain_mat = StandardMaterial3D::new_gd();
        terrain_mat.set_flag(MatFlags::ALBEDO_FROM_VERTEX_COLOR, true);
        terrain_mat.set_roughness(1.0);
        let terrain_mesh = build_mesh(&terrain_md, Some(terrain_mat.upcast()));
        let terrain_inst = rs.instance_create2(terrain_mesh.get_rid(), scenario);
        rs.instance_geometry_set_cast_shadows_setting(terrain_inst, ShadowCastingSetting::OFF);
        resources.push(terrain_mesh.upcast());

        // Soldier material: shared bone texture + skinning shader.
        let (soldier_mat, anim_tex) = soldier_material(&soldier_sh);
        resources.push(anim_tex.upcast());

        let soldier_cap = cfg.soldiers as usize + 64;
        let mut soldier_tris = [0usize; 3];
        let mut soldier_meshes = Vec::new();
        let soldier_layers: [Layer; 3] = std::array::from_fn(|lod| {
            let md = rig::soldier_mesh(lod as u8);
            soldier_tris[lod] = md.tri_count();
            let mesh = build_mesh(&md, Some(soldier_mat.clone().upcast()));
            // LOD0 is near-camera only: small floor.
            let min_cap = if lod == 0 { 16 } else { 64 };
            let layer = Layer::new(
                &mut rs,
                mesh.get_rid(),
                scenario,
                min_cap,
                soldier_cap,
                ShadowCastingSetting::OFF,
                aabb,
            );
            soldier_meshes.push(mesh.get_rid());
            resources.push(mesh.upcast());
            layer
        });
        // Shadow-only casters use the LOD1 mesh.
        let soldier_casters = Layer::new(
            &mut rs,
            soldier_meshes[1],
            scenario,
            64,
            soldier_cap,
            ShadowCastingSetting::SHADOWS_ONLY,
            aabb,
        );

        let mut vehicle_mat = ShaderMaterial::new_gd();
        vehicle_mat.set_shader(&vehicle_sh);
        let tank = build_mesh(&meshgen::tank_mesh(), Some(vehicle_mat.upcast()));
        let vehicle_layer = Layer::new(
            &mut rs,
            tank.get_rid(),
            scenario,
            16,
            vehicle_count,
            ShadowCastingSetting::OFF,
            aabb,
        );
        let vehicle_casters = Layer::new(
            &mut rs,
            tank.get_rid(),
            scenario,
            16,
            vehicle_count,
            ShadowCastingSetting::SHADOWS_ONLY,
            aabb,
        );
        resources.push(tank.upcast());

        let mut tracer_mat = ShaderMaterial::new_gd();
        tracer_mat.set_shader(&tracer_sh);
        let tracer = build_mesh(&meshgen::tracer_mesh(), Some(tracer_mat.upcast()));
        let tracer_layer = Layer::new(
            &mut rs,
            tracer.get_rid(),
            scenario,
            64,
            cfg.projectile_cap as usize,
            ShadowCastingSetting::OFF,
            aabb,
        );
        resources.push(tracer.upcast());

        // Placeholder vehicles spread over the spawn area (not simulated).
        let area = if spawn_area_m <= 0 {
            map
        } else {
            spawn_area_m as f32
        };
        let lo = half - area * 0.5;
        let vehicles = (0..vehicle_count as u32)
            .map(|i| {
                let speed = 2.0 + 3.0 * hash01(i * 4 + 2);
                let radius = 15.0 + 45.0 * hash01(i * 4 + 3);
                let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
                PlaceholderVehicle {
                    center: [lo + area * hash01(i * 4), lo + area * hash01(i * 4 + 1)],
                    radius,
                    omega: dir * speed / radius,
                    phase: std::f32::consts::TAU * hash01(i ^ 0x5bd1_e995),
                    team: (i % cfg.players.max(1) as u32) as f32,
                }
            })
            .collect();

        let epoch = Instant::now();
        let mut sim = SimHandle::spawn(world, tick_hz)?;
        let period = 1.0 / tick_hz.max(1) as f32;
        let mut hist = [Frame::empty(), Frame::empty(), Frame::empty()];
        // Draw the initial (tick 0) state at once rather than waiting for
        // the first publish.
        sim.take_current(&mut hist[HIST - 1]);
        Ok(Runtime {
            sim,
            period,
            delay: period as f64 * 1.35,
            half,
            heights,
            epoch,
            hist,
            nhist: 1,
            spare: Frame::empty(),
            pair: None,
            start: Vec::new(),
            snapped: Vec::new(),
            phases: Vec::new(),
            soldier_layers,
            soldier_casters,
            vehicle_layer,
            vehicle_casters,
            tracer_layer,
            terrain_inst,
            terrain_tris,
            vehicles,
            clock: 0.0,
            present_us: 0.0,
            last_alpha: 0.0,
            upload_bytes: 0,
            soldier_tris,
            sim_dead_reported: false,
            _resources: resources,
        })
    }

    fn newest(&self) -> &Frame {
        &self.hist[HIST - 1]
    }

    fn snapshot_age_ms(&self) -> f32 {
        self.newest().published.elapsed().as_secs_f32() * 1000.0
    }

    fn instances_drawn(&self) -> usize {
        self.soldier_layers.iter().map(|l| l.count).sum::<usize>()
            + self.vehicle_layer.count
            + self.tracer_layer.count
    }

    fn layers(&self) -> impl Iterator<Item = &Layer> {
        self.soldier_layers.iter().chain([
            &self.soldier_casters,
            &self.vehicle_layer,
            &self.vehicle_casters,
            &self.tracer_layer,
        ])
    }

    fn reallocs(&self) -> u32 {
        self.layers().map(|l| l.reallocs).sum()
    }

    fn set_visible(&mut self, visible: bool) {
        let mut rs = RenderingServer::singleton();
        for inst in self.layers().map(|l| l.inst).collect::<Vec<_>>() {
            rs.instance_set_visible(inst, visible);
        }
        rs.instance_set_visible(self.terrain_inst, visible);
    }

    fn secs(&self, t: Instant) -> f64 {
        t.saturating_duration_since(self.epoch).as_secs_f64()
    }

    /// Swaps in the newest published frame, if any.
    fn pull_snapshot(&mut self) {
        if self.sim.take_newest(&mut self.spare) {
            self.hist.rotate_left(1);
            std::mem::swap(&mut self.hist[HIST - 1], &mut self.spare);
            self.nhist = (self.nhist + 1).min(HIST);
        }
    }

    fn update(
        &mut self,
        frustum: &Frustum,
        cull: &CasterCull,
        dt: f32,
        [lod0, lod1, draw, tracer_s]: [f32; 4],
    ) {
        let mut rs = RenderingServer::singleton();
        self.clock += dt as f64;
        self.pull_snapshot();

        // Render time and the snapshot pair around it.
        let r = self.secs(Instant::now()) - self.delay;
        let base = HIST - self.nhist;
        let mut stamps = [0.0f64; HIST];
        for (st, f) in stamps.iter_mut().zip(&self.hist[base..]) {
            *st = self.secs(f.sched);
        }
        let (ai, bi, alpha) = interp_pair(&stamps[..self.nhist], r);
        let (ai, bi) = (base + ai, base + bi);
        let stamp_b = stamps[bi - base];
        self.last_alpha = alpha;
        let key = (self.hist[ai].tick, self.hist[bi].tick);
        if self.pair != Some(key) {
            self.pair = Some(key);
            let (a, b) = (&self.hist[ai], &self.hist[bi]);
            self.snapped.clear();
            carry_over(
                &a.ids,
                &a.soldiers,
                &b.ids,
                &b.soldiers,
                MAX_JUMP_M,
                &mut self.start,
                &mut self.snapped,
            );
            if let Some(&max_id) = b.ids.last() {
                while self.phases.len() <= max_id as usize {
                    let id = self.phases.len() as u32;
                    self.phases.push([hash01(id), hash01(id ^ 0xA5A5_1234)]);
                }
            }
            // A snapped id is a different soldier (or a teleport): fresh
            // animation phases.
            for &id in &self.snapped {
                self.phases[id as usize] = [hash01(id), hash01(id ^ 0xA5A5_1234)];
            }
        }

        let Runtime {
            hist,
            start,
            phases,
            soldier_layers,
            soldier_casters,
            vehicle_layer,
            vehicle_casters,
            tracer_layer,
            vehicles,
            heights,
            half,
            period,
            clock,
            ..
        } = self;
        let (half, period) = (*half, *period);
        let b = &hist[bi];
        let cam = cull.cam;
        let (lod0_2, lod1_2, draw_2) = (lod0 * lod0, lod1 * lod1, draw * draw);
        let dist2 = |p: [f32; 3]| {
            let d = [p[0] - cam[0], p[1] - cam[1], p[2] - cam[2]];
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2]
        };

        // Soldiers.
        for l in soldier_layers.iter_mut() {
            l.begin();
        }
        soldier_casters.begin();
        for i in 0..b.soldiers.len() {
            let (s, c) = (start[i], b.soldiers[i]);
            let v = [
                lerp(s[0], c[0], alpha),
                lerp(s[1], c[1], alpha),
                lerp(s[2], c[2], alpha),
                lerp_angle(s[3], c[3], alpha),
            ];
            // Walk speed from the sim's velocity, not from position deltas.
            let spd = b.speeds[i];
            let ph = &mut phases[b.ids[i] as usize];
            ph[0] = (ph[0] + dt * spd / rig::STRIDE_M).fract();
            ph[1] = (ph[1] + dt * 0.25).fract();
            let gp = sim_to_godot([v[0], v[1], v[2]], half);
            let centre = [gp[0], gp[1] + 0.9, gp[2]];
            let walk = ((spd - 0.25) / 0.75).clamp(0.0, 1.0);
            let custom = [walk, ph[0], ph[1], b.teams[i] as f32];
            let yaw = model_yaw(v[3]);
            if cull.casts_into_view(centre, 1.2, 1.9)
                && let Some(slot) = soldier_casters.push(centre, 1.2)
            {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
            if !frustum.sphere_visible(centre, 1.2) {
                continue;
            }
            let d2 = dist2(gp);
            let lod = if d2 <= lod0_2 {
                0
            } else if d2 <= lod1_2 {
                1
            } else if d2 <= draw_2 {
                2
            } else {
                continue;
            };
            if let Some(slot) = soldier_layers[lod].push(centre, 1.2) {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
        }

        // Placeholder vehicles (not simulated).
        vehicle_layer.begin();
        vehicle_casters.begin();
        let t = *clock as f32;
        for v in vehicles.iter() {
            let th = v.phase + v.omega * t;
            let (s, c) = th.sin_cos();
            let (x, y) = (v.center[0] + v.radius * c, v.center[1] + v.radius * s);
            let z = heights.at_sim(x, y);
            let gp = sim_to_godot([x, y, z], half);
            let centre = [gp[0], gp[1] + 1.2, gp[2]];
            let yaw = model_yaw(th + v.omega.signum() * std::f32::consts::FRAC_PI_2);
            let custom = [0.0, 0.0, 0.0, v.team];
            if cull.casts_into_view(centre, 4.5, 3.0)
                && let Some(slot) = vehicle_casters.push(centre, 4.5)
            {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
            if !frustum.sphere_visible(centre, 4.5) || dist2(gp) > draw_2 {
                continue;
            }
            if let Some(slot) = vehicle_layer.push(centre, 4.5) {
                write_yaw_instance(slot, gp, yaw, 1.0, custom);
            }
        }

        // Tracers, from frame b, at the same render time as the soldiers:
        // pulled back along the velocity by (stamp_b − r). A round whose age
        // at render time is ≤ 0 has not been fired yet; the streak is never
        // longer than the distance flown, so it starts at the muzzle.
        tracer_layer.begin();
        let back = ((stamp_b - r) as f32).max(-period);
        for p in &b.projectiles {
            let vel = [p[3], p[4], p[5]];
            let speed = (vel[0] * vel[0] + vel[1] * vel[1] + vel[2] * vel[2]).sqrt();
            let flown_s = p[6] - back;
            if speed < 1.0 || flown_s <= 0.0 {
                continue;
            }
            let pos = [
                p[0] - vel[0] * back,
                p[1] - vel[1] * back,
                p[2] - vel[2] * back,
            ];
            let head = sim_to_godot(pos, half);
            let dir = [vel[0] / speed, vel[2] / speed, -vel[1] / speed];
            let len = (speed * tracer_s).clamp(0.5, 15.0).min(speed * flown_s);
            if len < 0.05 {
                continue;
            }
            let mid = [
                head[0] - dir[0] * len * 0.5,
                head[1] - dir[1] * len * 0.5,
                head[2] - dir[2] * len * 0.5,
            ];
            let d2 = dist2(mid);
            if d2 > draw_2 || !frustum.sphere_visible(mid, len * 0.5 + 0.5) {
                continue;
            }
            // Keep streaks about a pixel wide at range.
            let width = (d2.sqrt() * 0.0012).max(0.04);
            let Some(slot) = tracer_layer.push(mid, len * 0.5 + width) else {
                break;
            };
            write_streak_instance(slot, head, dir, len, width, [1.0, 0.0, 0.0, 0.0]);
        }

        let mut bytes = 0;
        for l in soldier_layers.iter_mut() {
            bytes += l.commit(&mut rs);
        }
        bytes += soldier_casters.commit(&mut rs);
        bytes += vehicle_layer.commit(&mut rs);
        bytes += vehicle_casters.commit(&mut rs);
        bytes += tracer_layer.commit(&mut rs);
        self.upload_bytes = bytes;
    }

    fn free(&mut self) {
        self.sim.stop();
        let mut rs = RenderingServer::singleton();
        for l in &mut self.soldier_layers {
            l.free(&mut rs);
        }
        self.soldier_casters.free(&mut rs);
        self.vehicle_layer.free(&mut rs);
        self.vehicle_casters.free(&mut rs);
        self.tracer_layer.free(&mut rs);
        rs.free_rid(self.terrain_inst);
    }
}
