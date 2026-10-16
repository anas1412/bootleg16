// Studio model lighting as GoldSrc does it (studio.rs): an ambient level plus a shade level
// that falls off with GoldSrc's hemispherical lambert (`lambert` 1.5) away from the light,
// through the same light gamma as the lightmaps, times the light's colour.
import bevy_pbr::render::{
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::alpha_discard,
};

@if(PREPASS_PIPELINE)
import bevy_pbr::{
    prepass::io::{VertexOutput, FragmentOutput},
    deferred::functions::deferred_output,
};
@else
import bevy_pbr::render::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_functions::main_pass_post_lighting_processing,
};

struct Studio {
    // Which way the light travels (world, Y-up).
    dir: vec4<f32>,
    // Light colour, brightest channel 1.
    color: vec4<f32>,
    // x: ambient level, y: shade level (GoldSrc 0..255 units).
    levels: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100)
var<uniform> studio: Studio;

// GoldSrc's lightgammatable (brightness 1, gamma 2.5): the same curve as level.rs.
fn light_gamma(illum: f32) -> f32 {
    var f = illum / 255.0;
    let g3 = 0.05;
    if f > g3 {
        f = 0.125 + (f - g3) / (1.0 - g3) * 0.875;
    } else {
        f = f / g3 * 0.125;
    }
    return pow(f, 1.0 / 2.5);
}

@fragment
fn fragment(in: VertexOutput, @builtin(front_facing) is_front: bool) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(in, is_front);
    pbr_input.material.base_color = alpha_discard(pbr_input.material.flags, pbr_input.material.alpha_cutoff, pbr_input.material.base_color);
    var out: FragmentOutput;
@if(PREPASS_PIPELINE) {
    out = deferred_output(in, pbr_input);
} @else {
    // R_StudioLighting: full where the surface faces the light, down by the shade level
    // facing away (the 1.5 wrap keeps the back from going black).
    let n = normalize(in.world_normal);
    var lightcos = min(dot(n, studio.dir.xyz), 1.0);
    var illum = studio.levels.x + studio.levels.y;
    lightcos = (lightcos + 0.5) / 1.5;
    if lightcos > 0.0 {
        illum -= studio.levels.y * lightcos;
    }
    illum = clamp(illum, 0.0, 255.0);
    // Display-space light, back to linear for the multiply (the output encodes it again).
    let lv = light_gamma(illum) * studio.color.rgb;
    let lin = pow(lv, vec3<f32>(2.2));
    out.color = vec4<f32>(pbr_input.material.base_color.rgb * lin, pbr_input.material.base_color.a);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
}
    return out;
}
