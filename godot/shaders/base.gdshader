// The faction bases' surfaces (godot/scenes/base_*.tscn), worked out in world space on the
// face's own two axes like grid.gdshader, so a box of any size gets the same pattern scale.
// 0 concrete (stains and wet patches), 1 block wall, 2 roller shutter, 3 window blinds (the
// city glows between the slats), 4 polished stone floor, 5 the ops table's map (a lit grid).
// Patterns fade where they'd shimmer (fwidth).
shader_type spatial;

uniform int kind = 0;
uniform vec3 body : source_color = vec3(0.3, 0.3, 0.3);
uniform vec3 glow : source_color = vec3(0.29, 0.78, 1.0);
uniform float glow_energy = 1.0;

varying vec3 world_pos;
varying vec3 world_n;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	world_n = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

float hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	return mix(mix(hash(i), hash(i + vec2(1, 0)), f.x), mix(hash(i + vec2(0, 1)), hash(i + vec2(1, 1)), f.x), f.y);
}

float fbm(vec2 p) {
	float v = 0.0;
	float a = 0.5;
	for (int i = 0; i < 4; i++) {
		v += a * noise(p);
		p *= 2.03;
		a *= 0.5;
	}
	return v;
}

// 1 on thin lines every `every` metres along v, faded where they get finer than a pixel
float lines(float v, float every, float width) {
	float s = v / every;
	float fw = max(fwidth(s), 1e-4);
	float d = min(fract(s), 1.0 - fract(s)) * every;
	return (1.0 - smoothstep(width, width + fw * every, d)) * (1.0 - smoothstep(0.2, 0.5, fw));
}

void fragment() {
	vec3 n = abs(world_n);
	// metres along the face: across and up (or across and deep on a floor)
	vec2 p = n.y > 0.5 ? world_pos.xz : (n.x > 0.5 ? world_pos.zy : world_pos.xy);
	vec3 col = body;
	float rough = 0.85;
	float metal = 0.0;
	vec3 emit = vec3(0.0);
	if (kind == 0) {
		float stain = fbm(p * 0.7);
		float fine = fbm(p * 9.0);
		col = body * (0.75 + 0.35 * stain) * (0.92 + 0.16 * fine);
		// wet patches on the ground: darker and glossy, so the lights lie in them
		vec2 warp = vec2(fbm(p * 0.5 + 3.0), fbm(p * 0.5 + 9.0)) * 1.5;
		float wet = n.y > 0.5 ? smoothstep(0.5, 0.72, fbm(p * 0.3 + warp)) : 0.0;
		col *= 1.0 - 0.2 * wet;
		rough = mix(0.8, 0.3, wet);
		// formwork seams every 1.5 m
		col *= 1.0 - 0.25 * max(lines(p.x, 1.5, 0.004), lines(p.y, 1.5, 0.004));
	} else if (kind == 1) {
		// 0.4 x 0.2 m blocks, every other course offset half a block
		float course = floor(p.y / 0.2);
		float x = p.x + mod(course, 2.0) * 0.2;
		float mortar = max(lines(p.y, 0.2, 0.006), lines(x, 0.4, 0.006));
		float block = hash(vec2(floor(x / 0.4), course));
		col = body * (0.85 + 0.25 * block) * (0.85 + 0.3 * fbm(p * 3.0));
		col = mix(col, body * 0.45, mortar);
	} else if (kind == 2) {
		// a roller shutter: 8 cm ridges, grime rising from the ground
		float r = fract(p.y / 0.08);
		float ridge = smoothstep(0.0, 0.25, r) * smoothstep(1.0, 0.75, r);
		col = body * (0.6 + 0.45 * ridge);
		col *= 0.7 + 0.4 * fbm(vec2(p.x * 2.0, p.y * 0.5));
		col *= mix(0.55, 1.0, smoothstep(0.0, 1.2, world_pos.y));
		metal = 0.6;
		rough = 0.5;
	} else if (kind == 3) {
		// blinds: 5 cm slats, a lit gap between each, the city behind
		float r = fract(p.y / 0.05);
		float gap = smoothstep(0.78, 0.84, r) * (1.0 - smoothstep(0.96, 1.0, r));
		float fw = fwidth(p.y / 0.05);
		gap = mix(gap, 0.18, smoothstep(0.2, 0.5, fw));
		float city = 0.6 + 0.4 * noise(vec2(p.x * 1.3, floor(p.y / 0.05)));
		col = body * (0.8 + 0.2 * smoothstep(0.0, 0.8, r));
		emit = glow * gap * city * glow_energy;
		rough = 0.6;
	} else if (kind == 4) {
		// polished stone in 1.2 m slabs, faintly veined
		float vein = fbm(p * 1.5 + vec2(fbm(p * 0.6) * 2.0));
		col = body * (0.85 + 0.25 * vein);
		col *= 1.0 - 0.5 * max(lines(p.x, 1.2, 0.002), lines(p.y, 1.2, 0.002));
		rough = 0.08 + 0.1 * fbm(p * 4.0);
	} else {
		// the ops table: a lit map grid under glass
		float g = max(lines(p.x, 0.25, 0.004), lines(p.y, 0.25, 0.004));
		float blocks = step(0.62, noise(floor(p * 4.0) + 3.0));
		col = vec3(0.02);
		emit = glow * (g * 0.8 + blocks * 0.25) * glow_energy;
		rough = 0.1;
	}
	ALBEDO = col;
	ROUGHNESS = rough;
	METALLIC = metal;
	EMISSION = emit;
}
