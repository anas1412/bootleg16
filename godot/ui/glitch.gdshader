// The Interference glitch: splits the colour channels of what's under it sideways and nudges
// it, by `amount` (0 is clean). Used only for a moment: on arrival, hover start, a change of
// state or an alert, then tweened back to 0 and removed.
shader_type canvas_item;

uniform sampler2D screen : hint_screen_texture, filter_linear_mipmap;
uniform float amount = 0.0;
uniform float split_px = 4.0;

void fragment() {
	// four steps of a sideways nudge as amount runs down, like a tuning signal
	float step_x = (mod(floor(amount * 4.0), 2.0) * 2.0 - 1.0) * 2.0 * amount;
	vec2 nudge = vec2(step_x, 0.0) * SCREEN_PIXEL_SIZE;
	vec2 split = vec2(split_px * amount, 0.0) * SCREEN_PIXEL_SIZE;
	vec4 c = texture(screen, SCREEN_UV + nudge);
	float r = texture(screen, SCREEN_UV + nudge - split).r;
	float b = texture(screen, SCREEN_UV + nudge + split).b;
	COLOR = vec4(r, c.g, b, 1.0);
}
