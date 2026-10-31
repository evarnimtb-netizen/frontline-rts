// Bone-texture skinning on a skeleton-less MultiMesh (§4.1 rule 4, §2.2 M0
// gate 7). No Skeleton3D: every instance is skinned here from a baked
// bone-matrix texture shared by all soldiers on the rig.
//
// anim_tex: RGBA32F, one row per frame, 3 texels per bone = rows of that
//           bone's 3x4 skinning matrix (pose * inverse bind).
// CUSTOM0 (per vertex): (bone0, bone1, weight0, weight1).
// INSTANCE_CUSTOM (per instance, written by the Rust bridge):
//   x = walk weight (0 = pose clip, 1 = walk), y = walk phase,
//   z = pose clip + its phase: [0,1) idle, [1,2) crouch, [2,3) prone,
//       [3,4] death fall (non-looping; 4.0 = lying, the corpse pose),
//   w = team colour index (0-9).
// Pose clip k starts at row idle_row0 + k * frames_per_clip (rig.rs).
shader_type spatial;
render_mode cull_back, diffuse_lambert, specular_disabled;

uniform sampler2D anim_tex : filter_nearest, repeat_disable;
uniform int frames_per_clip = 32;
uniform int walk_row0 = 0;
uniform int idle_row0 = 32;
// One tint per player slot (SimConfig allows up to 10 players). A const:
// Godot 4.7 does not allow default values on uniform arrays. Colours are
// sRGB-authored like the mesh colours (converted below).
const vec3 TEAM_COLORS[10] = {
	vec3(0.30, 0.33, 0.27), vec3(0.42, 0.37, 0.24), vec3(0.28, 0.31, 0.36),
	vec3(0.40, 0.30, 0.26), vec3(0.34, 0.36, 0.22), vec3(0.36, 0.33, 0.38),
	vec3(0.24, 0.30, 0.28), vec3(0.44, 0.42, 0.33), vec3(0.30, 0.27, 0.22),
	vec3(0.38, 0.40, 0.40)
};

varying vec3 v_albedo;

void accum(int bone, int row, float w, inout vec4 a0, inout vec4 a1, inout vec4 a2) {
	int x = bone * 3;
	a0 += w * texelFetch(anim_tex, ivec2(x, row), 0);
	a1 += w * texelFetch(anim_tex, ivec2(x + 1, row), 0);
	a2 += w * texelFetch(anim_tex, ivec2(x + 2, row), 0);
}

// Adds one clip's contribution (two frames, up to two bones) with weight k.
// A non-looping clip maps phase 0..1 onto its frames 0..last and holds the
// last one.
void add_clip(int row0, float phase, bool looping, float k, int b0, int b1, float w0, float w1,
		inout vec4 a0, inout vec4 a1, inout vec4 a2) {
	int last = frames_per_clip - 1;
	float f = looping ? fract(phase) * float(frames_per_clip) : clamp(phase, 0.0, 1.0) * float(last);
	int f0 = min(int(floor(f)), last);
	int f1 = looping ? (f0 + 1) % frames_per_clip : min(f0 + 1, last);
	float t = f - float(f0);
	accum(b0, row0 + f0, k * w0 * (1.0 - t), a0, a1, a2);
	accum(b0, row0 + f1, k * w0 * t, a0, a1, a2);
	if (w1 > 0.0) {
		accum(b1, row0 + f0, k * w1 * (1.0 - t), a0, a1, a2);
		accum(b1, row0 + f1, k * w1 * t, a0, a1, a2);
	}
}

void vertex() {
	vec4 ic = INSTANCE_CUSTOM;
	float walk = clamp(ic.x, 0.0, 1.0);
	int b0 = int(CUSTOM0.x + 0.5);
	int b1 = int(CUSTOM0.y + 0.5);
	float w0 = CUSTOM0.z;
	float w1 = CUSTOM0.w;
	vec4 a0 = vec4(0.0);
	vec4 a1 = vec4(0.0);
	vec4 a2 = vec4(0.0);
	// Exact 0 / 1 thresholds: the clip weights always sum to 1.
	if (walk > 0.0) {
		add_clip(walk_row0, ic.y, true, walk, b0, b1, w0, w1, a0, a1, a2);
	}
	if (walk < 1.0) {
		int clip = clamp(int(floor(ic.z)), 0, 3);
		float phase = ic.z - float(clip);
		add_clip(idle_row0 + clip * frames_per_clip, phase, clip < 3, 1.0 - walk, b0, b1, w0, w1, a0, a1, a2);
	}
	vec4 p = vec4(VERTEX, 1.0);
	VERTEX = vec3(dot(a0, p), dot(a1, p), dot(a2, p));
	NORMAL = normalize(vec3(dot(a0.xyz, NORMAL), dot(a1.xyz, NORMAL), dot(a2.xyz, NORMAL)));
	vec3 team = pow(TEAM_COLORS[clamp(int(ic.w + 0.5), 0, 9)], vec3(2.2));
	v_albedo = COLOR.a > 0.5 ? team * COLOR.rgb : COLOR.rgb;
}

void fragment() {
	ALBEDO = v_albedo;
	ROUGHNESS = 0.9;
}
