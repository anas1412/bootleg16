// Hazard stripes for the takeover bands: diagonal bars of `color`, `bar` px wide with the
// same gap, at -45 degrees.
shader_type canvas_item;

uniform vec4 color : source_color = vec4(1.0, 0.627, 0.0, 1.0);
uniform float bar = 40.0;
uniform vec2 size = vec2(1920.0, 160.0);

void fragment() {
	vec2 px = UV * size;
	float d = mod(px.x + px.y, bar * 2.0);
	// a pixel of soft edge so the bars don't stair-step
	float on = smoothstep(0.0, 1.0, d) * (1.0 - smoothstep(bar, bar + 1.0, d));
	COLOR = vec4(color.rgb, color.a * on);
}
