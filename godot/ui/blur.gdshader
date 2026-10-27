// The soft blur behind a menu or popup: what's under it, blurred and dimmed by `amount`
// (0 is untouched). Tweened in when the menu opens and out when it closes.
shader_type canvas_item;

uniform sampler2D screen : hint_screen_texture, filter_linear_mipmap;
uniform float amount = 0.0;
uniform float max_lod = 4.0;
uniform vec4 dim : source_color = vec4(0.02, 0.024, 0.031, 0.7);

void fragment() {
	vec3 c = textureLod(screen, SCREEN_UV, amount * max_lod).rgb;
	COLOR = vec4(mix(c, dim.rgb, dim.a * amount), 1.0);
}
