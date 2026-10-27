// A Quaternius base body dressed in its side's clothes without touching its files: figure.gd
// paints each vertex's zone into COLOR.r (worked out on the T-pose), and this draws the skin
// texture where there's skin and plain cloth everywhere else.
// Zones: 0 skin, 1 top, 2 trousers, 3 shoes, 4 shirt (the Bureau's V under the jacket),
// 5 the side's stripe down the Strays' sleeves. Cloth stands a little off the body, so it
// reads as clothes, not paint.
shader_type spatial;

uniform sampler2D skin_tex : source_color, filter_linear_mipmap;
uniform sampler2D normal_tex : hint_normal, filter_linear_mipmap;
uniform sampler2D rough_tex : hint_roughness_r, filter_linear_mipmap;
uniform vec3 top : source_color = vec3(0.07, 0.075, 0.085);
uniform vec3 trousers : source_color = vec3(0.07, 0.075, 0.085);
uniform vec3 shoes : source_color = vec3(0.03, 0.03, 0.035);
uniform vec3 shirt : source_color = vec3(0.78, 0.8, 0.82);
uniform vec3 stripe : source_color = vec3(1.0, 0.31, 0.55);
uniform float cloth_rough = 0.85;
uniform float loose = 0.012;  // metres the cloth stands off the skin

varying flat float zone;

void vertex() {
	zone = COLOR.r * 8.0;
	if (zone > 0.5) {
		VERTEX += NORMAL * loose;
	}
}

void fragment() {
	int z = int(round(zone));
	vec3 n = texture(normal_tex, UV).rgb;
	if (z == 0) {
		ALBEDO = texture(skin_tex, UV).rgb;
		ROUGHNESS = texture(rough_tex, UV).r;
		NORMAL_MAP = n;
	} else {
		vec3 c = z == 1 ? top : z == 2 ? trousers : z == 3 ? shoes : z == 4 ? shirt : stripe;
		ALBEDO = c;
		ROUGHNESS = z == 3 ? 0.45 : cloth_rough;
		// cloth hides the body's muscle detail but keeps a little of its form
		NORMAL_MAP = mix(vec3(0.5, 0.5, 1.0), n, 0.15);
		if (z == 5) {
			EMISSION = c * 0.6;
		}
	}
}
