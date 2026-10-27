// A data center's raised floor: tiles 32 units (0.81 m) on the map's own grid, so seams meet
// the walls and edges, with thin seams, one in four perforated (the cold
// air comes up through them). Sides of a floor (ledges, steps) are plain. Seams and holes fade
// out where they'd get finer than a pixel (fwidth), so far floors don't shimmer.
shader_type spatial;

uniform vec3 tile : source_color = vec3(0.46, 0.48, 0.50);
uniform vec3 seam : source_color = vec3(0.28, 0.30, 0.32);
uniform vec3 side : source_color = vec3(0.30, 0.32, 0.34);

const float TILE = 32.0 * 0.0254;

varying vec3 world_pos;
varying vec3 world_normal;

float hash(vec2 c) {
	return fract(sin(dot(c, vec2(127.1, 311.7))) * 43758.5453);
}

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
	world_normal = normalize((MODEL_MATRIX * vec4(NORMAL, 0.0)).xyz);
}

void fragment() {
	vec2 p = world_pos.xz / TILE;
	vec2 cell = floor(p);
	vec2 f = fract(p);
	vec2 w = fwidth(p);
	vec2 e = min(f, 1.0 - f) / max(w, vec2(1e-4));
	float line = (1.0 - min(min(e.x, e.y), 1.0)) * (1.0 - smoothstep(0.08, 0.25, max(w.x, w.y)));
	// perforated tiles: a 12 x 12 grid of holes inside a margin
	vec2 d = f * 12.0;
	float fw = max(fwidth(d.x), fwidth(d.y));
	float inside = step(0.08, f.x) * step(f.x, 0.92) * step(0.08, f.y) * step(f.y, 0.92);
	float hole = 1.0 - smoothstep(0.25 - fw, 0.25 + fw, length(fract(d) - 0.5));
	float near = 1.0 - smoothstep(0.25, 0.6, fw);
	float perf = step(hash(cell), 0.25) * inside * mix(0.2, hole, near);
	vec3 top = mix(tile * (1.0 - perf * 0.5), seam, line);
	ALBEDO = abs(world_normal.y) > 0.5 ? top : side;
	ROUGHNESS = 0.8;
}
