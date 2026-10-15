//! Map viewer: loads a GoldSrc map from the user's install and walks around it
//! with CS 1.6 movement (V toggles free flight).
//!
//! Usage: `client [map]` (default de_dust2). The game directory comes from
//! `BOOTLEG16_DIR` (default: the Steam Half-Life install). Set `BOOTLEG16_SCREENSHOT=path.png`
//! to save one frame and exit (used for automated checks).
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
            primary_window: Some(Window { title: "bootleg1.6".into(), ..default() }),
            ..default()
        }))
        // GoldSrc lighting is fully baked into lightmaps; no extra ambient.
        .insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() })
        .insert_resource(ClearColor(Color::srgb(0.45, 0.6, 0.8)))
        // The movement simulation runs at a fixed 128 ticks per second, like the server will.
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .add_systems(Startup, setup)
        .add_systems(FixedUpdate, walk)
        .add_systems(Update, (mouse_look, move_camera, move_doors, grab_cursor, screenshot).chain())
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
    let name = std::env::args().nth(1).unwrap_or_else(|| "de_dust2".into());
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
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD)
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
    let world = sim::collision::World::from_bsp(&goldsrc::bsp::Bsp::load(&path).expect("map loaded above"));
    let player = sim::pmove::Player::new(map::from_y_up(pos));
    let eye = Vec3::from(map::to_y_up(player.eye()));
    commands.insert_resource(Walker { world, player, prev_eye: player.eye(), noclip: false, use_held: false });
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
        if let Some(d) = w.world.doors.doors.iter().find(|d| d.model == *model) {
            *tf = door_transform(d);
        }
    }
}

#[derive(Component)]
struct FlyCam {
    yaw: f32,
    pitch: f32,
}

/// The local player, simulated with the same code the server will run.
#[derive(Resource)]
struct Walker {
    world: sim::collision::World,
    player: sim::pmove::Player,
    /// Eye position at the previous tick, for smoothing between ticks.
    prev_eye: [f32; 3],
    /// V toggles free flying (no collision).
    noclip: bool,
    use_held: bool,
}

fn mouse_look(motion: Res<AccumulatedMouseMotion>, cursor: Single<&CursorOptions>, mut cam: Single<(&mut Transform, &mut FlyCam)>) {
    let (ref mut tf, ref mut fc) = *cam;
    if cursor.grab_mode != CursorGrabMode::None {
        fc.yaw -= motion.delta.x * 0.002;
        fc.pitch = (fc.pitch - motion.delta.y * 0.002).clamp(-1.55, 1.55);
    }
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fc.yaw, fc.pitch, 0.0);
}

/// One simulation tick from the keyboard. WASD move, Space jump, Ctrl crouch, Shift walk, E use.
fn walk(keys: Res<ButtonInput<KeyCode>>, cam: Single<&FlyCam>, mut w: ResMut<Walker>) {
    if w.noclip {
        return;
    }
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
    };
    let w = &mut *w;
    w.prev_eye = w.player.eye();
    let _events = sim::pmove::step(&mut w.player, &input, 1.0 / 128.0, &w.world, &sim::pmove::MoveVars::default());
    let h = w.player.hull();
    let o = w.player.origin;
    let (s, c) = ((-cam.pitch).sin(), (-cam.pitch).cos());
    let (sy, cy) = input.yaw.to_radians().sin_cos();
    let actor = sim::doors::Actor {
        mins: [o[0] + h.mins()[0], o[1] + h.mins()[1], o[2] + h.mins()[2]],
        maxs: [o[0] + h.maxs()[0], o[1] + h.maxs()[1], o[2] + h.maxs()[2]],
        eye: w.player.eye(),
        forward: [c * cy, c * sy, -s],
        // Several ticks can run per frame, so take the key edge per tick.
        using: keys.pressed(KeyCode::KeyE) && !w.use_held,
    };
    w.use_held = keys.pressed(KeyCode::KeyE);
    w.world.update(1.0 / 128.0, &[actor]);
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
        if !w.noclip {
            // Drop the player where the camera is.
            let feet = map::from_y_up(tf.translation.into());
            w.player = sim::pmove::Player::new([feet[0], feet[1], feet[2] - sim::pmove::VIEW_HEIGHT_STANDING]);
            w.prev_eye = w.player.eye();
        }
    }
    if !w.noclip {
        let a = fixed.overstep_fraction();
        let (p, c) = (Vec3::from(map::to_y_up(w.prev_eye)), Vec3::from(map::to_y_up(w.player.eye())));
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
/// Click to capture the mouse, Escape to release it.
fn grab_cursor(
    mut cursor: Single<&mut CursorOptions>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse.just_pressed(MouseButton::Left) {
        cursor.grab_mode = CursorGrabMode::Locked;
        cursor.visible = false;
    }
    if keys.just_pressed(KeyCode::Escape) {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

/// With BOOTLEG16_SCREENSHOT set: save frame 120 to that path, then quit.
fn screenshot(mut commands: Commands, mut frame: Local<u32>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::var_os("BOOTLEG16_SCREENSHOT") else { return };
    *frame += 1;
    if *frame == 120 {
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(path)));
    }
    if *frame == 180 {
        exit.write(AppExit::Success);
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
