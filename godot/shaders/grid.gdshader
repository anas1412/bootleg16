// Grey with a 1 m grid in world space, so distance, height and speed read at a glance. Lines
// keep about a pixel's width on screen and fade out where they'd get finer than a pixel
// (fwidth), so far surfaces don't shimmer or break into dots.
shader_type spatial;

uniform vec3 base : source_color = vec3(0.42, 0.44, 0.47);
uniform vec3 line : source_color = vec3(0.26, 0.28, 0.31);

varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

float grid(vec2 p) {
	vec2 w = fwidth(p);
	vec2 d = abs(fract(p - 0.5) - 0.5) / max(w, vec2(1e-4));
	float l = 1.0 - min(min(d.x, d.y), 1.0);
	// cells smaller than a few pixels: the grid fades out instead of turning to noise
	return l * (1.0 - smoothstep(0.08, 0.25, max(w.x, w.y)));
}

void fragment() {
	vec3 n = abs(NORMAL * mat3(VIEW_MATRIX));
	// the two axes along this face (not the one it faces)
	vec2 p = n.x > 0.5 ? world_pos.zy : (n.y > 0.5 ? world_pos.xz : world_pos.xy);
	ALBEDO = mix(base, line, grid(p));
	ROUGHNESS = 0.9;
}
