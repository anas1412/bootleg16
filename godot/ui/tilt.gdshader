// A HUD group on the visor: the visor curves round your eyes, so a group near a side of the
// screen turns in towards you, its outer edge nearer (taller) than its inner one. Draws the
// group's own viewport with its far edge shrunk about the group's middle.
shader_type canvas_item;

// > 0: the left edge is near and the right edge is this much smaller (a group on the left);
// < 0: the mirror (a group on the right).
uniform float tilt_x = 0.0;
// > 0: the top edge is near and the bottom one smaller (the group at the top centre).
uniform float tilt_y = 0.0;

void fragment() {
	vec2 uv = UV;
	float sy = 1.0 - (tilt_x > 0.0 ? tilt_x * uv.x : -tilt_x * (1.0 - uv.x));
	float sx = 1.0 - (tilt_y > 0.0 ? tilt_y * uv.y : -tilt_y * (1.0 - uv.y));
	vec2 src = vec2(0.5 + (uv.x - 0.5) / sx, 0.5 + (uv.y - 0.5) / sy);
	if (src.x < 0.0 || src.x > 1.0 || src.y < 0.0 || src.y > 1.0) {
		COLOR = vec4(0.0);
	} else {
		COLOR = texture(TEXTURE, src);
	}
}
