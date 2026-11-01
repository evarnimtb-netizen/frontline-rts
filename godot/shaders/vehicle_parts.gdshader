// S3 vehicle parts (hull, turret, gun) from FrontlineMeshes, one MultiMesh
// per type and part (godot/match/vehicle_view.gd). Vertex COLOR.a = 1 marks
// painted faces that take the instance tint; INSTANCE_CUSTOM.rgb is the
// tint (sRGB), INSTANCE_CUSTOM.a a brightness factor (wrecks are dark).
shader_type spatial;
render_mode cull_back, diffuse_lambert, specular_disabled;

varying vec3 v_albedo;

void vertex() {
	vec3 tint = pow(INSTANCE_CUSTOM.rgb, vec3(2.2));
	v_albedo = (COLOR.a > 0.5 ? tint * COLOR.rgb : COLOR.rgb) * INSTANCE_CUSTOM.a;
}

void fragment() {
	ALBEDO = v_albedo;
	ROUGHNESS = 0.82;
}
