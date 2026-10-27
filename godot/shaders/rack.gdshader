// Server rack fronts along a wall: doors 0.6 m wide with a perforated mesh, a frame, a few
// small status lights near the top. Kept mid-dark and quiet so players stand out against it.
// Doors are stretched a little so a whole number of them fills the panel (no half doors).
// The panel's size (metres) comes per instance; the perforation fades out where it would
// shimmer (fwidth), as the grid does.
shader_type spatial;

instance uniform vec2 size = vec2(1.0, 2.0);
uniform vec3 frame_color : source_color = vec3(0.20, 0.21, 0.23);
uniform vec3 door_color : source_color = vec3(0.38, 0.40, 0.43);
uniform vec3 led_green : source_color = vec3(0.25, 1.0, 0.55);
uniform vec3 led_blue : source_color = vec3(0.35, 0.7, 1.0);

float hash(float n) {
	return fract(sin(n * 127.1 + 3.7) * 43758.5453);
}

void fragment() {
	// metres along the panel, up from its bottom
	vec2 p = vec2(UV.x * size.x, (1.0 - UV.y) * size.y);
	float rack = size.x / max(1.0, round(size.x / 0.6));
	float i = min(floor(p.x / rack), round(size.x / rack) - 1.0);
	vec2 q = vec2(p.x - i * rack, p.y);
	float frame = max(max(step(q.x, 0.03), step(rack - 0.03, q.x)), max(step(q.y, 0.07), step(size.y - 0.05, q.y)));
	// the perforated door: holes 2.5 cm apart, faded to their average far away
	vec2 d = q * 40.0;
	float fw = max(fwidth(d.x), fwidth(d.y));
	float hole = 1.0 - smoothstep(0.28 - fw, 0.28 + fw, length(fract(d) - 0.5));
	float near = 1.0 - smoothstep(0.25, 0.6, fw);
	float dark = mix(0.25, hole, near) * 0.45;
	// a handle on each door
	float handle = step(abs(q.x - (rack - 0.07)), 0.008) * step(abs(q.y - 1.05), 0.12);
	vec3 col = mix(door_color * (1.0 - dark), frame_color, max(frame, handle));
	// two to four status lights per rack, some blinking slowly
	vec3 glow = vec3(0.0);
	for (int k = 0; k < 4; k++) {
		float h = hash(i * 7.0 + float(k));
		vec2 at = vec2(0.08 + float(k) * 0.035, size.y - 0.22);
		float on = step(0.35, h) * step(0.12, fract(TIME * (0.1 + h * 0.3) + h));
		float led = step(abs(q.x - at.x), 0.006) * step(abs(q.y - at.y), 0.006) * on;
		glow += led * (h > 0.7 ? led_blue : led_green);
	}
	ALBEDO = col;
	EMISSION = glow * 3.0;
	ROUGHNESS = 0.55;
	METALLIC = 0.35;
}
