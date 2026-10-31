// Tracer streak: unshaded, additive, bright head fading to the tail.
// The mesh spans local z 0 (head) .. 1 (tail); the bridge orients and
// scales each instance along the projectile velocity.
shader_type spatial;
render_mode unshaded, blend_add, cull_disabled, depth_draw_never, shadows_disabled;

uniform vec3 color : source_color = vec3(1.0, 0.62, 0.22);
uniform float energy = 2.5;

varying float v_t;

void vertex() {
	v_t = clamp(VERTEX.z, 0.0, 1.0);
}

void fragment() {
	float head = 1.0 - v_t;
	ALBEDO = color * energy * (0.25 + 0.75 * head * head);
}
