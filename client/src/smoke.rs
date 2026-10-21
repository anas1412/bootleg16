//! Smoke clouds drawn like CS 1.6 (`CreateSmokePop` / `CreateGasSmoke` / `CPartSmokeGrenade`
//! in client.so, read with REA): the gas puff sprite, each puff 175 to 225 units, grey 155 to
//! 175, fully opaque within a moment of appearing (brightness ramps to 255), thinning out only
//! over the last five seconds. The puffs billow out over the first second and a half.
//! (The server also treats them as blocking sight: server/visibility.rs.)
use crate::game::{FlyCam, Frame, Walker};
use crate::level::rgba_image;
use crate::menu::AppState;
use crate::{map, sound};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

pub struct SmokePlugin;

impl Plugin for SmokePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, (clouds, puffs).chain().in_set(Frame::World));
    }
}

/// Puffs per cloud, and how far they reach from its middle (GoldSrc units).
const PUFFS: usize = 40;
const SPREAD: f32 = 120.0;
/// Puff size range (CS: `RandomFloat(175, 225)`) and grey range (`RandomFloat(155, 175)`).
const PUFF_SIZE: (f32, f32) = (175.0, 225.0);
const GREY: (f32, f32) = (155.0, 175.0);
/// Seconds to billow out, and to fade at the end (CS fades its puffs over the last 5).
const BLOOM: f32 = 1.5;
const FADE: f32 = 5.0;

#[derive(Resource)]
struct Looks {
    quad: Handle<Mesh>,
    puff: Handle<Image>,
}

/// A soft round puff: grey, alpha falling off smoothly to the edge, a little lumpy.
fn puff_image(n: u32) -> Image {
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let ang = dy.atan2(dx);
            let lump = 1.0 + 0.08 * (ang * 5.0).sin() + 0.05 * (ang * 9.0 + 2.0).cos();
            let r = (dx * dx + dy * dy).sqrt() / c / lump;
            let a = (1.0 - r).clamp(0.0, 1.0).powf(1.6);
            let g = (200.0 + 25.0 * (1.0 - r)) as u8;
            px.extend_from_slice(&[g, g, g, (a * 255.0) as u8]);
        }
    }
    rgba_image(n, n, px, false, TextureFormat::Rgba8UnormSrgb)
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>) {
    // CS's gas puff sprite (first frame); a soft puff of our own if the data has none.
    let sprite = ["sprites/gas_puff_01.spr", "sprites/gas_puff_01_wh.spr"].iter().flat_map(|f| crate::gamefiles::paths(f)).find_map(|p| goldsrc::spr::Sprite::load(p).ok());
    let puff = match sprite.and_then(|s| s.frames.into_iter().next().map(|f| (s.width, s.height, f))) {
        Some((w, h, rgba)) => rgba_image(w, h, rgba, false, TextureFormat::Rgba8UnormSrgb),
        None => puff_image(64),
    };
    commands.insert_resource(Looks { quad: meshes.add(Rectangle::new(1.0, 1.0)), puff: images.add(puff) });
}

/// One cloud, keyed by where it popped.
#[derive(Component)]
struct Cloud([i32; 3]);

/// One puff: where it settles relative to the cloud, its size, drift and spin.
#[derive(Component)]
struct Puff {
    home: Vec3,
    size: f32,
    drift: Vec3,
    spin: f32,
    material: Handle<StandardMaterial>,
}

/// Spawn clouds that appeared, remove ones that are gone.
fn clouds(mut commands: Commands, mut w: ResMut<Walker>, looks: Res<Looks>, mut materials: ResMut<Assets<StandardMaterial>>, existing: Query<(Entity, &Cloud)>) {
    let key = |at: [f32; 3]| at.map(|v| v.round() as i32);
    let live: Vec<_> = w.client.smokes.iter().map(|s| (key(s.at), *s)).collect();
    for (e, c) in &existing {
        if !live.iter().any(|(k, _)| *k == c.0) {
            commands.entity(e).despawn();
        }
    }
    for (k, smoke) in live {
        if existing.iter().any(|(_, c)| c.0 == k) {
            continue;
        }
        if smoke.left > sim::grenades::SMOKE_SECONDS as f32 - 1.0 {
            w.cues.push(sound::Cue::Smoke { at: smoke.at }); // just popped (not one we joined late)
        }
        let centre = Vec3::from(map::to_y_up(smoke.at));
        let parent = commands.spawn((Cloud(k), Transform::from_translation(centre), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        // Deterministic scatter (golden-angle spiral, layered upwards) so every client sees the same cloud.
        for i in 0..PUFFS {
            let f = i as f32;
            let a = f * 2.399_963;
            let r = SPREAD * (0.25 + 0.75 * ((f + 0.5) / PUFFS as f32).sqrt());
            let home = Vec3::new(a.cos() * r, 25.0 + (i % 5) as f32 * 28.0 + (f * 0.37).sin() * 10.0, a.sin() * r * 0.9);
            let k = (f * 1.7).sin() * 0.5 + 0.5;
            let grey = (GREY.0 + (GREY.1 - GREY.0) * ((f * 2.9).cos() * 0.5 + 0.5)) / 255.0;
            let material = materials.add(StandardMaterial { base_color: Color::srgb(grey, grey, grey), base_color_texture: Some(looks.puff.clone()), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
            let puff = Puff {
                home,
                size: PUFF_SIZE.0 + (PUFF_SIZE.1 - PUFF_SIZE.0) * k,
                drift: Vec3::new((f * 0.9).sin(), 0.3, (f * 1.3).cos()) * 6.0,
                spin: ((f * 2.3).sin()) * 0.15,
                material: material.clone(),
            };
            let child = commands.spawn((puff, Mesh3d(looks.quad.clone()), MeshMaterial3d(material), Transform::from_scale(Vec3::ZERO))).id();
            commands.entity(parent).add_child(child);
        }
    }
}

/// Grow, drift and fade each puff, always facing the camera.
fn puffs(
    w: Res<Walker>,
    time: Res<Time>,
    cam: Single<&GlobalTransform, With<FlyCam>>,
    clouds: Query<(&Cloud, &Children)>,
    mut puffs: Query<(&Puff, &mut Transform)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let t = time.elapsed_secs();
    let cam_rot = cam.compute_transform().rotation;
    for (cloud, children) in &clouds {
        let Some(s) = w.client.smokes.iter().find(|s| s.at.map(|v| v.round() as i32) == cloud.0) else { continue };
        let age = sim::grenades::SMOKE_SECONDS as f32 - s.left;
        let grow = (age / BLOOM).clamp(0.0, 1.0);
        let grow = 1.0 - (1.0 - grow).powi(3); // fast out, settles gently
        let alpha = (s.left / FADE).clamp(0.0, 1.0);
        for c in children {
            let Ok((p, mut tf)) = puffs.get_mut(*c) else { continue };
            tf.translation = p.home * grow + p.drift * (age * 0.3).min(4.0);
            tf.rotation = cam_rot * Quat::from_rotation_z(p.spin * t);
            tf.scale = Vec3::splat(p.size * (0.3 + 0.7 * grow));
            if let Some(mut m) = materials.get_mut(&p.material) {
                if (m.base_color.alpha() - alpha).abs() > 0.01 {
                    m.base_color.set_alpha(alpha);
                }
            }
        }
    }
}
