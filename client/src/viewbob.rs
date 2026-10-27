//! Walking bob and stair smoothing on the Bevy camera, as CS 1.6 does them (the maths is in
//! `client_core::view`). The gun moves forward by 0.4 of the bob and turns a little (yaw by
//! half of it, roll by all of it, in degrees). Only what you see moves: your shots still come
//! from your eye.
use crate::game::{FlyCam, Frame, Walker};
use crate::viewmodel::ViewModelCamera;
use bevy::prelude::*;
use client_core::view::{bob_at, step_up};

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
