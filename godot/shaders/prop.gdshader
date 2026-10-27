// The look of a map's blocks (cover, door panels), so each reads as the thing it is: a cabinet
// (rack doors and status lights), a crate (slats), a machine (vent slots, a striped base), a
// barrier (hazard stripes), a kerb or plinth (a striped top edge), a door (a metal panel, a
// status light).
// On a box mesh whose size (metres) comes per instance; worked out in the box's own space, so
// each face's pattern starts at its bottom edge. Patterns fade where they'd shimmer (fwidth).
shader_type spatial;

instance uniform vec3 size = vec3(1.0);
// 0 cabinet, 1 crate, 2 machine, 3 barrier, 4 kerb, 5 door
uniform int kind = 0;
uniform vec3 body : source_color = vec3(0.38, 0.40, 0.43);
uniform vec3 accent : source_color = vec3(0.25, 1.0, 0.55);

const vec3 HAZARD = vec3(0.85, 0.66, 0.12);
const vec3 DARK = vec3(0.10, 0.10, 0.11);

varying vec3 lp;
varying vec3 ln;

void vertex() {
	lp = VERTEX;
	ln = NORMAL;
}

float hash(float n) {
	return fract(sin(n * 127.1 + 3.7) * 43758.5453);
}

// 1 on every other diagonal band (bands `width` metres), softened over a pixel
float stripes(vec2 p, float width) {
	float s = (p.x + p.y) / width;
	float fw = max(fwidth(s), 1e-4);
	return clamp((abs(fract(s) - 0.5) - 0.25) / fw + 0.5, 0.0, 1.0);
}

// thin lines every `every` metres along `v`, faded far away
float lines(float v, float every, float width) {
	float s = v / every;
	float fw = max(fwidth(s), 1e-4);
	float d = min(fract(s), 1.0 - fract(s)) * every;
	return (1.0 - smoothstep(width, width + fw * every, d)) * (1.0 - smoothstep(0.2, 0.5, fw));
}

void fragment() {
	vec3 n = abs(ln);
	bool top = n.y > 0.5;
	// metres across the face from its left edge, and up from its bottom edge
	vec2 f = top ? lp.xz + size.xz * 0.5 : (n.x > 0.5 ? vec2(lp.z + size.z * 0.5, lp.y + size.y * 0.5) : vec2(lp.x + size.x * 0.5, lp.y + size.y * 0.5));
	float h = size.y;
	vec3 col = body;
	vec3 glow = vec3(0.0);
	float metal = 0.2;
	if (kind == 0) {
		// rack doors about 0.6 m wide (a whole number across each face) with a frame,
		// perforation, two to four status lights
		metal = 0.35;
		if (top) {
			col = body * 0.55;
		} else {
			float w = n.x > 0.5 ? size.z : size.x;
			float rack = w / max(1.0, round(w / 0.6));
			float i = floor(f.x / rack);
			vec2 q = vec2(f.x - i * rack, f.y);
			float frame = max(max(step(q.x, 0.03), step(rack - 0.03, q.x)), max(step(q.y, 0.05), step(h - 0.04, q.y)));
			vec2 d = q * 40.0;
			float fw = max(fwidth(d.x), fwidth(d.y));
			float hole = 1.0 - smoothstep(0.28 - fw, 0.28 + fw, length(fract(d) - 0.5));
			float dark = mix(0.25, hole, 1.0 - smoothstep(0.25, 0.6, fw)) * 0.45;
			col = mix(body * (1.0 - dark), body * 0.5, frame);
			for (int k = 0; k < 4; k++) {
				float r = hash(i * 7.0 + float(k) + floor(size.x * 10.0));
				float on = step(0.35, r) * step(0.12, fract(TIME * (0.1 + r * 0.3) + r));
				vec2 at = vec2(0.08 + float(k) * 0.035, h - 0.15);
				glow += accent * on * step(abs(q.x - at.x), 0.006) * step(abs(q.y - at.y), 0.006);
			}
		}
	} else if (kind == 1) {
		// slats 0.15 m, boards at the corners
		float slat = lines(top ? f.y : f.y, 0.15, 0.006);
		float w = top ? size.x : (n.x > 0.5 ? size.z : size.x);
		float corner = top ? 0.0 : max(step(f.x, 0.06), step(w - 0.06, f.x));
		col = mix(body, body * 0.6, max(slat, corner * 0.6));
	} else if (kind == 2) {
		// a machine housing: a band of vent slots, a hazard-striped base
		metal = 0.4;
		if (top) {
			col = body * 0.7;
		} else {
			float band = step(h * 0.45, f.y) * step(f.y, h * 0.8);
			float slot = lines(f.y, 0.04, 0.008) * band;
			col = mix(body, DARK, slot * 0.8);
			if (f.y < 0.12) {
				col = mix(DARK, HAZARD, stripes(f, 0.12));
			}
		}
	} else if (kind == 3) {
		// a barrier: hazard stripes all over its sides
		col = top ? body : mix(DARK, HAZARD, stripes(f, 0.18));
	} else if (kind == 4) {
		// a kerb or a plinth: a striped band round its top edge
		float edge = top ? min(min(f.x, size.x - f.x), min(f.y, size.z - f.y)) : h - f.y;
		if (edge < 0.08) {
			col = mix(DARK, HAZARD, stripes(top ? f : vec2(f.x, 0.0), 0.12));
		}
	} else {
		// a door panel: a frame, a darker kick plate, a status light
		metal = 0.5;
		float frame = top ? 0.0 : max(step(f.y, 0.03), step(h - 0.03, f.y));
		float kick = step(f.y, 0.25);
		col = mix(body, body * 0.6, max(frame, kick * 0.7));
		float w = n.x > 0.5 ? size.z : size.x;
		glow = top ? vec3(0.0) : accent * step(abs(f.x - w * 0.5), 0.015) * step(abs(f.y - 1.3), 0.015);
	}
	ALBEDO = col;
	EMISSION = glow * 3.0;
	METALLIC = metal;
	ROUGHNESS = 0.6;
}
