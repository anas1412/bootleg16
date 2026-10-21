//! Walking bob and stair smoothing, as CS 1.6 does them (`V_CalcBob` and `V_CalcNormalRefdef`
//! in client.so, read with REA). The bob follows your horizontal speed on a 0.8 s cycle (rising
//! for the first half), between -7 and 4 units: the camera moves up and down by it, the gun
//! moves forward by 0.4 of it and turns a little (yaw by half of it, roll by all of it, in
//! degrees). Stepping up a stair, the view lags behind and catches up at 150 units a second
//! (never more than 18 behind), so stairs glide instead of jerking. Only what you see moves:
//! your shots still come from your eye.
use crate::game::{FlyCam, Frame, Walker};
use crate::viewmodel::ViewModelCamera;
use bevy::prelude::*;

/// `cl_bobcycle`, `cl_bobup`, `cl_bob` (client.so defaults) and the bob's limits.
const CYCLE: f32 = 0.8;
const UP: f32 = 0.5;
const AMOUNT: f32 = 0.01;
const LIMITS: (f32, f32) = (-7.0, 4.0);
/// Stair smoothing: how fast the view catches up, and how far it may lag.
const STEP_SPEED: f32 = 150.0;
const STEP_MAX: f32 = 18.0;

pub struct ViewBobPlugin;

impl Plugin for ViewBobPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bob>().add_systems(Update, apply.after(crate::game::move_camera).in_set(Frame::Camera));
    }
}

#[derive(Resource, Default)]
struct Bob {
    time: f32,
    bob: f32,
    /// Where the view is on the way up a stair (GoldSrc z).
    view_z: Option<f32>,
}

/// The bob for a horizontal speed at a point in the cycle (`V_CalcBob`).
fn bob_at(time: f32, speed: f32) -> f32 {
    let cycle = (time % CYCLE) / CYCLE;
    let phase = if cycle < UP { std::f32::consts::PI * cycle / UP } else { std::f32::consts::PI + std::f32::consts::PI * (cycle - UP) / (1.0 - UP) };
    let b = speed * AMOUNT;
    (b * 0.3 + b * 0.7 * phase.sin()).clamp(LIMITS.0, LIMITS.1)
}

/// The view's lag behind a player who went up to `z` (`V_CalcNormalRefdef`'s step smoothing):
/// the new view z, never below `z - STEP_MAX`.
fn step_up(view_z: f32, z: f32, dt: f32) -> f32 {
    (view_z + dt * STEP_SPEED).min(z).max(z - STEP_MAX)
}

fn apply(time: Res<Time>, w: Res<Walker>, mut bob: ResMut<Bob>, mut cam: Single<&mut Transform, (With<FlyCam>, Without<ViewModelCamera>)>, mut gun: Single<&mut Transform, (With<ViewModelCamera>, Without<FlyCam>)>) {
    let own_view = w.client.id().is_some() && w.client.vitals.alive() && !w.noclip && !crate::spectate::spectating(&w) && std::env::var_os("BOOTLEG16_CAM").is_none();
    if !own_view {
        *bob = Bob::default();
        **gun = Transform::default();
        return;
    }
    let me = w.client.me();
    let dt = time.delta_secs();
    if me.on_ground {
        bob.time += dt;
        bob.bob = bob_at(bob.time, (me.velocity[0] * me.velocity[0] + me.velocity[1] * me.velocity[1]).sqrt());
    }
    let z = me.origin[2];
    let view_z = match bob.view_z {
        Some(v) if me.on_ground && z > v => step_up(v, z, dt),
        _ => z,
    };
    bob.view_z = Some(view_z);
    // (GoldSrc z is up; the camera is y-up)
    cam.translation.y += bob.bob + (view_z - z);
    // The gun camera hangs on the world camera: moving and turning it the other way moves and
    // turns the gun (its origin is the eye, as in GoldSrc).
    let b = bob.bob;
    let turn = Quat::from_euler(EulerRot::YXZ, (-0.5 * b).to_radians(), 0.0, (-b).to_radians());
    **gun = Transform { translation: Vec3::new(0.0, 0.0, 0.4 * b), rotation: turn.inverse(), ..default() };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_still_does_not_bob_and_running_stays_within_cs_limits() {
        assert_eq!(bob_at(0.37, 0.0), 0.0);
        for i in 0..200 {
            let b = bob_at(i as f32 * 0.013, 250.0);
            assert!((LIMITS.0..=LIMITS.1).contains(&b), "{b}");
        }
        // at 250 u/s the peak is 0.3*2.5 + 0.7*2.5 = 2.5 a quarter into the cycle
        assert!((bob_at(CYCLE * UP / 2.0, 250.0) - 2.5).abs() < 1e-3);
    }

    #[test]
    fn a_stair_is_climbed_smoothly_and_never_lags_more_than_18() {
        // up 16 in one tick: the view starts 16 behind and catches up at 150 u/s
        let v = step_up(0.0, 16.0, 1.0 / 60.0);
        assert!((v - 2.5).abs() < 1e-3, "{v}");
        assert_eq!(step_up(0.0, 40.0, 0.0), 22.0);
        assert_eq!(step_up(15.0, 16.0, 1.0), 16.0);
    }
}
