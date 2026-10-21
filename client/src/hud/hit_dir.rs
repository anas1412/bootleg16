//! Where a hit came from: a short red arc on a ring round the crosshair, on the side the
//! shot (or blast) came from, fading over a second (CS 1.6's damage arcs, in our style). It
//! keeps pointing the right way as you turn, so a hit from behind tells you where to look.
//! Each hit gets its own mark; nothing for falls (they have no direction).
use crate::game::{FlyCam, Walker};
use crate::menu::AppState;
use crate::ui::{vh, HudLayer, BAD};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

/// How long a mark lasts, and how far from the centre it sits (1080p units).
const LIFE: f32 = 1.0;
const RADIUS: f32 = 170.0;
/// The arc: length along the ring and thickness.
const LENGTH: f32 = 120.0;
const THICK: f32 = 5.0;

/// A mark: the world yaw (GoldSrc degrees) it points to, and when it was made.
#[derive(Component)]
pub(super) struct Mark {
    yaw: f32,
    born: f32,
}

#[derive(Component)]
pub(super) struct Ring;

pub(super) fn spawn(mut commands: Commands) {
    // a zero-size point at the centre of the screen; the marks hang off it
    commands.spawn((Ring, HudLayer, DespawnOnExit(AppState::InGame), Pickable::IGNORE, Node { position_type: PositionType::Absolute, left: percent(50), top: percent(50), ..default() }));
}

pub(super) fn marks(
    mut commands: Commands,
    mut w: ResMut<Walker>,
    time: Res<Time>,
    cam: Single<&FlyCam>,
    window: Single<&Window, With<PrimaryWindow>>,
    ring: Single<Entity, With<Ring>>,
    mut q: Query<(Entity, &Mark, &mut UiTransform, &mut BackgroundColor)>,
) {
    let now = time.elapsed_secs();
    for yaw in w.client.take_hurt_from() {
        commands.entity(*ring).with_child((
            Mark { yaw, born: now },
            Node { position_type: PositionType::Absolute, left: px(0), top: px(0), width: vh(LENGTH), height: vh(THICK), border_radius: BorderRadius::all(vh(THICK / 2.0)), ..default() },
            UiTransform::default(),
            BackgroundColor(Color::NONE),
            Pickable::IGNORE,
        ));
    }
    // 1080p units to pixels at this window's height
    let s = window.height() / 1080.0;
    let view = cam.yaw.to_degrees() + 90.0;
    for (e, m, mut t, mut bg) in &mut q {
        let age = now - m.born;
        if age >= LIFE {
            commands.entity(e).despawn();
            continue;
        }
        // 0 ahead (top of the ring), positive to your left
        let a = (m.yaw - view).to_radians();
        let (sin, cos) = a.sin_cos();
        // the bar's centre on the ring, the bar along the ring
        let at = Vec2::new(-sin, -cos) * RADIUS * s;
        t.translation = Val2::px(at.x - LENGTH * s / 2.0, at.y - THICK * s / 2.0);
        t.rotation = Rot2::radians(-a);
        bg.0 = BAD.with_alpha(0.9 * (1.0 - age / LIFE).powf(1.5));
    }
}
