// Match terrain (FrontlineMatch chunks). The vertex colour carries the
// ground type the bridge computed (grass by height, woods floor, slope dirt,
// fresh earth in trenches and craters, spoil on parapets and rims); this
// shader breaks it up with world-space value noise so open ground reads at
// RTS heights: field-sized patches, tufts, and a fine grain. Presentation
// only (§12); a style placeholder until the art pass (§12.1).
shader_type spatial;
render_mode cull_back, diffuse_burley, specular_disabled;

uniform float patch_scale = 0.035;
uniform float tuft_scale = 0.45;

varying vec3 v_world;

float hash(vec2 p) {
	p = fract(p * vec2(123.34, 456.21));
	p += dot(p, p + 45.32);
	return fract(p.x * p.y);
}

float noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	vec2 u = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
			mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
	float v = 0.0;
	float a = 0.5;
	for (int i = 0; i < 4; i++) {
		v += a * noise(p);
		p = p * 2.03 + vec2(17.1, 9.2);
		a *= 0.5;
	}
	return v;
}

void vertex() {
	v_world = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec3 c = COLOR.rgb;
	vec2 p = v_world.xz;
	float fields = fbm(p * patch_scale);
	float tufts = fbm(p * tuft_scale);
	float grain = noise(p * 4.0);
	// Earth (reddish-brown vertex colours) keeps its own look; grass gets
	// dry and lush patches.
	float earth = smoothstep(1.05, 1.4, c.r / max(c.g, 1e-4));
	vec3 dry = c * vec3(1.22, 1.12, 0.84);
	vec3 lush = c * vec3(0.78, 0.92, 0.80);
	vec3 grass = mix(lush, dry, smoothstep(0.38, 0.68, fields));
	grass *= 0.82 + 0.30 * tufts;
	vec3 col = mix(grass, c * (0.85 + 0.3 * tufts), earth);
	col *= 0.93 + 0.14 * grain;
	ALBEDO = col;
	ROUGHNESS = 0.96;
}
