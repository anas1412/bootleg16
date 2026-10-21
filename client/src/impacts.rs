//! Bullet holes and dust where shots hit the map (every shotgun pellet too), and blood
//! where they hit players: a spray, more for headshots, and a splat on the wall behind.
//! The marks are CS's own decals (`decals.wad`: `{shot`, `{blood`, `{scorch`), drawn a texel
//! a unit like GoldSrc; maps' `infodecal`s are put on their walls when the map loads.
//! Each wall hit also puffs as CS 1.6 (`EV_HLDM_FireBullets` / `EV_HLDM_DecalGunshot` in
//! client.so, read with REA): an additive `wall_puff` sprite drifting off the wall at 25 u/s,
//! 0.4 to 0.6 size (wood 0.5 to 0.75), tinted by the surface (concrete lighter, wood brown),
//! with sparks off concrete, metal, vents, grates, tiles and computers.
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
            // Once the connection (and the map's collision) is in place.
            .add_systems(Update, map_decals.run_if(resource_added::<Walker>))
            .add_systems(Update, (spawn_hits, particles, puffs).chain().in_set(Frame::World));
    }
}

/// Something a bullet hit (sent by `effects::events`). Points are GoldSrc coordinates.
#[derive(Message)]
pub enum Hit {
    Wall { at: [f32; 3], normal: [f32; 3] },
    /// `dir`: which way the bullet was going.
    Player { at: [f32; 3], dir: [f32; 3], head: bool },
    /// Sparks (`env_spark`, bullets on metal).
    Sparks { at: [f32; 3] },
    /// An HE went off here: a scorch on the floor under it.
    Scorch { at: [f32; 3] },
}

/// Most decals kept at once (oldest go first, like GoldSrc's decal limit).
const MAX_DECALS: usize = 300;

#[derive(Resource, Default)]
struct Decals(VecDeque<Entity>);

#[derive(Resource)]
struct Looks {
    quad: Handle<Mesh>,
    ball: Handle<Mesh>,
    /// Decals from decals.wad, each with its size in units (one texel each).
    shots: Vec<(Handle<StandardMaterial>, Vec2)>,
    bloods: Vec<(Handle<StandardMaterial>, Vec2)>,
    scorches: Vec<(Handle<StandardMaterial>, Vec2)>,
    /// The wad, for the maps' own `infodecal`s.
    wad: Option<goldsrc::wad::Wad>,
    dust: Handle<StandardMaterial>,
    blood: Handle<StandardMaterial>,
    /// Hot sparks (additive, bright).
    spark: Handle<StandardMaterial>,
    /// The four wall puff sprites' frames.
    puffs: Vec<Vec<Handle<Image>>>,
    materials: goldsrc::materials::Materials,
}

/// A wall puff: which sprite, its age, size and drift (Y-up), and its own material.
#[derive(Component)]
struct Puff {
    sprite: usize,
    age: f32,
    size: f32,
    vel: Vec3,
    material: Handle<StandardMaterial>,
}

/// The puff sprites' frame rate (`EV_HLDM_DecalGunshot`'s 35) and their brightness (`renderamt` 150).
const PUFF_FPS: f32 = 35.0;
const PUFF_AMOUNT: f32 = 150.0 / 255.0;

/// How a surface puffs (`EV_HLDM_FireBullets`): tint (0-255), size, sparks.
fn puff_style(material: char) -> ([f32; 3], f32, bool) {
    match material {
        'C' => ([65.0; 3], 0.4, true),
        'W' => ([75.0, 42.0, 15.0], 0.5, false),
        'G' | 'V' | 'M' | 'P' | 'T' => ([40.0; 3], 0.4, true),
        _ => ([40.0; 3], 0.4, false),
    }
}

/// A decal's material: alpha-blended over the wall it sits on.
fn decal_material(img: Handle<Image>) -> StandardMaterial {
    StandardMaterial {
        base_color_texture: Some(img),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        // Drawn on top of the wall it sits on.
        depth_bias: 50.0,
        ..default()
    }
}

/// These decals from the wad (missing ones skipped), sized a texel a unit.
fn load_decals(wad: &Option<goldsrc::wad::Wad>, names: &[&str], images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> Vec<(Handle<StandardMaterial>, Vec2)> {
    names
        .iter()
        .filter_map(|n| wad.as_ref()?.decal(n))
        .map(|t| (materials.add(decal_material(images.add(rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb)))), Vec2::new(t.width as f32, t.height as f32)))
        .collect()
}

/// A decal flat on a surface (GoldSrc coordinates in, Y-up out): upright on walls.
fn decal_transform(at: Vec3, normal: Vec3, size: Vec2) -> Transform {
    let up = if normal.y.abs() > 0.9 { Vec3::Z } else { Vec3::Y };
    Transform::from_translation(at + normal * 0.15).looking_to(-normal, up).with_scale(size.extend(1.0))
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let solid = |c: Color| StandardMaterial { base_color: c, unlit: true, ..default() };
    let wad = crate::gamefiles::paths("decals.wad").iter().find_map(|p| goldsrc::wad::Wad::load(p).ok());
    commands.insert_resource(Looks {
        quad: meshes.add(Rectangle::new(1.0, 1.0)),
        ball: meshes.add(Sphere::new(1.0).mesh().ico(1).unwrap()),
        shots: load_decals(&wad, &["{shot1", "{shot2", "{shot3", "{shot4", "{shot5"], &mut images, &mut materials),
        bloods: load_decals(&wad, &["{blood1", "{blood2", "{blood3", "{blood4", "{blood5", "{blood6"], &mut images, &mut materials),
        scorches: load_decals(&wad, &["{scorch1", "{scorch2", "{scorch3"], &mut images, &mut materials),
        wad,
        dust: materials.add(solid(Color::srgb(0.55, 0.5, 0.42))),
        blood: materials.add(solid(Color::srgb(0.55, 0.02, 0.01))),
        spark: materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.8, 0.35), unlit: true, alpha_mode: AlphaMode::Add, ..default() }),
        // Additive: their texels are display values (see `explosions`), so no sRGB decode.
        puffs: (1..=4)
            .filter_map(|i| crate::gamefiles::paths(&format!("sprites/wall_puff{i}.spr")).iter().find_map(|p| goldsrc::spr::Sprite::load(p).ok()))
            .map(|s| s.frames.into_iter().map(|f| images.add(rgba_image(s.width, s.height, f, false, TextureFormat::Rgba8Unorm))).collect())
            .collect(),
        materials: crate::gamefiles::paths("sound/materials.txt").iter().find_map(|p| std::fs::read_to_string(p).ok()).map(|t| goldsrc::materials::Materials::parse(&t)).unwrap_or_else(|| goldsrc::materials::Materials::parse("")),
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

fn spawn_hits(mut commands: Commands, mut hits: MessageReader<Hit>, looks: Res<Looks>, mut decals: ResMut<Decals>, w: Res<Walker>, time: Res<Time>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let salt = time.elapsed_secs();
    // One of a set of decals, picked by `k`.
    let mut decal = |commands: &mut Commands, at: Vec3, normal: Vec3, set: &[(Handle<StandardMaterial>, Vec2)], k: f32| {
        let Some((material, size)) = set.get(((k * set.len() as f32) as usize).min(set.len().saturating_sub(1))) else { return };
        let e = commands
            .spawn((Mesh3d(looks.quad.clone()), MeshMaterial3d(material.clone()), decal_transform(at, normal, *size), DespawnOnExit(AppState::InGame)))
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
        match *hit {
            Hit::Wall { at, normal } => {
                let (p, n) = (y_up(at), y_up(normal).normalize_or(Vec3::Y));
                decal(&mut commands, p, n, &looks.shots, noise(k as u32, salt));
                burst(&mut commands, p + n, n, 5, 90.0, 0.9, 0.35, &looks.dust, true);
                // The surface decides the puff and the sparks.
                let (a, b): ([f32; 3], [f32; 3]) = (std::array::from_fn(|i| at[i] + normal[i] * 2.0), std::array::from_fn(|i| at[i] - normal[i] * 2.0));
                let world = &w.client.world;
                let surface = world.texture_of(world.trace_line(a, b).model, a, b).map(|t| looks.materials.kind(t)).unwrap_or('C');
                let (tint, scale, sparks) = puff_style(surface);
                if sparks {
                    let count = 15 + (noise(k as u32 + 11, salt) * 16.0) as u32;
                    burst(&mut commands, p + n, n, count, 75.0, 0.35, 0.3, &looks.spark, true);
                }
                if !looks.puffs.is_empty() {
                    let sprite = ((noise(k as u32 + 3, salt) * looks.puffs.len() as f32) as usize).min(looks.puffs.len() - 1);
                    let c = tint.map(|v| v / 255.0 * PUFF_AMOUNT);
                    let material = mats.add(StandardMaterial { base_color: Color::linear_rgb(c[0], c[1], c[2]), base_color_texture: looks.puffs[sprite].first().cloned(), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
                    let size = 128.0 * scale * (1.0 + 0.5 * noise(k as u32 + 7, salt));
                    commands.spawn((
                        Puff { sprite, age: 0.0, size, vel: n * 25.0, material: material.clone() },
                        Mesh3d(looks.quad.clone()),
                        MeshMaterial3d(material),
                        Transform::from_translation(p + n * 5.0).with_scale(Vec3::splat(size)),
                        DespawnOnExit(AppState::InGame),
                    ));
                }
            }
            // CS: a scorch on the floor under an HE (`UTIL_DecalTrace` down).
            Hit::Scorch { at } => {
                let t = w.client.world.trace_line(at, [at[0], at[1], at[2] - 64.0]);
                if t.fraction < 1.0 && !t.hit_sky {
                    decal(&mut commands, y_up(t.end), y_up(t.normal).normalize_or(Vec3::Y), &looks.scorches, noise(k as u32 + 3, salt));
                }
            }
            // GoldSrc's TE_SPARKS: a spray of hot bits that fall and fade fast.
            Hit::Sparks { at } => burst(&mut commands, y_up(at), Vec3::Y * 0.3, 14, 220.0, 0.5, 0.45, &looks.spark, true),
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
                    decal(&mut commands, y_up(t.end), y_up(t.normal).normalize_or(Vec3::Y), &looks.bloods, noise(k as u32 + 7, salt));
                }
            }
        }
    }
}

/// Play each puff's frames at 35 a second, facing the camera, drifting off the wall and rising.
fn puffs(mut commands: Commands, time: Res<Time>, looks: Res<Looks>, cam: Single<&GlobalTransform, With<crate::game::FlyCam>>, mut q: Query<(Entity, &mut Puff, &mut Transform), Without<crate::game::FlyCam>>, mut mats: ResMut<Assets<StandardMaterial>>) {
    let dt = time.delta_secs();
    let facing = cam.compute_transform().rotation;
    for (e, mut p, mut tf) in &mut q {
        p.age += dt;
        let frames = &looks.puffs[p.sprite];
        let Some(frame) = frames.get((p.age * PUFF_FPS) as usize) else {
            mats.remove(&p.material);
            commands.entity(e).despawn();
            continue;
        };
        if let Some(mut m) = mats.get_mut(&p.material) {
            if m.base_color_texture.as_ref() != Some(frame) {
                m.base_color_texture = Some(frame.clone());
            }
        }
        p.vel.y += 20.0 * dt; // smoke rises
        tf.translation += p.vel * dt;
        tf.rotation = facing;
        tf.scale = Vec3::splat(p.size);
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

/// The map's own decals (`infodecal`: a decals.wad texture stuck on the nearest surface).
fn map_decals(mut commands: Commands, looks: Res<Looks>, map: Res<crate::loading::MapData>, w: Res<Walker>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let bsp = &map.bsp;
    for e in bsp.entity_list().iter().filter(|e| e.get("classname").map(String::as_str) == Some("infodecal")) {
        let Some(name) = e.get("texture") else { continue };
        let Some(set) = Some(load_decals(&looks.wad, &[name.as_str()], &mut images, &mut materials)).filter(|s| !s.is_empty()) else { continue };
        let v: Vec<f32> = e.get("origin").map(|o| o.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        let Some(&[x, y, z]) = v.get(..3).map(|s| <&[f32; 3]>::try_from(s).unwrap()) else { continue };
        // The nearest surface within 16 units, along the six axes.
        let hit = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]]
            .iter()
            .map(|d: &[f32; 3]| w.client.world.trace_line([x, y, z], [x + d[0] * 16.0, y + d[1] * 16.0, z + d[2] * 16.0]))
            .filter(|t| t.fraction < 1.0 && !t.start_solid)
            .min_by(|a, b| a.fraction.total_cmp(&b.fraction));
        if let Some(t) = hit {
            let (material, size) = &set[0];
            commands.spawn((Mesh3d(looks.quad.clone()), MeshMaterial3d(material.clone()), decal_transform(y_up(t.end), y_up(t.normal).normalize_or(Vec3::Y), *size), DespawnOnExit(AppState::InGame)));
        }
    }
}
