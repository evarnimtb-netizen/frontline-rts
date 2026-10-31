// Placeholder tank (NOT a sim entity; bridge-driven so draw counts resemble
// the Regiment cap). INSTANCE_CUSTOM.w = team (player index, 0-9). Vertex COLOR.a = 1 marks
// team-tinted parts.
shader_type spatial;
render_mode cull_back, diffuse_lambert, specular_disabled;

// One tint per player slot (SimConfig allows up to 10 players). A const:
// Godot 4.7 does not allow default values on uniform arrays. Colours are
// sRGB-authored like the mesh colours (converted below).
const vec3 TEAM_COLORS[10] = {
	vec3(0.33, 0.36, 0.30), vec3(0.45, 0.40, 0.27), vec3(0.31, 0.34, 0.39),
	vec3(0.43, 0.33, 0.29), vec3(0.37, 0.39, 0.25), vec3(0.39, 0.36, 0.41),
	vec3(0.27, 0.33, 0.31), vec3(0.47, 0.45, 0.36), vec3(0.33, 0.30, 0.25),
	vec3(0.41, 0.43, 0.43)
};

varying vec3 v_albedo;

void vertex() {
	vec3 team = pow(TEAM_COLORS[clamp(int(INSTANCE_CUSTOM.w + 0.5), 0, 9)], vec3(2.2));
	v_albedo = COLOR.a > 0.5 ? team * COLOR.rgb : COLOR.rgb;
}

void fragment() {
	ALBEDO = v_albedo;
	ROUGHNESS = 0.85;
}
