// Grey with a 1 m grid in world space, so distance, height and speed read at a glance.
shader_type spatial;

uniform vec3 base : source_color = vec3(0.42, 0.44, 0.47);
uniform vec3 line : source_color = vec3(0.26, 0.28, 0.31);

varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec3 cell = abs(fract(world_pos + 0.5) - 0.5);
	vec3 n = abs(NORMAL);
	// the two axes along this face (not the one it faces)
	float near = n.x > 0.5 ? min(cell.y, cell.z) : (n.y > 0.5 ? min(cell.x, cell.z) : min(cell.x, cell.y));
	ALBEDO = mix(line, base, smoothstep(0.0, 0.02, near));
	ROUGHNESS = 0.9;
}
