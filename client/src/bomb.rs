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

/// The LED glow's size (units).
const GLOW_SIZE: f32 = 0.3 * 64.0;

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
        // CS's LED glow (`TE_GLOWSPRITE` with `ledglow.spr` at a tenth of its size: 0.3 x 64 texels).
        let glow = crate::gamefiles::paths("sprites/ledglow.spr").iter().find_map(|p| goldsrc::spr::Sprite::load(p).ok()).and_then(|s| s.frames.into_iter().next().map(|f| (s.width, s.height, f)));
        let material = match glow {
            Some((gw, gh, f)) => StandardMaterial { base_color_texture: Some(images.add(crate::level::rgba_image(gw, gh, f, false, bevy::render::render_resource::TextureFormat::Rgba8Unorm))), unlit: true, alpha_mode: AlphaMode::Add, ..default() },
            None => StandardMaterial { base_color: Color::srgb(1.0, 0.1, 0.05), unlit: true, ..default() },
        };
        (b, c, meshes.add(Rectangle::new(GLOW_SIZE, GLOW_SIZE)), materials.add(material))
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

/// The bomb's beeps and blinks as CS 1.6's `CGrenade::C4Think` (cs.so, read with REA): a beep
/// every 1.4 s from 0.5 s after the plant; the beep sound goes through five stages (each file
/// beeps more often, and carries further: attenuation 1.5, 1.0, 0.8, 0.5, 0.2), the first a
/// quarter of the timer long and each one 10% shorter than the last; the LED glows for a tenth
/// of a second every 2 s from 2 s in.
const BEEP_EVERY: f32 = 1.4;
const FIRST_BEEP: f32 = 0.5;
const BLINK_EVERY: f32 = 2.0;
const BLINK_FOR: f32 = 0.1;
const STAGES: [(&str, f32); 5] = [("weapons/c4_beep1.wav", 1.5), ("weapons/c4_beep2.wav", 1.0), ("weapons/c4_beep3.wav", 0.8), ("weapons/c4_beep4.wav", 0.5), ("weapons/c4_beep5.wav", 0.2)];

/// Which beep stage the bomb is in, `elapsed` seconds after a plant with a `timer`-second fuse.
fn stage(elapsed: f32, timer: f32) -> usize {
    let mut interval = (timer as i32 >> 2) as f32; // whole seconds, as CS stores it
    let (mut start, mut k) = (0.0, 0);
    while k + 1 < STAGES.len() && elapsed >= start + interval {
        start += interval;
        interval *= 0.9;
        k += 1;
    }
    k
}

fn beeps(mut w: ResMut<Walker>, mut total: Local<f32>, mut beeped: Local<u32>, cam: Single<&GlobalTransform, With<crate::game::FlyCam>>, mut lights: Query<(&mut Visibility, &mut Transform, &ChildOf), With<BombLight>>, parents: Query<&GlobalTransform, Without<crate::game::FlyCam>>) {
    let BombNet::Planted { at, left, .. } = w.client.bomb else {
        (*total, *beeped) = (0.0, 0);
        return;
    };
    if *total < left {
        *total = left; // first sight of it: about the full timer
        *beeped = 0;
    }
    let elapsed = *total - left;
    let lit = elapsed >= BLINK_EVERY && elapsed % BLINK_EVERY < BLINK_FOR;
    let face = cam.compute_transform().rotation;
    for (mut v, mut tf, parent) in &mut lights {
        v.set_if_neq(if lit { Visibility::Inherited } else { Visibility::Hidden });
        // face the camera (the parent is the bomb, which has no turn of its own)
        let up = parents.get(parent.parent()).map(|g| g.compute_transform().rotation).unwrap_or_default();
        tf.rotation = up.inverse() * face;
    }
    let due = if elapsed < FIRST_BEEP { 0 } else { ((elapsed - FIRST_BEEP) / BEEP_EVERY) as u32 + 1 };
    if due > *beeped {
        *beeped = due;
        let (file, attn) = STAGES[stage(elapsed, *total)];
        w.cues.push(sound::Cue::Far { file, at, attn });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_beep_stages_follow_cs() {
        // 40 s fuse: stages start at 0, 10, 19, 27.1 and 34.39 s
        assert_eq!(stage(0.0, 40.0), 0);
        assert_eq!(stage(9.9, 40.0), 0);
        assert_eq!(stage(10.0, 40.0), 1);
        assert_eq!(stage(18.9, 40.0), 1);
        assert_eq!(stage(19.0, 40.0), 2);
        assert_eq!(stage(27.2, 40.0), 3);
        assert_eq!(stage(34.5, 40.0), 4);
        assert_eq!(stage(39.9, 40.0), 4);
    }
}
