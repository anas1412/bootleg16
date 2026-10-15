//! Map viewer: loads a GoldSrc map from the user's install and flies around it.
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
        .add_systems(Startup, setup)
        .add_systems(Update, (fly_camera, grab_cursor, screenshot))
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

    for b in geo.batches {
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
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(material),
            Lightmap { image: lightmap.clone(), uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0), ..default() },
        ));
    }

    let (pos, yaw) = geo.spawns.first().copied().unwrap_or(([0.0; 3], 0.0));
    let eye = Vec3::from(pos) + Vec3::Y * 28.0; // standing eye height above the spawn origin
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

#[derive(Component)]
struct FlyCam {
    yaw: f32,
    pitch: f32,
}

fn fly_camera(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    cursor: Single<&CursorOptions>,
    mut cam: Single<(&mut Transform, &mut FlyCam)>,
) {
    let (ref mut tf, ref mut fc) = *cam;
    if cursor.grab_mode != CursorGrabMode::None {
        fc.yaw -= motion.delta.x * 0.002;
        fc.pitch = (fc.pitch - motion.delta.y * 0.002).clamp(-1.55, 1.55);
    }
    tf.rotation = Quat::from_euler(EulerRot::YXZ, fc.yaw, fc.pitch, 0.0);
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
