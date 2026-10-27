// The gun in your hands: drawn as if it sat just past the camera's near plane, so it never
// goes into a wall you stand against, while its own parts still hide each other in order.
shader_type spatial;
render_mode cull_back, shadows_disabled;

uniform vec4 albedo : source_color = vec4(0.12, 0.13, 0.15, 1.0);
uniform float roughness = 0.55;
uniform float metallic = 0.3;
// A strip lit in your side's colour.
uniform vec4 glow : source_color = vec4(0.0);

void vertex() {
	POSITION = PROJECTION_MATRIX * MODELVIEW_MATRIX * vec4(VERTEX, 1.0);
	// reversed depth: 1 is the near plane; squeeze everything towards it, keeping the order
	POSITION.z = mix(POSITION.w, POSITION.z, 0.05);
}

void fragment() {
	ALBEDO = albedo.rgb;
	ROUGHNESS = roughness;
	METALLIC = metallic;
	EMISSION = glow.rgb * glow.a;
}
