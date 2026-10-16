// The HUD seen through a visor (visor.rs): the middle swells toward you, the corners stay
// pinned. Each pixel samples a little nearer the middle, less and less toward the corners.
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct Visor {
    bend: vec4<f32>,
}

@group(1) @binding(0) var<uniform> visor: Visor;
@group(1) @binding(1) var hud: texture_2d<f32>;
@group(1) @binding(2) var hud_sampler: sampler;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let q = in.uv * 2.0 - 1.0;
    let r2 = dot(q, q);
    let s = q * (1.0 - visor.bend.x * (1.0 - r2 * 0.5));
    let c = textureSample(hud, hud_sampler, s * 0.5 + 0.5);
    // the image holds colour already multiplied by alpha (UI blended over clear): undo it, the
    // pipeline blends again
    if (c.a <= 0.0) {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(c.rgb / c.a, c.a);
}
