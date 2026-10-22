// A burst of bullet-hit bits (impacts.rs): dust, blood, sparks. One shared mesh of small
// spheres, one per bit; this shader flies each from the burst's origin (out along the burst's
// direction with its own jitter, falling at sv_gravity 800) and shrinks it as its life runs
// out, from the time alone, so the CPU never touches a bit after the hit. The colour is the
// standard material's.
//   uv_b.x  which bit of the burst this vertex belongs to
import bevy_pbr::render::{
    forward_io::{Vertex, VertexOutput},
    mesh_view_bindings::globals,
    view_transformations::position_world_to_clip,
};

struct Burst {
    // xyz where (world, Y-up), w when (globals.time, wraps hourly)
    origin: vec4<f32>,
    // xyz which way (its length weighs it against the jitter), w how fast
    dir: vec4<f32>,
    // x size, y life (s), z 1 if it falls, w how many bits
    look: vec4<f32>,
    // x the noise salt (the hit's time)
    salt: vec4<f32>,
}

@group(constants::MATERIAL_BIND_GROUP) @binding(100) var<uniform> burst: Burst;

// The same tiny noise as impacts.rs `noise`.
fn noise(i: f32, salt: f32) -> f32 {
    return fract(sin(i * 12.9898 + salt * 78.233) * 43758.547);
}

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    let i = v.uv_b.x;
    let salt = burst.salt.x;
    var t = globals.time - burst.origin.w;
    if t < 0.0 {
        t += 3600.0;
    }
    let life = burst.look.y;
    var scale = 0.0;
    if i < burst.look.w && t < life {
        scale = burst.look.x * sqrt((life - t) / life);
    }
    let jitter = vec3<f32>(noise(i, salt) - 0.5, noise(i + 31.0, salt) - 0.5, noise(i + 77.0, salt) - 0.5);
    let d = burst.dir.xyz + jitter * 1.2;
    let vel = select(vec3<f32>(0.0), normalize(d), dot(d, d) > 0.0) * burst.dir.w * (0.5 + noise(i + 5.0, salt));
    var at = burst.origin.xyz + vel * t;
    at.y -= 400.0 * burst.look.z * t * t;
    let world = at + v.position * scale;
    var out: VertexOutput;
    out.position = position_world_to_clip(world);
    out.world_position = vec4<f32>(world, 1.0);
    out.world_normal = v.normal;
    out.uv = v.uv;
    out.uv_b = v.uv_b;
@if(VERTEX_OUTPUT_INSTANCE_INDEX)
    out.instance_index = v.instance_index;
    return out;
}
