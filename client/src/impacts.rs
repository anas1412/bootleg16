//! Bullet holes and dust where shots hit the map (every shotgun pellet too), and blood
//! where they hit players: a spray, more for headshots, and a splat on the wall behind.
use crate::game::{Frame, Walker};
use crate::level::rgba_image;
use crate::map;
use crate::menu::AppState;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use std::collections::VecDeque;

pub struct ImpactsPlugin;

impl Plugin for ImpactsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Hit>()
            .init_resource::<Decals>()
            .add_systems(Startup, load)
            .add_systems(OnEnter(AppState::InGame), |mut d: ResMut<Decals>| d.0.clear())
            .add_systems(Update, (spawn_hits, particles).chain().in_set(Frame::World));
    }
}

/// Something a bullet hit (sent by `effects::events`). Points are GoldSrc coordinates.
#[derive(Message)]
pub enum Hit {
    Wall { at: [f32; 3], normal: [f32; 3] },
    /// `dir`: which way the bullet was going.
    Player { at: [f32; 3], dir: [f32; 3], head: bool },
    /// A breakable broke here: a burst of bits in its material's colour.
    Debris { at: [f32; 3], material: sim::breakables::Material },
}

/// Most decals kept at once (oldest go first, like GoldSrc's decal limit).
const MAX_DECALS: usize = 300;

#[derive(Resource, Default)]
struct Decals(VecDeque<Entity>);

#[derive(Resource)]
struct Looks {
    quad: Handle<Mesh>,
    ball: Handle<Mesh>,
    hole: Handle<StandardMaterial>,
    splat: Handle<StandardMaterial>,
    dust: Handle<StandardMaterial>,
    blood: Handle<StandardMaterial>,
    /// Breakable debris: glass, wood, metal.
    shards: [Handle<StandardMaterial>; 3],
}

/// A round soft-edged spot: `rgb` with alpha falling off from the middle (`hard` 0..1 keeps
/// the centre solid), roughened a little so it doesn't look like a perfect circle.
fn spot(n: u32, rgb: [u8; 3], hard: f32, rough: f32) -> Image {
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let ang = dy.atan2(dx);
            let edge = 1.0 - rough * (0.5 + 0.5 * (ang * 5.0).sin() * (ang * 3.0 + 1.0).cos());
            let r = (dx * dx + dy * dy).sqrt() / c / edge;
            let a = ((1.0 - r) / (1.0 - hard)).clamp(0.0, 1.0);
            px.extend_from_slice(&[rgb[0], rgb[1], rgb[2], (a * 255.0) as u8]);
        }
    }
    rgba_image(n, n, px, false, TextureFormat::Rgba8UnormSrgb)
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let decal = |img: Handle<Image>| StandardMaterial {
        base_color_texture: Some(img),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        // Drawn on top of the wall it sits on.
        depth_bias: 50.0,
        ..default()
    };
    let solid = |c: Color| StandardMaterial { base_color: c, unlit: true, ..default() };
    commands.insert_resource(Looks {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        ball: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        hole: materials.add(decal(images.add(spot(32, [18, 16, 14], 0.45, 0.1)))),
        splat: materials.add(decal(images.add(spot(64, [110, 6, 4], 0.3, 0.45)))),
        dust: materials.add(solid(Color::srgb(0.55, 0.5, 0.42))),
        blood: materials.add(solid(Color::srgb(0.55, 0.02, 0.01))),
        shards: [
            materials.add(StandardMaterial { base_color: Color::srgba(0.8, 0.9, 1.0, 0.6), unlit: true, alpha_mode: AlphaMode::Blend, ..default() }),
            materials.add(solid(Color::srgb(0.45, 0.3, 0.15))),
            materials.add(solid(Color::srgb(0.5, 0.52, 0.55))),
        ],
    });
}

/// A short-lived blob: velocity (GoldSrc units/s, Y-up), seconds left of `life`, falls if `gravity`.
#[derive(Component)]
struct Particle {
    vel: Vec3,
    left: f32,
    life: f32,
    size: f32,
    gravity: bool,
}

fn y_up(v: [f32; 3]) -> Vec3 {
    Vec3::from(map::to_y_up(v))
}

/// Tiny deterministic noise so effects vary without a random number generator.
fn noise(i: u32, salt: f32) -> f32 {
    ((i as f32 * 12.9898 + salt * 78.233).sin() * 43758.547).fract()
}

fn spawn_hits(mut commands: Commands, mut hits: MessageReader<Hit>, looks: Res<Looks>, mut decals: ResMut<Decals>, w: Res<Walker>, time: Res<Time>) {
    let salt = time.elapsed_secs();
    let mut decal = |commands: &mut Commands, at: Vec3, normal: Vec3, size: f32, material: &Handle<StandardMaterial>, spin: f32| {
        let rot = Quat::from_rotation_arc(Vec3::Z, normal) * Quat::from_rotation_z(spin);
        let e = commands
            .spawn((
                Mesh3d(looks.quad.clone()),
                MeshMaterial3d(material.clone()),
                Transform { translation: at + normal * 0.15, rotation: rot, scale: Vec3::splat(size) },
                DespawnOnExit(AppState::InGame),
            ))
            .id();
        decals.0.push_back(e);
        if decals.0.len() > MAX_DECALS {
            if let Some(old) = decals.0.pop_front() {
                commands.entity(old).try_despawn();
            }
        }
    };
    let burst = |commands: &mut Commands, at: Vec3, dir: Vec3, n: u32, speed: f32, size: f32, life: f32, material: &Handle<StandardMaterial>, gravity: bool| {
        for i in 0..n {
            let jitter = Vec3::new(noise(i, salt) - 0.5, noise(i + 31, salt) - 0.5, noise(i + 77, salt) - 0.5);
            let vel = (dir + jitter * 1.2).normalize_or_zero() * speed * (0.5 + noise(i + 5, salt));
            commands.spawn((
                Particle { vel, left: life, life, size, gravity },
                Mesh3d(looks.ball.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(at).with_scale(Vec3::splat(size)),
                DespawnOnExit(AppState::InGame),
            ));
        }
    };
    for (k, hit) in hits.read().enumerate() {
        let spin = noise(k as u32, salt) * std::f32::consts::TAU;
        match *hit {
            Hit::Wall { at, normal } => {
                let (p, n) = (y_up(at), y_up(normal).normalize_or(Vec3::Y));
                decal(&mut commands, p, n, 4.0, &looks.hole, spin);
                burst(&mut commands, p + n, n, 5, 90.0, 0.9, 0.35, &looks.dust, true);
            }
            Hit::Debris { at, material } => {
                use sim::breakables::Material::*;
                let m = match material {
                    Glass | UnbreakableGlass => &looks.shards[0],
                    Wood => &looks.shards[1],
                    _ => &looks.shards[2],
                };
                burst(&mut commands, y_up(at), Vec3::Y, 24, 180.0, 1.6, 1.2, m, true);
            }
            Hit::Player { at, dir, head } => {
                let (p, d) = (y_up(at), y_up(dir).normalize_or(Vec3::Y));
                // Spray out of the exit side, and a puff back towards the shooter.
                let (n, speed, size) = if head { (22, 160.0, 1.4) } else { (10, 110.0, 1.0) };
                burst(&mut commands, p, d, n, speed, size, 0.5, &looks.blood, true);
                burst(&mut commands, p, -d, n / 2, speed * 0.5, size, 0.3, &looks.blood, true);
                // Splat on a wall close behind them, as CS does.
                let reach = if head { 180.0 } else { 120.0 };
                let end = std::array::from_fn(|i| at[i] + dir[i] * reach);
                let t = w.client.world.trace_line(at, end);
                if t.fraction < 1.0 && !t.hit_sky && !t.start_solid {
                    decal(&mut commands, y_up(t.end), y_up(t.normal).normalize_or(Vec3::Y), if head { 34.0 } else { 24.0 }, &looks.splat, spin);
                }
            }
        }
    }
}

fn particles(mut commands: Commands, time: Res<Time>, mut q: Query<(Entity, &mut Particle, &mut Transform)>) {
    let dt = time.delta_secs();
    for (e, mut p, mut tf) in &mut q {
        p.left -= dt;
        if p.left <= 0.0 {
            commands.entity(e).despawn();
            continue;
        }
        if p.gravity {
            p.vel.y -= 800.0 * dt; // sv_gravity
        }
        tf.translation += p.vel * dt;
        tf.scale = Vec3::splat(p.size * (p.left / p.life).sqrt());
    }
}
