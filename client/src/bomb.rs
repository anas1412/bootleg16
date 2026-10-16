//! The bomb in the world: the backpack when it's dropped, the planted C4 with its blinking
//! light and beeps that speed up as it runs down.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::models::{self, Parts};
use crate::{map, sound};
use bevy::prelude::*;
use net::protocol::BombNet;

pub struct BombPlugin;

impl Plugin for BombPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (show_bomb, beeps).in_set(Frame::World));
    }
}

#[derive(Component)]
struct BombView;

#[derive(Component)]
struct BombLight;

/// What is drawn now: dropped or planted, and where.
#[derive(Clone, Copy, PartialEq)]
enum Shown {
    Dropped([f32; 3]),
    Planted([f32; 3]),
}

#[allow(clippy::too_many_arguments)]
fn show_bomb(
    mut commands: Commands,
    w: Res<Walker>,
    mut cache: ResMut<models::ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut studio: ResMut<Assets<crate::studio::StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Local<Option<(Parts, Parts, Handle<Mesh>, Handle<StandardMaterial>)>>,
    mut shown: Local<Option<Shown>>,
    views: Query<Entity, With<BombView>>,
) {
    let want = match w.client.bomb {
        BombNet::Dropped(at) => Some(Shown::Dropped(at)),
        BombNet::Planted { at, .. } => Some(Shown::Planted(at)),
        _ => None,
    };
    if *shown == want {
        return;
    }
    *shown = want;
    for e in &views {
        commands.entity(e).despawn();
    }
    let Some(want) = want else { return };
    let (backpack, c4, light, red) = parts.get_or_insert_with(|| {
        let mut load = |file: &str| match cache.get(file, &mut images, &mut studio) {
            Some(m) => models::static_parts(&m, &m.model.pose(0, 0, 0.0), &mut meshes),
            None => Vec::new(),
        };
        let (b, c) = (load("w_backpack.mdl"), load("w_c4.mdl"));
        (b, c, meshes.add(Sphere::new(1.2)), materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.1, 0.05), unlit: true, ..default() }))
    });
    let (at, list) = match want {
        Shown::Dropped(at) => (at, &*backpack),
        Shown::Planted(at) => (at, &*c4),
    };
    let parent = commands.spawn((BombView, Transform::from_translation(Vec3::from(map::to_y_up(at))), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
    models::spawn_lit(&mut commands, parent, list, &mut studio);
    if matches!(want, Shown::Planted(_)) {
        let l = commands.spawn((BombLight, Mesh3d(light.clone()), MeshMaterial3d(red.clone()), Transform::from_xyz(0.0, 5.0, 0.0), Visibility::Hidden)).id();
        commands.entity(parent).add_child(l);
    }
}

/// Beep (and blink) faster as the timer runs down; CS goes through five beep sounds.
fn beeps(time: Res<Time>, mut w: ResMut<Walker>, mut next: Local<f32>, mut total: Local<f32>, mut lights: Query<&mut Visibility, With<BombLight>>) {
    let BombNet::Planted { at, left, .. } = w.client.bomb else {
        *total = 0.0;
        return;
    };
    if *total < left {
        *total = left; // first sight of it: about the full timer
        *next = 0.0;
    }
    let frac = (left / total.max(0.01)).clamp(0.0, 1.0);
    let now = time.elapsed_secs();
    let blink = now < *next - (0.1 + 1.4 * frac) * 0.5; // lit for the first half of each gap
    for mut v in &mut lights {
        v.set_if_neq(if blink { Visibility::Inherited } else { Visibility::Hidden });
    }
    if now >= *next {
        *next = now + 0.1 + 1.4 * frac;
        const BEEPS: [&str; 5] = ["weapons/c4_beep1.wav", "weapons/c4_beep2.wav", "weapons/c4_beep3.wav", "weapons/c4_beep4.wav", "weapons/c4_beep5.wav"];
        let file = BEEPS[(((1.0 - frac) * 5.0) as usize).min(4)];
        w.cues.push(sound::Cue::At { file, at, far: false });
    }
}
