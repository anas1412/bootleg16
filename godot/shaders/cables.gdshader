// Cable bundles run along the crawlspace walls: rows of round cables (shaded across their
// width), dark and grey jackets with coloured ones between. The panel's size (metres) comes per
// instance; the rows fade to their average far away (fwidth).
shader_type spatial;

instance uniform vec2 size = vec2(1.0, 1.0);
uniform vec3 jacket : source_color = vec3(0.12, 0.13, 0.14);
uniform vec3 yellow : source_color = vec3(0.75, 0.62, 0.18);
uniform vec3 blue : source_color = vec3(0.18, 0.38, 0.62);
uniform vec3 grey : source_color = vec3(0.42, 0.44, 0.46);
uniform vec3 orange : source_color = vec3(0.70, 0.36, 0.14);

const float CABLE = 0.05;

float hash(float n) {
	return fract(sin(n * 91.7 + 1.3) * 43758.5453);
}

void fragment() {
	float y = (1.0 - UV.y) * size.y / CABLE;
	float row = floor(y);
	float across = fract(y);
	float h = hash(row);
	vec3 col = h > 0.9 ? yellow : (h > 0.8 ? blue : (h > 0.72 ? orange : (h > 0.5 ? grey : jacket * (0.8 + h * 0.6))));
	float round_ = sin(across * 3.14159);
	float near = 1.0 - smoothstep(0.3, 0.8, fwidth(y));
	ALBEDO = col * mix(0.75, 0.35 + 0.65 * round_, near);
	ROUGHNESS = 0.7;
}
