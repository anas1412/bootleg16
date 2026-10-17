//! Bullet tracers, in one place. A shot (yours, predicted in `game`; anyone else's, from the
//! server in `effects`) calls `Tracers::shot` with where the muzzle is and where the shot ended;
//! a short bright streak flies that way. A shot that ends far off (the sky) is drawn only for
//! its first stretch and fades, so it never races away across the map.
use crate::map::to_y_up;
use bevy::gizmos::config::{GizmoConfig, GizmoConfigGroup, GizmoLineConfig};
use bevy::prelude::*;

/// How far a tracer reaches (units), how fast it flies (units a second), and how long it stays
/// joined to the gun before the tail leaves (seconds). A shot that ends farther off is drawn
/// only this far, fading, so it never races across the map.
const REACH: f32 = 1400.0;
const SPEED: f32 = 30000.0;
const HOLD: f32 = 0.03;

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
        let (to, fades) = if len > REACH { (a + (b - a) / len * REACH, true) } else { (b, false) };
        self.0.push(Tracer { from: a, to, age: 0.0, fades });
    }
}

/// The streak at `age` on a path of `len`: (tail, head, fade), distances from the muzzle; None
/// once it is gone. The head shoots out of the gun, the tail stays a moment, then follows.
fn streak(age: f32, len: f32, fades: bool) -> Option<(f32, f32, f32)> {
    let head = (age * SPEED).min(len);
    let tail = ((age - HOLD).max(0.0) * SPEED).min(head);
    if tail >= len {
        return None;
    }
    Some((tail, head, if fades { (1.0 - tail / len).clamp(0.0, 1.0) } else { 1.0 }))
}

fn draw(mut tracers: ResMut<Tracers>, time: Res<Time>, mut gizmos: Gizmos<TracerGizmos>, mut hold: Local<Option<bool>>) {
    // Screenshot runs hold a tracer a moment after it leaves the gun (it lives under a tenth of a second).
    let hold = *hold.get_or_insert_with(|| std::env::var_os("BOOTLEG16_SCREENSHOT").is_some());
    let dt = if hold { 0.0 } else { time.delta_secs() };
    for t in &mut tracers.0 {
        let len = t.from.distance(t.to).max(1.0);
        let age = if hold { 0.03 } else { t.age };
        let Some((tail, head, fade)) = streak(age, len, t.fades) else { continue };
        let (a, b) = (t.from.lerp(t.to, tail / len), t.from.lerp(t.to, head / len));
        // A warm line with a white core.
        gizmos.line(a, b, Color::srgba(1.0, 0.75, 0.3, 0.85 * fade));
        gizmos.line(a.lerp(b, 0.25), b, Color::srgba(1.0, 1.0, 0.9, fade));
        t.age += dt;
    }
    tracers.0.retain(|t| hold || streak(t.age, t.from.distance(t.to).max(1.0), t.fades).is_some());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tracer_is_gone_within_a_tenth_of_a_second_whatever_its_length() {
        for len in [1.0, 50.0, 400.0, REACH] {
            assert!(streak(0.0, len, false).is_some(), "it starts at the gun ({len})");
            assert!(streak(0.1, len, len >= REACH).is_none(), "gone after 0.1 s ({len})");
        }
    }

    #[test]
    fn the_head_leaves_the_gun_first_and_the_tail_follows() {
        let (tail, head, _) = streak(0.01, 1000.0, false).unwrap();
        assert_eq!(tail, 0.0);
        assert!((head - 300.0).abs() < 1.0);
        let (tail, _, fade) = streak(0.05, 1400.0, true).unwrap();
        assert!(tail > 0.0 && fade < 1.0);
    }
}
