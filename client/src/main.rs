//! The game client. Loads the map from the user's CS 1.6 install, connects to a
//! server (or starts one inside the game when playing alone) and plays with
//! CS 1.6 movement. V toggles free flight.
//!
//! Usage: `client [map] [--connect host:port] [--name name]` (default de_dust2, offline).
//! The game directory comes from `BOOTLEG16_DIR` (default: the Steam Half-Life install).
//! `BOOTLEG16_FAKE_PING=ms` and `BOOTLEG16_FAKE_LOSS=0.05` simulate a bad connection.
//! `BOOTLEG16_SCREENSHOT=path.png` saves one frame (after `BOOTLEG16_SCREENSHOT_AT` seconds,
//! default 2) and exits; such runs ignore mouse and keyboard (used for automated checks).
//! `BOOTLEG16_SPECTATE=1` watches another player instead of playing (as when dead).
//! `--give ak47,awp` (offline) hands you extra weapons until there's a buy menu.
mod map;

use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    input::mouse::AccumulatedMouseMotion,
    light::GlobalAmbientLight,
    math::Rect,
    mesh::{Indices, PrimitiveTopology},
    pbr::Lightmap,
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::window::screenshot::{save_to_disk, Screenshot},
    },
    window::{CursorGrabMode, CursorOptions},
};
use std::path::PathBuf;

/// GoldSrc brightens lightmaps (overbright); this matches it by eye.
const LIGHTMAP_SCALE: f32 = 2.0;

fn game_dir() -> PathBuf {
    std::env::var_os("BOOTLEG16_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".steam/steam/steamapps/common/Half-Life")
    })
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "bootleg16".into(), ..default() }),
            ..default()
        }))
        // GoldSrc lighting is fully baked into lightmaps; no extra ambient.
        .insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() })
        .insert_resource(ClearColor(Color::srgb(0.45, 0.6, 0.8)))
        // The movement simulation runs at a fixed 128 ticks per second, like the server will.
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, walk)
        .add_systems(Update, (mouse_look, move_camera, move_doors, draw_others, effects, grenade_effects, grab_cursor, buy_menu, scoreboard, hud, screenshot).chain())
        .run();
}

fn rgba_image(w: u32, h: u32, rgba: Vec<u8>, repeat: bool, format: TextureFormat) -> Image {
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    if repeat {
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    } else {
        img.sampler = ImageSampler::linear();
    }
    img
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let root = game_dir();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    let name = args.first().filter(|a| !a.starts_with("--")).cloned().unwrap_or_else(|| "de_dust2".into());
    let path = root.join("cstrike/maps").join(format!("{name}.bsp"));
    let geo = map::load(&path, &[&root.join("cstrike"), &root.join("valve")])
        .unwrap_or_else(|e| panic!("loading {}: {e}", path.display()));
    info!("{name}: {} batches, lightmap {:?}", geo.batches.len(), geo.lightmap_size);

    let (lw, lh) = geo.lightmap_size;
    // Lightmaps are light intensities, not colours: keep them linear (no sRGB decode).
    let lightmap = images.add(rgba_image(lw as u32, lh as u32, geo.lightmap_rgba, false, TextureFormat::Rgba8Unorm));
    let mut tex_handles = vec![None; geo.textures.len()];

    let mut spawn_batch = |b: map::Batch, commands: &mut Commands| -> Entity {
        let t = &geo.textures[b.texture];
        let image = tex_handles[b.texture]
            .get_or_insert_with(|| images.add(rgba_image(t.width, t.height, t.rgba.clone(), true, TextureFormat::Rgba8UnormSrgb)))
            .clone();
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            lightmap_exposure: LIGHTMAP_SCALE,
            alpha_mode: if t.name.starts_with('{') { AlphaMode::Mask(0.5) } else { AlphaMode::Opaque },
            ..default()
        });
        // Keep the mesh on the CPU side too: Bevy needs it to compute the bounding box for
        // culling (GPU-only meshes get a box at the origin and vanish when it's off-screen).
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, b.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, b.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, b.lightmap_uv)
            .with_inserted_indices(Indices::U32(b.indices))
            .with_computed_normals(); // face vertices are never shared, so these are flat
        commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Lightmap { image: lightmap.clone(), uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0), ..default() },
            ))
            .id()
    };
    for b in geo.batches {
        spawn_batch(b, &mut commands);
    }
    // Doors: one parent per door, moved every frame to where the sim says it is.
    for (model, batches) in geo.movers {
        let parent = commands.spawn((DoorMesh(model), Transform::default(), Visibility::default())).id();
        for b in batches {
            let child = spawn_batch(b, &mut commands);
            commands.entity(parent).add_child(child);
        }
    }

    let (pos, yaw) = geo.spawns.first().copied().unwrap_or(([0.0; 3], 0.0));
    let bsp = goldsrc::bsp::Bsp::load(&path).expect("map loaded above");
    // Playing alone still goes through a server, started inside the game.
    let server_addr = match flag("--connect") {
        Some(a) => std::net::ToSocketAddrs::to_socket_addrs(&a)
            .ok()
            .and_then(|mut it| it.next())
            .unwrap_or_else(|| panic!("can't resolve server address {a}")),
        None => {
            let assets = net::server::Assets::load(&root).unwrap_or_else(|e| panic!("loading game files: {e}"));
            let mut server = net::Server::new(([127, 0, 0, 1], 0).into(), &name, &bsp, assets).expect("starting local server");
            if let Some(list) = flag("--give") {
                server.set_give(list.split(',').filter_map(sim::weapons::data::by_name).collect());
            }
            let addr = server.local_addr();
            std::thread::spawn(move || server.run());
            addr
        }
    };
    let player_name = flag("--name").unwrap_or_else(|| "player".into());
    let mut client = net::Client::connect(server_addr, &player_name, sim::collision::World::from_bsp(&bsp)).expect("opening socket");
    let ping = std::env::var("BOOTLEG16_FAKE_PING").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let loss = std::env::var("BOOTLEG16_FAKE_LOSS").ok().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    client.simulate_network(ping, loss);
    info!("connecting to {server_addr} as {player_name} (fake ping {ping} ms, loss {loss})");
    let eye = Vec3::from(pos) + Vec3::Y * sim::pmove::VIEW_HEIGHT_STANDING;
    commands.insert_resource(Walker {
        client,
        prev_eye: [0.0; 3],
        noclip: false,
        slot_held: 0,
        drop_held: false,
        wheel: 0.0,
        tracers: Vec::new(),
        feed: Vec::new(),
        menu: None,
        pending_fx: Vec::new(),
        flash_hold: 0.0,
        flash_fade: 0.0,
        flash_total_fade: 0.0,
        flash_alpha: 0.0,
    });
    // HUD: crosshair, status line, kill feed.
    commands.spawn((
        Text::new("+"),
        TextFont { font_size: bevy::text::FontSize::Px(22.0), ..default() },
        TextColor(Color::srgb(0.2, 1.0, 0.2)),
        Node { position_type: PositionType::Absolute, left: percent(50), top: percent(50), margin: UiRect { left: px(-6), top: px(-14), ..default() }, ..default() },
    ));
    commands.spawn((
        HudStatus,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(22.0), ..default() },
        TextColor(Color::srgb(1.0, 0.85, 0.3)),
        Node { position_type: PositionType::Absolute, left: px(16), bottom: px(12), ..default() },
    ));
    commands.spawn((
        HudMenu,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(20.0), ..default() },
        TextColor(Color::srgb(1.0, 0.85, 0.3)),
        Node { position_type: PositionType::Absolute, left: px(24), top: percent(30), ..default() },
    ));
    commands.spawn((
        HudScores,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(20.0), ..default() },
        TextColor(Color::WHITE),
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
        Node { position_type: PositionType::Absolute, left: percent(30), top: percent(20), ..default() },
    ));
    commands.spawn((
        HudFeed,
        Text::new(""),
        TextFont { font_size: bevy::text::FontSize::Px(18.0), ..default() },
        TextColor(Color::WHITE),
        Node { position_type: PositionType::Absolute, right: px(16), top: px(12), ..default() },
    ));
    // Player models from the install: one T and one CT look, posed standing and crouched.
    let mut looks = Vec::new();
    for name in ["arctic", "urban"] {
        let path = root.join(format!("cstrike/models/player/{name}/{name}.mdl"));
        let Ok(model) = goldsrc::mdl::Model::load(&path) else {
            warn!("missing player model {}", path.display());
            continue;
        };
        let textures: Vec<Handle<StandardMaterial>> = model
            .textures()
            .into_iter()
            .map(|t| {
                let image = images.add(rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb));
                // ⚠ Unlit until models are lit from the map's lightmaps like GoldSrc does.
                materials.add(StandardMaterial { base_color_texture: Some(image), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() })
            })
            .collect();
        let mut stance = |seq: &str| -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
            let Some(seq) = model.sequence(seq) else { return Vec::new() };
            model
                .posed_mesh(&model.pose(seq, 0, 0.5))
                .into_iter()
                .filter(|p| !p.indices.is_empty())
                .map(|p| {
                    let positions: Vec<[f32; 3]> = p.positions.iter().map(|v| map::to_y_up(*v)).collect();
                    let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uvs)
                        .with_inserted_indices(Indices::U32(p.indices))
                        .with_computed_normals();
                    (meshes.add(mesh), textures.get(p.texture).cloned().unwrap_or_default())
                })
                .collect()
        };
        let standing = stance("ref_aim_carbine");
        looks.push([standing, stance("crouch_aim_carbine")]);
    }
    commands.insert_resource(OtherPlayers { looks });

    // Grenade world models and effect materials.
    let mut grenade_parts = Vec::new();
    for name in ["w_hegrenade", "w_flashbang", "w_smokegrenade"] {
        let parts = goldsrc::mdl::Model::load(root.join(format!("cstrike/models/{name}.mdl")))
            .map(|m| model_parts(&m, 0, &mut meshes, &mut materials, &mut images))
            .unwrap_or_default();
        grenade_parts.push(parts);
    }
    commands.insert_resource(Effects {
        grenades: grenade_parts,
        sphere: meshes.add(Sphere::new(1.0)),
        // Double-sided: you can stand inside a smoke cloud.
        smoke_puff: materials.add(StandardMaterial { base_color: Color::srgba(0.55, 0.55, 0.55, 0.85), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() }),
    });
    // Full-screen white for flashbangs (invisible until flashed).
    commands.spawn((
        FlashOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        GlobalZIndex(10),
    ));
    commands.spawn((
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: 73.74_f32.to_radians(), // vertical FOV matching 90° horizontal at 4:3
            near: 4.0,
            far: 16384.0,
            ..default()
        }),
        // Exposure of 1.0, so lightmap values map straight to screen brightness.
        bevy::camera::Exposure { ev100: -(1.2_f32.log2()) },
        Tonemapping::None,
        Transform::from_translation(eye).with_rotation(Quat::from_rotation_y((yaw - 90.0).to_radians())),
        FlyCam { yaw: (yaw - 90.0).to_radians(), pitch: 0.0 },
    ));
}

/// A door's render entity; the value is its brush model index.
#[derive(Component)]
struct DoorMesh(usize);

/// GoldSrc (Z up) to Bevy (Y up) for direction vectors.
fn y_up(v: [f32; 3]) -> Vec3 {
    Vec3::from(map::to_y_up(v))
}

/// Where to draw a door: the same transform the sim uses for its collision.
fn door_transform(d: &sim::doors::Door) -> Transform {
    let (off, ang) = d.transform();
    let (f, r, u) = sim::doors::angle_vectors(ang);
    // Model space -> world: columns are where the model's x, z, -y axes end up.
    Transform {
        rotation: Quat::from_mat3(&Mat3::from_cols(y_up(f), y_up(u), y_up(r))),
        translation: y_up([d.pivot[0] + off[0], d.pivot[1] + off[1], d.pivot[2] + off[2]]),
        ..default()
    }
}

fn move_doors(w: Res<Walker>, mut doors: Query<(&DoorMesh, &mut Transform)>) {
    for (DoorMesh(model), mut tf) in &mut doors {
        if let Some(d) = w.client.world.doors.doors.iter().find(|d| d.model == *model) {
            *tf = door_transform(d);
        }
    }
}

#[derive(Component)]
struct FlyCam {
    yaw: f32,
    pitch: f32,
}

/// The connection to the server; it also predicts our own player.
#[derive(Resource)]
struct Walker {
    client: net::Client,
    /// Eye position at the previous tick, for smoothing between ticks.
    prev_eye: [f32; 3],
    /// V detaches the camera for free flight (the player stays put).
    noclip: bool,
    /// Weapon slot key held (to send each press once).
    slot_held: u8,
    drop_held: bool,
    /// Mouse wheel movement not yet turned into a weapon switch.
    wheel: f32,
    /// Bullet tracers: from, to, seconds left.
    tracers: Vec<([f32; 3], [f32; 3], f32)>,
    /// Kill feed lines and seconds left.
    feed: Vec<(String, f32)>,
    /// Weapon menu: closed (None), open on categories (Some(None)) or a category.
    menu: Option<Option<usize>>,
    /// Events for `grenade_effects` (explosions, flashes).
    pending_fx: Vec<net::protocol::GameEvent>,
    /// Flashbang: seconds of full white left, seconds of fade left (and total), strength.
    flash_hold: f32,
    flash_fade: f32,
    flash_total_fade: f32,
    flash_alpha: f32,
}

/// Meshes and materials for one posed studio model.
fn model_parts(
    model: &goldsrc::mdl::Model,
    seq: usize,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    images: &mut Assets<Image>,
) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
    let textures: Vec<Handle<StandardMaterial>> = model
        .textures()
        .into_iter()
        .map(|t| {
            let image = images.add(rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb));
            materials.add(StandardMaterial { base_color_texture: Some(image), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() })
        })
        .collect();
    model
        .posed_mesh(&model.pose(seq, 0, 0.5))
        .into_iter()
        .filter(|p| !p.indices.is_empty())
        .map(|p| {
            let positions: Vec<[f32; 3]> = p.positions.iter().map(|v| map::to_y_up(*v)).collect();
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uvs)
                .with_inserted_indices(Indices::U32(p.indices))
                .with_computed_normals();
            (meshes.add(mesh), textures.get(p.texture).cloned().unwrap_or_default())
        })
        .collect()
}

#[derive(Resource)]
struct Effects {
    /// HE, flash, smoke world models.
    grenades: Vec<Vec<(Handle<Mesh>, Handle<StandardMaterial>)>>,
    sphere: Handle<Mesh>,
    smoke_puff: Handle<StandardMaterial>,
}

#[derive(Component)]
struct GrenadeView;

/// An HE fireball: age in seconds.
#[derive(Component)]
struct Fireball(f32);

/// A smoke cloud's puffs, keyed by where the cloud is.
#[derive(Component)]
struct SmokeCloud([i32; 3]);

#[derive(Component)]
struct FlashOverlay;

/// Grenades in flight, fireballs, smoke clouds, and the flashbang white-out.
#[allow(clippy::too_many_arguments)]
fn grenade_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut w: ResMut<Walker>,
    fx: Res<Effects>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    views: Query<Entity, With<GrenadeView>>,
    mut fireballs: Query<(Entity, &mut Fireball, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    clouds: Query<(Entity, &SmokeCloud)>,
    mut overlay: Single<&mut BackgroundColor, With<FlashOverlay>>,
) {
    let dt = time.delta_secs();
    // Grenades in flight: few enough to redraw from scratch every frame.
    for e in &views {
        commands.entity(e).despawn();
    }
    for (kind, at) in w.client.grenades.clone() {
        let i = match kind {
            sim::grenades::Kind::He => 0,
            sim::grenades::Kind::Flash => 1,
            sim::grenades::Kind::Smoke => 2,
        };
        let parent = commands.spawn((GrenadeView, Transform::from_translation(Vec3::from(map::to_y_up(at))), Visibility::default())).id();
        for (mesh, mat) in &fx.grenades[i] {
            let child = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()))).id();
            commands.entity(parent).add_child(child);
        }
    }
    // Explosions and flashes from server events (handled here, the rest in `effects`).
    let me = w.client.id();
    for e in std::mem::take(&mut w.pending_fx) {
        match e {
            net::protocol::GameEvent::Explosion { at } => {
                let mat = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 0.6, 0.15, 0.9), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
                commands.spawn((Fireball(0.0), Mesh3d(fx.sphere.clone()), MeshMaterial3d(mat), Transform::from_translation(Vec3::from(map::to_y_up(at))).with_scale(Vec3::splat(20.0))));
            }
            net::protocol::GameEvent::Flash { blinded, .. } => {
                if let Some((_, b)) = blinded.iter().find(|(id, _)| Some(*id) == me) {
                    // A new flash never shortens one that's still on (simplified "max" rule).
                    w.flash_hold = w.flash_hold.max(b.hold);
                    w.flash_fade = w.flash_fade.max(b.fade);
                    w.flash_alpha = w.flash_alpha.max(b.alpha as f32 / 255.0);
                    w.flash_total_fade = w.flash_fade;
                }
            }
            _ => {}
        }
    }
    for (e, mut f, mut tf, mat) in &mut fireballs {
        f.0 += dt;
        let t = f.0 / 0.35;
        tf.scale = Vec3::splat(20.0 + 130.0 * t.min(1.0));
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color.set_alpha(0.9 * (1.0 - t).max(0.0));
        }
        if t >= 1.0 {
            commands.entity(e).despawn();
        }
    }
    // Flashbang white-out: full for `hold` seconds, then fades out over `fade`.
    let a = if w.flash_hold > 0.0 {
        w.flash_hold -= dt;
        w.flash_alpha
    } else if w.flash_fade > 0.0 {
        w.flash_fade -= dt;
        w.flash_alpha * (w.flash_fade / w.flash_total_fade.max(0.01)).max(0.0)
    } else {
        w.flash_alpha = 0.0;
        0.0
    };
    overlay.0 = Color::srgba(1.0, 1.0, 1.0, a);
    // Smoke clouds: a cluster of grey puffs, thinning out in the last 3 seconds.
    let key = |at: [f32; 3]| at.map(|v| v.round() as i32);
    let live: Vec<_> = w.client.smokes.iter().map(|s| (key(s.at), *s)).collect();
    for (e, c) in &clouds {
        if !live.iter().any(|(k, _)| *k == c.0) {
            commands.entity(e).despawn();
        }
    }
    for (k, smoke) in &live {
        if clouds.iter().any(|(_, c)| c.0 == *k) {
            continue;
        }
        let centre = Vec3::from(map::to_y_up(smoke.at));
        let parent = commands.spawn((SmokeCloud(*k), Transform::from_translation(centre), Visibility::default())).id();
        for i in 0..14 {
            let a = i as f32 * 2.4;
            let r = 30.0 + (i % 4) as f32 * 22.0;
            let offset = Vec3::new(a.cos() * r, 20.0 + (i % 3) as f32 * 35.0, a.sin() * r);
            let child = commands.spawn((Mesh3d(fx.sphere.clone()), MeshMaterial3d(fx.smoke_puff.clone()), Transform::from_translation(offset).with_scale(Vec3::splat(70.0)))).id();
            commands.entity(parent).add_child(child);
        }
    }
}

/// Player model meshes: [look][standing, crouched] -> parts.
#[derive(Resource)]
struct OtherPlayers {
    looks: Vec<[Vec<(Handle<Mesh>, Handle<StandardMaterial>)>; 2]>,
}

/// Another player's model; `crouched` is the stance its children were built for.
#[derive(Component)]
struct OtherPlayer {
    id: u8,
    crouched: bool,
}

/// Each other player's model, where the server (interpolated) says they are.
fn draw_others(mut commands: Commands, w: Res<Walker>, look: Res<OtherPlayers>, mut drawn: Query<(Entity, &OtherPlayer, &mut Transform)>) {
    let others: Vec<_> = w.client.others().into_iter().filter(|o| o.alive).collect();
    let place = |o: &net::client::Remote| {
        Transform::from_translation(Vec3::from(map::to_y_up(o.origin))).with_rotation(Quat::from_rotation_y(o.yaw.to_radians()))
    };
    for (e, p, mut tf) in &mut drawn {
        match others.iter().find(|o| o.id == p.id) {
            // Stance changed: rebuild with the other pose.
            Some(o) if o.crouched != p.crouched => commands.entity(e).despawn(),
            Some(o) => *tf = place(o),
            None => {
                info!("player {} left", p.id);
                commands.entity(e).despawn();
            }
        }
    }
    for o in others.iter().filter(|o| !drawn.iter().any(|(_, d, _)| d.id == o.id && d.crouched == o.crouched)) {
        if !drawn.iter().any(|(_, d, _)| d.id == o.id) {
            info!("player {} is in the game at {:?}", o.id, o.origin);
        }
        let Some(parts) = look.looks.get(o.id as usize % look.looks.len().max(1)) else { continue };
        let parent = commands.spawn((OtherPlayer { id: o.id, crouched: o.crouched }, place(o), Visibility::default())).id();
        for (mesh, material) in &parts[o.crouched as usize] {
            let child = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()))).id();
            commands.entity(parent).add_child(child);
        }
    }
}

fn mouse_look(
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    w: Res<Walker>,
    mut cam: Single<(&mut Transform, &mut FlyCam, &mut Projection)>,
) {
    let (ref mut tf, ref mut fc, ref mut proj) = *cam;
    // Zoomed weapons slow the mouse down in proportion, like CS.
    let fov = w.client.state.loadout.active_gun().map(|g| g.fov()).unwrap_or(90) as f32;
    let sens = 0.002 * fov / 90.0;
    if cursor.grab_mode != CursorGrabMode::None {
        fc.yaw -= motion.delta.x * sens;
        fc.pitch = (fc.pitch - motion.delta.y * sens).clamp(-1.55, 1.55);
    }
    // Recoil punch moves the view (GoldSrc pitch is positive downwards).
    let punch = w.client.state.loadout.punch;
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fc.yaw + punch[1].to_radians(), fc.pitch - punch[0].to_radians(), punch[2].to_radians());
    // Horizontal FOV (90 = normal) at 4:3, as a vertical FOV.
    if let Projection::Perspective(p) = &mut **proj {
        p.fov = 2.0 * ((fov.to_radians() / 2.0).tan() * 0.75).atan();
    }
}

/// One simulation tick from the keyboard and mouse. WASD move, Space jump, Ctrl crouch,
/// Shift walk, E use, mouse fire, R reload, 1-5 weapon slots.
fn walk(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    wheel: Res<bevy::input::mouse::AccumulatedMouseScroll>,
    cursor: Single<&CursorOptions>,
    cam: Single<&FlyCam>,
    mut w: ResMut<Walker>,
) {
    // Mouse buttons only count while the mouse is captured (the first click just captures it).
    let captured = cursor.grab_mode != CursorGrabMode::None;
    // Automated screenshot runs ignore the keyboard too (the window may pop up while you type).
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    let keys = if auto { &ButtonInput::<KeyCode>::default() } else { &*keys };
    let slot_keys = [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5];
    // While the buy menu is open, number keys belong to the menu.
    let mut slot = if w.menu.is_some() { 0 } else { slot_keys.iter().position(|k| keys.pressed(*k)).map(|i| i as u8 + 1).unwrap_or(0) };
    if keys.pressed(KeyCode::KeyQ) {
        slot = sim::pmove::SLOT_LAST;
    }
    // Mouse wheel: up = previous weapon, down = next (CS invprev / invnext).
    let scroll = if captured && !auto { wheel.delta.y } else { 0.0 };
    w.wheel += scroll;
    // Several ticks can run per frame: send a slot press once.
    let held = slot;
    let mut slot = if slot == w.slot_held { 0 } else { slot };
    w.slot_held = held;
    if slot == 0 && w.wheel.abs() >= 1.0 {
        slot = if w.wheel > 0.0 { sim::pmove::SLOT_PREV } else { sim::pmove::SLOT_NEXT };
        w.wheel = 0.0;
    }
    let drop = keys.pressed(KeyCode::KeyG) && !w.drop_held;
    w.drop_held = keys.pressed(KeyCode::KeyG);
    let axis = |pos: KeyCode, neg: KeyCode| 400.0 * (keys.pressed(pos) as i32 - keys.pressed(neg) as i32) as f32;
    let input = sim::pmove::Input {
        forward: axis(KeyCode::KeyW, KeyCode::KeyS),
        side: axis(KeyCode::KeyD, KeyCode::KeyA),
        // Bevy yaw 0 looks down -Z, which is GoldSrc yaw 90.
        yaw: cam.yaw.to_degrees() + 90.0,
        // Bevy pitch is positive looking up; GoldSrc is positive looking down.
        pitch: -cam.pitch.to_degrees(),
        jump: keys.pressed(KeyCode::Space),
        crouch: keys.pressed(KeyCode::ControlLeft),
        walk: keys.pressed(KeyCode::ShiftLeft),
        use_key: keys.pressed(KeyCode::KeyE),
        attack: captured && mouse.pressed(MouseButton::Left),
        attack2: captured && mouse.pressed(MouseButton::Right),
        reload: keys.pressed(KeyCode::KeyR),
        slot,
        drop,
    };
    // Free flight: keep the connection alive but stand still.
    let input = if w.noclip { sim::pmove::Input { yaw: input.yaw, pitch: input.pitch, ..default() } } else { input };
    w.prev_eye = w.client.me().eye();
    let (_events, fired) = w.client.tick(input);
    // Our own shots show at once (predicted); other players' come from server events.
    for shot in fired.shots {
        let eye = w.client.me().eye();
        let (f, _, _) = sim::weapons::bullets::view_vectors(shot.angles[0], shot.angles[1]);
        let end = w.client.world.trace_line(eye, std::array::from_fn(|i| eye[i] + f[i] * 8192.0)).end;
        w.tracers.push((eye, end, 0.08));
    }
}

fn move_camera(
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut w: ResMut<Walker>,
    mut tf: Single<&mut Transform, With<FlyCam>>,
) {
    if keys.just_pressed(KeyCode::KeyV) {
        w.noclip = !w.noclip;
    }
    // Dead (or forced with BOOTLEG16_SPECTATE): watch another player from in front of them.
    let spectate = !w.client.vitals.alive() || std::env::var_os("BOOTLEG16_SPECTATE").is_some();
    if spectate && !w.noclip {
        if let Some(o) = w.client.others().into_iter().find(|o| o.alive) {
            let eye = [o.origin[0], o.origin[1], o.origin[2] + 20.0];
            let (s, c) = o.yaw.to_radians().sin_cos();
            // Chase view from behind (like CS); in front if there's no room. Never in a wall.
            let spot = [-1.0f32, 1.0]
                .iter()
                .map(|side| {
                    let want = [eye[0] + c * 100.0 * side, eye[1] + s * 100.0 * side, eye[2] + 10.0];
                    let t = w.client.world.trace_line(eye, want);
                    (t.fraction, [eye[0] + (want[0] - eye[0]) * t.fraction * 0.9, eye[1] + (want[1] - eye[1]) * t.fraction * 0.9, eye[2] + (want[2] - eye[2]) * t.fraction * 0.9])
                })
                .find(|(f, _)| *f > 0.6)
                .map(|(_, p)| p)
                .unwrap_or(eye);
            tf.translation = Vec3::from(map::to_y_up(spot));
            tf.look_at(Vec3::from(map::to_y_up(eye)), Vec3::Y);
            debug!("spectating {} from {spot:?} at {eye:?} (yaw {})", o.id, o.yaw);
            return;
        }
    }
    if !w.noclip {
        if w.client.id().is_none() {
            return; // still connecting
        }
        let a = fixed.overstep_fraction();
        let (p, c) = (Vec3::from(map::to_y_up(w.prev_eye)), Vec3::from(map::to_y_up(w.client.me().eye())));
        tf.translation = p.lerp(c, a);
        return;
    }
    let mut dir = Vec3::ZERO;
    for (k, d) in [
        (KeyCode::KeyW, tf.forward().as_vec3()),
        (KeyCode::KeyS, tf.back().as_vec3()),
        (KeyCode::KeyA, tf.left().as_vec3()),
        (KeyCode::KeyD, tf.right().as_vec3()),
        (KeyCode::Space, Vec3::Y),
        (KeyCode::ControlLeft, Vec3::NEG_Y),
    ] {
        if keys.pressed(k) {
            dir += d;
        }
    }
    let speed = if keys.pressed(KeyCode::ShiftLeft) { 1200.0 } else { 400.0 };
    tf.translation += dir.normalize_or_zero() * speed * time.delta_secs();
}
/// One buy menu entry.
#[derive(Clone)]
enum Entry {
    /// Buys straight away from the category list (6 and 7: ammo).
    Direct(&'static str, net::protocol::BuyItem),
    Category(&'static str, Vec<(&'static str, net::protocol::BuyItem)>),
}

/// The CS 1.6 buy menu (B). Terrorists and counter-terrorists see different guns.
fn buy_menu_layout(ct: bool) -> Vec<Entry> {
    use net::protocol::BuyItem::*;
    use sim::weapons::Id::*;
    let side = |t: (&'static str, sim::weapons::Id), c: (&'static str, sim::weapons::Id)| if ct { (c.0, Weapon(c.1)) } else { (t.0, Weapon(t.1)) };
    let mut equipment = vec![("Kevlar", Kevlar), ("Kevlar + Helmet", KevlarHelmet), ("Flashbang", Weapon(Flashbang)), ("HE Grenade", Weapon(HeGrenade)), ("Smoke Grenade", Weapon(Smoke))];
    if ct {
        equipment.push(("Defuse Kit", DefuseKit));
    }
    equipment.push(("NightVision", NightVision));
    vec![
        Entry::Category(
            "Pistols",
            vec![("Glock-18", Weapon(Glock)), ("USP", Weapon(Usp)), ("P228", Weapon(P228)), ("Desert Eagle", Weapon(Deagle)), side(("Dual Elites", Elite), ("Five-SeveN", FiveSeven))],
        ),
        Entry::Category("Shotguns", vec![("M3", Weapon(M3)), ("XM1014", Weapon(Xm1014))]),
        Entry::Category("Sub-Machine Guns", vec![side(("MAC-10", Mac10), ("TMP", Tmp)), ("MP5", Weapon(Mp5)), ("UMP45", Weapon(Ump45)), ("P90", Weapon(P90))]),
        Entry::Category(
            "Rifles",
            if ct {
                vec![("FAMAS", Weapon(Famas)), ("Scout", Weapon(Scout)), ("M4A1", Weapon(M4a1)), ("AUG", Weapon(Aug)), ("SG550", Weapon(Sg550)), ("AWP", Weapon(Awp))]
            } else {
                vec![("Galil", Weapon(Galil)), ("AK-47", Weapon(Ak47)), ("Scout", Weapon(Scout)), ("SG552", Weapon(Sg552)), ("AWP", Weapon(Awp)), ("G3SG1", Weapon(G3sg1))]
            },
        ),
        Entry::Category("Machine Gun", vec![("M249", Weapon(M249))]),
        Entry::Direct("Primary Ammo", PrimaryAmmo),
        Entry::Direct("Secondary Ammo", SecondaryAmmo),
        Entry::Category("Equipment", equipment),
    ]
}

impl Entry {
    fn name(&self) -> &'static str {
        match self {
            Entry::Direct(n, _) | Entry::Category(n, _) => n,
        }
    }
}

#[derive(Component)]
struct HudMenu;

#[derive(Component)]
struct HudScores;

/// CS 1.6 buy menu: B opens/closes, a number picks, 0 cancels, buying closes it.
/// `,` / `.` buy primary / secondary ammo without the menu.
fn buy_menu(keys: Res<ButtonInput<KeyCode>>, mut w: ResMut<Walker>, mut text: Single<&mut Text, With<HudMenu>>) {
    let auto = std::env::var_os("BOOTLEG16_SCREENSHOT").is_some();
    if auto && std::env::var_os("BOOTLEG16_MENU").is_none() {
        return;
    }
    // Counter-terrorists are the odd player numbers until teams exist (as for the spawn pistol).
    let ct = w.client.id().is_some_and(|id| id % 2 == 1);
    let layout = buy_menu_layout(ct);
    let digits = [KeyCode::Digit0, KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3, KeyCode::Digit4, KeyCode::Digit5, KeyCode::Digit6, KeyCode::Digit7, KeyCode::Digit8, KeyCode::Digit9];
    let pressed = digits.iter().position(|k| keys.just_pressed(*k));
    if keys.just_pressed(KeyCode::Comma) {
        w.client.buy(net::protocol::BuyItem::PrimaryAmmo);
    }
    if keys.just_pressed(KeyCode::Period) {
        w.client.buy(net::protocol::BuyItem::SecondaryAmmo);
    }
    if keys.just_pressed(KeyCode::KeyB) {
        w.menu = if w.menu.is_some() { None } else { Some(None) };
    } else if let (Some(state), Some(n)) = (w.menu, pressed) {
        if n == 0 {
            w.menu = None; // 0. Cancel
        } else {
            let i = n - 1;
            match state {
                None => match layout.get(i) {
                    Some(Entry::Category(..)) => w.menu = Some(Some(i)),
                    Some(Entry::Direct(_, item)) => {
                        w.client.buy(*item);
                        w.menu = None;
                    }
                    None => {}
                },
                Some(cat) => {
                    if let Some(Entry::Category(_, items)) = layout.get(cat) {
                        if let Some((_, item)) = items.get(i) {
                            w.client.buy(*item);
                            w.menu = None;
                        }
                    }
                }
            }
        }
    }
    if auto && w.menu.is_none() {
        w.menu = Some(Some(3)); // screenshot check: show the rifles page
    }
    text.0 = match w.menu {
        None => String::new(),
        Some(None) => {
            let mut t = format!("Buy Menu ({}, free for now)\n", if ct { "CT" } else { "T" });
            for (i, e) in layout.iter().enumerate() {
                t += &format!("\n{}. {}", i + 1, e.name());
            }
            t + "\n\n0. Cancel"
        }
        Some(Some(cat)) => match layout.get(cat) {
            Some(Entry::Category(name, items)) => {
                let mut t = format!("{name}\n");
                for (i, (item, _)) in items.iter().enumerate() {
                    t += &format!("\n{}. {item}", i + 1);
                }
                t + "\n\n0. Cancel"
            }
            _ => String::new(),
        },
    };
}

/// Tab: scoreboard while held.
fn scoreboard(keys: Res<ButtonInput<KeyCode>>, w: Res<Walker>, mut text: Single<&mut Text, With<HudScores>>) {
    text.0 = if keys.pressed(KeyCode::Tab) {
        let mut t = format!("{:<24} {:>6} {:>6}\n", "Player", "Kills", "Deaths");
        for s in &w.client.scores {
            let me = if Some(s.id) == w.client.id() { " (you)" } else { "" };
            let dead = if s.alive { "" } else { "  dead" };
            t += &format!("\n{:<24} {:>6} {:>6}{dead}", format!("{}{me}", s.name), s.kills, s.deaths);
        }
        t
    } else {
        String::new()
    };
}

#[derive(Component)]
struct HudStatus;

#[derive(Component)]
struct HudFeed;

/// Tracers and the kill feed from server events.
fn effects(time: Res<Time>, mut w: ResMut<Walker>, mut gizmos: Gizmos) {
    let me = w.client.id();
    for e in w.client.take_events() {
        match e {
            net::protocol::GameEvent::Shot { shooter, from, to, .. } if Some(shooter) != me => w.tracers.push((from, to, 0.08)),
            net::protocol::GameEvent::Kill { killer, victim, weapon, headshot } => {
                let name = |id: u8| if Some(id) == me { "you".to_string() } else { format!("player {id}") };
                let line = match weapon {
                    None => format!("{} fell", name(victim)),
                    Some(w) => format!("{} [{}{}] {}", name(killer), sim::weapons::def(w).name, if headshot { " HS" } else { "" }, name(victim)),
                };
                w.feed.push((line, 6.0));
            }
            e @ (net::protocol::GameEvent::Explosion { .. } | net::protocol::GameEvent::Flash { .. }) => w.pending_fx.push(e),
            _ => {}
        }
    }
    let dt = time.delta_secs();
    for (from, to, left) in &mut w.tracers {
        gizmos.line(Vec3::from(map::to_y_up(*from)), Vec3::from(map::to_y_up(*to)), Color::srgba(1.0, 0.9, 0.5, 0.8));
        *left -= dt;
    }
    w.tracers.retain(|t| t.2 > 0.0);
    for (_, left) in &mut w.feed {
        *left -= dt;
    }
    w.feed.retain(|f| f.1 > 0.0);
    let n = w.feed.len();
    if n > 5 {
        w.feed.drain(..n - 5);
    }
}

fn hud(w: Res<Walker>, mut status: Single<&mut Text, (With<HudStatus>, Without<HudFeed>)>, mut feed: Single<&mut Text, (With<HudFeed>, Without<HudStatus>)>) {
    let v = w.client.vitals;
    let l = &w.client.state.loadout;
    let weapon = sim::weapons::def(l.active_id()).name;
    let ammo = l.active_gun().map(|g| format!("{} / {}", g.clip, g.reserve)).unwrap_or_default();
    let [he, fl, sm] = l.grenades;
    let nades = if he + fl + sm > 0 { format!("      HE {he}  flash {fl}  smoke {sm}") } else { String::new() };
    status.0 = if v.alive() {
        format!("+ {:.0}   armor {:.0}{}      {weapon}  {ammo}{nades}", v.health, v.armor, if v.helmet { " (helmet)" } else { "" })
    } else {
        "dead - respawning".into()
    };
    feed.0 = w.feed.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
}

/// Click to capture the mouse, Escape to release it.
fn grab_cursor(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    // Automated screenshot runs never take the user's mouse.
    if mouse.just_pressed(MouseButton::Left) && std::env::var_os("BOOTLEG16_SCREENSHOT").is_none() {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

/// With BOOTLEG16_SCREENSHOT set: save a frame to that path after ~2 s (or
/// BOOTLEG16_SCREENSHOT_AT seconds), then quit.
fn screenshot(mut commands: Commands, time: Res<Time>, mut taken: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::var_os("BOOTLEG16_SCREENSHOT") else { return };
    let at: f32 = std::env::var("BOOTLEG16_SCREENSHOT_AT").ok().and_then(|f| f.parse().ok()).unwrap_or(2.0);
    let now = time.elapsed_secs();
    match *taken {
        None if now >= at => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(path)));
            *taken = Some(now);
        }
        Some(t) if now >= t + 1.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A door drawn with `door_transform` must sit exactly where its collision is.
    #[test]
    fn rendered_door_matches_collision_on_inferno() {
        let path = game_dir().join("cstrike/maps/de_inferno.bsp");
        let Ok(bsp) = goldsrc::bsp::Bsp::load(&path) else { return };
        let mut doors = sim::doors::Doors::from_bsp(&bsp);
        let actor = sim::doors::Actor { mins: [0.0; 3], maxs: [0.0; 3], eye: [0.0; 3], forward: [1.0, 0.0, 0.0], using: false };
        // Fire it open by touching it, then let it swing partway.
        let (lo, hi) = doors.doors[0].bounds();
        let toucher = sim::doors::Actor { mins: lo, maxs: hi, ..actor };
        doors.update(1.0 / 128.0, &[toucher], |_, _| false);
        for _ in 0..40 {
            doors.update(1.0 / 128.0, &[], |_, _| false);
        }
        let d = &doors.doors[0];
        assert!(d.transform().1[1].abs() > 1.0, "door did not move");
        let (off, ang) = d.transform();
        let (f, r, u) = sim::doors::angle_vectors(ang);
        let tf = door_transform(d);
        for corner in [d.mins, d.maxs, [d.mins[0], d.maxs[1], d.mins[2]]] {
            let sim_world = sim::doors::local_to_world(corner, f, r, u);
            let sim_world = [sim_world[0] + d.pivot[0] + off[0], sim_world[1] + d.pivot[1] + off[1], sim_world[2] + d.pivot[2] + off[2]];
            let drawn = tf.transform_point(Vec3::from(map::to_y_up(corner)));
            assert!(drawn.distance(Vec3::from(map::to_y_up(sim_world))) < 0.01, "{drawn} vs {sim_world:?}");
        }
    }
}
