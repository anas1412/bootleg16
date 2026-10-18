//! Bullet tracers, as CS 1.6 draws them (`EV_HLDM_CheckTracer` in client.so, read from the
//! disassembly): the tracer starts at the shooter's eye + 2 units forward + 16 to the right +
//! (0, 0, -4), not at the gun model, and flies straight to where the bullet ended; the engine's
//! tracer is a short bright dash at 6000 units a second. Your shots (predicted in `game`) and
//! everyone else's (from the server in `effects`) both call `Tracers::shot`.
use crate::map::to_y_up;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::prelude::*;

/// Dash speed (the engine's `tracerspeed`), dash length, and the longest shot that gets a tracer
/// (units; one that ends farther, or in the sky, has none: it would stop in mid air).
const SPEED: f32 = 6000.0;
const DASH: f32 = 60.0;
const REACH: f32 = 4096.0;
/// The first stretch of a path is not drawn, and a shot that ends inside it has no tracer: a
/// line that close to the camera is stretched across the screen by perspective, and how far
/// the wall is is what makes it depend on where you stand.
const NEAR: f32 = 64.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct TracerGizmos;

struct Tracer {
    from: Vec3,
    to: Vec3,
    age: f32,
}

#[derive(Resource, Default)]
pub struct Tracers(Vec<Tracer>);

/// Where a player's tracer starts (GoldSrc coordinates), CS's offsets from the eye along the
/// shot: 2 forward, 16 right, 4 down.
pub fn start(eye: [f32; 3], end: [f32; 3]) -> [f32; 3] {
    let d = [end[0] - eye[0], end[1] - eye[1], end[2] - eye[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
    let f = [d[0] / len, d[1] / len, d[2] / len];
    // right = forward x up (GoldSrc: z is up), flat
    let flat = (f[0] * f[0] + f[1] * f[1]).sqrt().max(1e-3);
    let right = [f[1] / flat, -f[0] / flat, 0.0];
    [eye[0] + 2.0 * f[0] + 16.0 * right[0], eye[1] + 2.0 * f[1] + 16.0 * right[1], eye[2] + 2.0 * f[2] - 4.0]
}

impl Tracers {
    /// A shot by someone whose eye is at `eye` (GoldSrc coordinates), ending at `end`.
    pub fn shot(&mut self, eye: [f32; 3], end: [f32; 3]) {
        let (a, b) = (Vec3::from(to_y_up(start(eye, end))), Vec3::from(to_y_up(end)));
        let len = a.distance(b);
        // Too close: stretched across the screen. Too far: it would stop in mid air, not on a surface.
        if len < NEAR * 2.0 || len > REACH {
            return;
        }
        let dir = (b - a) / len;
        let to = b;
        self.0.push(Tracer { from: a + dir * NEAR, to, age: 0.0 });
    }
}

/// The dash at `age` on a path of `len`: (tail, head) distances from the start; None once it
/// has arrived.
fn dash(age: f32, len: f32) -> Option<(f32, f32)> {
    let head = (age * SPEED).min(len);
    let tail = (head - DASH).max(0.0);
    (age * SPEED - DASH < len).then_some((tail, head))
}

/// Whether a dash is drawn: not while it is still coming out of the start (a short dash right
/// next to the camera is stretched by perspective into a long false line, and a dash with no
/// length at all has no direction); once whole, and while it arrives at the end.
fn shown(tail: f32, head: f32, len: f32) -> bool {
    head - tail >= DASH - 0.5 || (head >= len - 0.01 && head - tail >= 2.0)
}

fn draw(mut tracers: ResMut<Tracers>, time: Res<Time>, mut gizmos: Gizmos<TracerGizmos>, mut hold: Local<Option<bool>>) {
    // Screenshot runs hold the latest tracer part-way (they live a fraction of a second).
    let hold = *hold.get_or_insert_with(|| std::env::var_os("BOOTLEG16_SCREENSHOT").is_some());
    if hold && tracers.0.len() > 1 {
        let last = tracers.0.pop().unwrap();
        tracers.0 = vec![last];
    }
    let dt = if hold { 0.0 } else { time.delta_secs() };
    for t in &mut tracers.0 {
        let len = t.from.distance(t.to).max(1.0);
        let age = if hold { (len * 0.5 / SPEED).min(0.05) } else { t.age };
        if let Some((tail, head)) = dash(age, len).filter(|&(tail, head)| shown(tail, head, len)) {
            let (a, b) = (t.from.lerp(t.to, tail / len), t.from.lerp(t.to, head / len));
            // the engine's tracer colour: warm yellow
            gizmos.line(a, b, Color::srgba(1.0, 0.85, 0.4, 0.9));
        }
        t.age += dt;
    }
    tracers.0.retain(|t| hold || dash(t.age, t.from.distance(t.to).max(1.0)).is_some());
}

pub struct TracerPlugin;

impl Plugin for TracerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tracers>()
            .init_gizmo_group::<TracerGizmos>()
            .insert_gizmo_config(TracerGizmos, GizmoConfig { line: GizmoLineConfig { width: 2.5, ..default() }, ..default() })
            .add_systems(Update, draw);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_cs_offsets_from_the_eye_along_the_shot() {
        // Shooting along +x from the origin: 2 ahead, 16 to the right (-y in GoldSrc), 4 down.
        let s = start([0.0, 0.0, 0.0], [1000.0, 0.0, 0.0]);
        assert!((s[0] - 2.0).abs() < 1e-4 && (s[1] + 16.0).abs() < 1e-4 && (s[2] + 4.0).abs() < 1e-4, "{s:?}");
        // Along +y the right-hand side is +x.
        let s = start([0.0, 0.0, 0.0], [0.0, 1000.0, 0.0]);
        assert!((s[0] - 16.0).abs() < 1e-4 && (s[1] - 2.0).abs() < 1e-4, "{s:?}");
    }

    #[test]
    fn nothing_is_drawn_close_to_the_camera() {
        let mut t = Tracers::default();
        t.shot([0.0; 3], [100.0, 0.0, 0.0]);
        assert!(t.0.is_empty(), "a wall right in front has no tracer");
        t.shot([0.0; 3], [1000.0, 0.0, 0.0]);
        assert!(t.0[0].from.distance(Vec3::from(to_y_up(start([0.0; 3], [1000.0, 0.0, 0.0])))) > NEAR - 0.5);
    }

    #[test]
    fn a_dash_is_not_drawn_while_it_is_coming_out_of_the_start() {
        for age in [0.0, 0.0005, 0.002, 0.005, 0.0099] {
            let (tail, head) = dash(age, 1000.0).unwrap();
            assert!(!shown(tail, head, 1000.0), "partial dash drawn at {age}: {tail}..{head}");
        }
        let (tail, head) = dash(0.02, 1000.0).unwrap();
        assert!(shown(tail, head, 1000.0), "the whole dash is drawn");
        // arriving at the end it shrinks, and is still drawn until it is almost gone
        let (tail, head) = dash(1000.0 / SPEED + 0.004, 1000.0).unwrap();
        assert!(shown(tail, head, 1000.0));
    }

    #[test]
    fn the_dash_flies_from_the_start_to_the_end_and_is_gone() {
        assert!(dash(0.0, 1000.0).is_some());
        let (tail, head) = dash(0.05, 1000.0).unwrap();
        assert!((head - 300.0).abs() < 0.5 && (head - tail - DASH).abs() < 0.5);
        assert!(dash(1000.0 / SPEED + 0.02, 1000.0).is_none());
    }
}
