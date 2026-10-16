//! Explosions as CS 1.6 draws them, with its own sprites:
//! - HE (`CGrenade::Explode3`): `fexplo` (scale 2.5) 20 units up and `eexplo` (3.0) up to
//!   64 units off, both played once at 30 fps, additive (`TE_EXPLOSION`); 0-3 spark showers;
//!   debris clatter; 0.55 s later a puff of `steam1` smoke (3.5-4.5, 5 fps) rising.
//! - C4 (`CGrenade::Explode2`): four huge sprites (scale 13.5) up to 512 units around, three
//!   at 150 brightness, `zerogxplode` at 17 (`TE_SPRITE`).
//!
//! The flash lights the walls too: a short orange point light (the engine's dynamic light).
use crate::effects::GameNews;
use crate::game::{Frame, Walker};
use crate::impacts::Hit;
use crate::level::rgba_image;
use crate::map;
use crate::menu::AppState;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use net::protocol::GameEvent;

pub struct ExplosionsPlugin;

impl Plugin for ExplosionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load).add_systems(Update, (start, play).chain().in_set(Frame::World).run_if(resource_exists::<Sprites>));
    }
}

/// One sprite's frames (images) and its width/height in texels.
struct Frames {
    images: Vec<Handle<Image>>,
    size: Vec2,
    /// Each frame's centre relative to the sprite's origin (texels, x right, y up).
    centres: Vec<Vec2>,
}

#[derive(Resource)]
struct Sprites {
    fexplo: Frames,
    eexplo: Frames,
    zerogxplode: Frames,
    steam: Frames,
    quad: Handle<Mesh>,
}

/// `additive`: GoldSrc adds these on screen (display values), so their texels are used as they
/// are (no sRGB decode): a dim fire frame still brightens a bright wall the way it does in CS.
fn frames(name: &str, additive: bool, images: &mut Assets<Image>) -> Frames {
    let s = ["cstrike", "valve"].iter().find_map(|d| goldsrc::spr::Sprite::load(crate::game_dir().join(d).join("sprites").join(name)).ok());
    let format = if additive { TextureFormat::Rgba8Unorm } else { TextureFormat::Rgba8UnormSrgb };
    match s {
        Some(s) => Frames {
            centres: s.origins.iter().map(|(x, y)| Vec2::new(*x as f32 + s.width as f32 / 2.0, *y as f32 - s.height as f32 / 2.0)).collect(),
            images: s.frames.into_iter().map(|f| images.add(rgba_image(s.width, s.height, f, false, format))).collect(),
            size: Vec2::new(s.width as f32, s.height as f32),
        },
        None => Frames { images: Vec::new(), size: Vec2::ONE, centres: Vec::new() },
    }
}

fn load(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(Sprites {
        fexplo: frames("fexplo.spr", true, &mut images),
        eexplo: frames("eexplo.spr", true, &mut images),
        zerogxplode: frames("zerogxplode.spr", true, &mut images),
        steam: frames("steam1.spr", false, &mut images),
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
    });
}

#[derive(Clone, Copy)]
enum Which {
    Fexplo,
    Eexplo,
    Zero,
    Steam,
}

/// A sprite animation playing once, facing the camera.
#[derive(Component)]
struct Anim {
    which: Which,
    at: Vec3,
    scale: f32,
    fps: f32,
    age: f32,
    /// Seconds before it shows (the smoke comes after the blast).
    delay: f32,
    /// Units a second it drifts up (smoke).
    rise: f32,
    material: Handle<StandardMaterial>,
}

/// A brief light from the blast.
#[derive(Component)]
struct Burst(f32);

fn start(mut commands: Commands, mut news: MessageReader<GameNews>, sprites: Res<Sprites>, mut materials: ResMut<Assets<StandardMaterial>>, mut hits: MessageWriter<Hit>, mut w: ResMut<Walker>, mut rng: Local<u64>) {
    if *rng == 0 {
        *rng = 0x2F6B_9D3C_11A7_E4F5;
    }
    let mut rand = || {
        *rng ^= *rng << 13;
        *rng ^= *rng >> 7;
        *rng ^= *rng << 17;
        (*rng % 100_000) as f32 / 100_000.0
    };
    for GameNews(e) in news.read() {
        let mut spawn = |which: Which, at: [f32; 3], scale: f32, fps: f32, bright: f32, delay: f32, rise: f32| {
            let additive = !matches!(which, Which::Steam);
            let first = match which {
                Which::Fexplo => &sprites.fexplo,
                Which::Eexplo => &sprites.eexplo,
                Which::Zero => &sprites.zerogxplode,
                Which::Steam => &sprites.steam,
            }
            .images
            .first()
            .cloned();
            let material = materials.add(StandardMaterial {
                base_color_texture: first,
                base_color: if additive { Color::linear_rgb(bright, bright, bright) } else { Color::WHITE },
                unlit: true,
                alpha_mode: if additive { AlphaMode::Add } else { AlphaMode::Blend },
                ..default()
            });
            commands.spawn((
                Anim { which, at: Vec3::from(map::to_y_up(at)), scale, fps, age: 0.0, delay, rise, material: material.clone() },
                Mesh3d(sprites.quad.clone()),
                MeshMaterial3d(material),
                Transform::default(),
                Visibility::Hidden,
                DespawnOnExit(AppState::InGame),
            ));
        };
        match e {
            GameEvent::Explosion { at } => {
                spawn(Which::Fexplo, [at[0], at[1], at[2] + 20.0], 2.5, 30.0, 1.0, 0.0, 0.0);
                spawn(Which::Eexplo, [at[0] + (rand() - 0.5) * 128.0, at[1] + (rand() - 0.5) * 128.0, at[2] + 30.0 + rand() * 5.0], 3.0, 30.0, 1.0, 0.0, 0.0);
                spawn(Which::Steam, [at[0], at[1], at[2] - 5.0], 3.5 + rand(), 5.0, 1.0, 0.55, 30.0);
                for _ in 0..(rand() * 4.0) as usize {
                    hits.write(Hit::Sparks { at: *at });
                }
                let file = ["weapons/debris1.wav", "weapons/debris2.wav", "weapons/debris3.wav"][((rand() * 3.0) as usize).min(2)];
                w.cues.push(crate::sound::Cue::Quiet { file, at: *at, volume: 0.55 });
                commands.spawn((Burst(0.0), PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 1.5e5, range: 400.0, ..default() }, Transform::from_translation(Vec3::from(map::to_y_up(*at))), DespawnOnExit(AppState::InGame)));
            }
            GameEvent::BombExploded { at } => {
                let around = |r: &mut dyn FnMut() -> f32| [at[0] + (r() - 0.5) * 1024.0, at[1] + (r() - 0.5) * 1024.0, at[2] + (r() - 0.5) * 20.0];
                spawn(Which::Fexplo, [at[0], at[1], at[2] - 10.0], 13.5, 10.0, 150.0 / 255.0, 0.0, 0.0);
                spawn(Which::Eexplo, around(&mut rand), 13.5, 10.0, 150.0 / 255.0, 0.0, 0.0);
                spawn(Which::Fexplo, around(&mut rand), 13.5, 10.0, 150.0 / 255.0, 0.0, 0.0);
                spawn(Which::Zero, around(&mut rand), 13.5, 10.0, 17.0 / 255.0, 0.0, 0.0);
                commands.spawn((Burst(0.0), PointLight { color: Color::srgb(1.0, 0.75, 0.4), intensity: 2.0e6, range: 1500.0, ..default() }, Transform::from_translation(Vec3::from(map::to_y_up(*at))), DespawnOnExit(AppState::InGame)));
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    mut commands: Commands,
    time: Res<Time>,
    sprites: Res<Sprites>,
    cam: Single<&GlobalTransform, With<crate::game::FlyCam>>,
    mut anims: Query<(Entity, &mut Anim, &mut Transform, &mut Visibility), Without<Burst>>,
    mut bursts: Query<(Entity, &mut Burst, &mut PointLight)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let dt = time.delta_secs();
    let eye = cam.translation();
    for (e, mut a, mut tf, mut vis) in &mut anims {
        a.age += dt;
        if a.age < a.delay {
            continue;
        }
        let set = match a.which {
            Which::Fexplo => &sprites.fexplo,
            Which::Eexplo => &sprites.eexplo,
            Which::Zero => &sprites.zerogxplode,
            Which::Steam => &sprites.steam,
        };
        let frame = ((a.age - a.delay) * a.fps) as usize;
        let Some(image) = set.images.get(frame) else {
            commands.entity(e).despawn();
            continue;
        };
        if let Some(mut m) = materials.get_mut(&a.material) {
            if m.base_color_texture.as_ref() != Some(image) {
                m.base_color_texture = Some(image.clone());
            }
        }
        let rise = a.rise * dt;
        a.at.y += rise;
        *tf = Transform::from_translation(a.at).looking_at(eye, Vec3::Y).with_scale((set.size * a.scale).extend(1.0));
        tf.rotate_local_y(std::f32::consts::PI); // the quad faces +Z
        // The frame sits where its origin puts it (camera-facing right and up).
        let c = set.centres.get(frame).copied().unwrap_or(Vec2::ZERO) * a.scale;
        let (right, up) = (tf.rotation * Vec3::X, tf.rotation * Vec3::Y);
        tf.translation += right * c.x + up * c.y;
        vis.set_if_neq(Visibility::Inherited);
    }
    // The engine's explosion light: bright, gone in a third of a second.
    for (e, mut b, mut light) in &mut bursts {
        b.0 += dt;
        if b.0 > 0.35 {
            commands.entity(e).despawn();
        } else {
            light.intensity *= (1.0 - dt / 0.35).max(0.0);
        }
    }
}
