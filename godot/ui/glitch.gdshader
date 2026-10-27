// The Interference glitch, over whatever is under it, by `amount` (0 is clean):
// a cyan ghost of the content to one side and a pink one to the other (the two sides' channel
// split), a few horizontal slices torn sideways, and a stepped nudge. Used only for a moment
// (arrival, hover start, a change of state, an alert), tweened back to 0 and removed.
shader_type canvas_item;

uniform sampler2D screen : hint_screen_texture, filter_linear_mipmap;
uniform float amount = 0.0;
uniform float split_px = 4.0;
uniform vec4 left_color : source_color = vec4(0.286, 0.776, 1.0, 1.0);   // Bureau cyan
uniform vec4 right_color : source_color = vec4(1.0, 0.31, 0.545, 1.0);   // Strays pink
uniform float seed = 0.0;
// how tall each torn slice is, and how far the worst one moves (px)
uniform float slice_px = 6.0;
uniform float tear_px = 10.0;

float luma(vec3 c) {
	return dot(c, vec3(0.299, 0.587, 0.114));
}

float hash(float n) {
	return fract(sin(n * 12.9898 + seed * 78.233) * 43758.5453);
}

void fragment() {
	vec2 px = SCREEN_PIXEL_SIZE;
	// about one slice in four tears, by a different amount each, per glitch
	float slice = floor(FRAGCOORD.y / slice_px);
	float h = hash(slice);
	float tear = h > 0.75 ? (hash(slice + 7.0) * 2.0 - 1.0) * tear_px : 0.0;
	// four steps of a sideways nudge as it settles, like a signal tuning in
	float nudge = (mod(floor(amount * 4.0), 2.0) * 2.0 - 1.0) * 2.0;
	vec2 at = SCREEN_UV + vec2((tear + nudge) * amount, 0.0) * px;
	vec2 split = vec2(split_px * amount, 0.0) * px;

	vec3 base = texture(screen, at).rgb;
	float here = luma(base);
	// ghosts: where the content shifted left / right is brighter than what's here
	float left = max(luma(texture(screen, at + split).rgb) - here, 0.0);
	float right = max(luma(texture(screen, at - split).rgb) - here, 0.0);
	vec3 c = base + left_color.rgb * left + right_color.rgb * right;
	COLOR = vec4(min(c, vec3(1.0)), 1.0);
}
