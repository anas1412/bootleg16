//! Bullet tracers, in one place. A shot (yours, predicted in `game`; anyone else's, from the
//! server in `effects`) calls `Tracers::shot` with where the muzzle is and where the shot ended;
//! a short bright streak flies that way. A shot that ends far off (the sky) is drawn only for
//! its first stretch and fades, so it never races away across the map.
use crate::map::to_y_up;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::prelude::*;

/// The longest path drawn (units), the streak's length, and how fast it flies (units a second).
const RANGE: f32 = 2200.0;
const STREAK: f32 = 150.0;
const SPEED: f32 = 14000.0;

#[derive(Default, Reflect, GizmoConfigGroup)]
struct TracerGizmos;

struct Tracer {
    from: Vec3,
    to: Vec3,
    age: f32,
    /// Cut short (the shot went on): fades out instead of ending in a hit.
    fades: bool,
}

#[derive(Resource, Default)]
pub struct Tracers(Vec<Tracer>);

impl Tracers {
    /// A shot from the muzzle to where it ended (GoldSrc coordinates).
    pub fn shot(&mut self, from: [f32; 3], to: [f32; 3]) {
        let (a, b) = (Vec3::from(to_y_up(from)), Vec3::from(to_y_up(to)));
        let len = a.distance(b);
        let (to, fades) = if len > RANGE { (a + (b - a) / len * RANGE, true) } else { (b, false) };
        self.0.push(Tracer { from: a, to, age: 0.0, fades });
    }
}

fn draw(mut tracers: ResMut<Tracers>, time: Res<Time>, mut gizmos: Gizmos<TracerGizmos>, mut hold: Local<Option<bool>>) {
    // Screenshot runs hold a streak in mid-flight (it lives a tenth of a second).
    let hold = *hold.get_or_insert_with(|| std::env::var_os("BOOTLEG16_SCREENSHOT").is_some());
    let dt = if hold { 0.0 } else { time.delta_secs() };
    for t in &mut tracers.0 {
        let len = t.from.distance(t.to).max(1.0);
        let head = if hold { (len * 0.5).min(900.0) } else { (t.age * SPEED).min(len) };
        let tail = (head - STREAK).max(0.0);
        let fade = if t.fades { (1.0 - head / len).clamp(0.0, 1.0) } else { 1.0 };
        let (a, b) = (t.from.lerp(t.to, tail / len), t.from.lerp(t.to, head / len));
        // A warm streak with a white core.
        gizmos.line(a, b, Color::srgba(1.0, 0.75, 0.3, 0.9 * fade));
        gizmos.line(a.lerp(b, 0.4), b, Color::srgba(1.0, 1.0, 0.9, fade));
        t.age += dt;
    }
    tracers.0.retain(|t| hold || t.age * SPEED < t.from.distance(t.to) + STREAK);
}

pub struct TracerPlugin;

impl Plugin for TracerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tracers>()
            .init_gizmo_group::<TracerGizmos>()
            .insert_gizmo_config(TracerGizmos, GizmoConfig { line: GizmoLineConfig { width: 3.0, ..default() }, ..default() })
            .add_systems(Update, draw);
    }
}
