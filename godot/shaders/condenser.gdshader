// Cooling condensers along an outdoor wall: housings 1.2 m wide, each with a round fan grille
// (rings, and blades turning slowly behind them). The panel's size (metres) comes per instance.
shader_type spatial;

instance uniform vec2 size = vec2(1.2, 1.2);
uniform vec3 housing : source_color = vec3(0.52, 0.54, 0.55);
uniform vec3 grille : source_color = vec3(0.18, 0.19, 0.20);

const float UNIT = 1.2;

void fragment() {
	vec2 p = vec2(UV.x * size.x, (1.0 - UV.y) * size.y);
	float i = floor(p.x / UNIT);
	vec2 q = vec2(p.x - i * UNIT, p.y) - vec2(UNIT * 0.5, size.y * 0.5);
	float r = length(q);
	float radius = min(UNIT, size.y) * 0.4;
	float fw = fwidth(r * 30.0);
	float rings = 1.0 - smoothstep(0.15 - fw, 0.15 + fw, abs(fract(r * 30.0) - 0.5));
	float a = atan(q.y, q.x) + TIME * (0.6 + fract(i * 0.37));
	float blades = smoothstep(0.55, 0.75, abs(sin(a * 2.0)));
	float fan = step(r, radius);
	float seam = step(abs(p.x - (i + 1.0) * UNIT), 0.01) + step(abs(p.x - i * UNIT), 0.01);
	vec3 col = mix(housing, grille * (1.0 + blades * 0.8) * (1.0 - rings * 0.4), fan);
	ALBEDO = mix(col, grille, min(seam, 1.0));
	ROUGHNESS = 0.6;
	METALLIC = 0.4;
}
