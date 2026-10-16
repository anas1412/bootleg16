//! Muzzle flashes: a short, bright, additive star at the gun's muzzle (the model's first
//! attachment), for your own gun (`viewmodel.rs`) and other players' (`players.rs`).
use crate::level::rgba_image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

pub struct MuzzlePlugin;

impl Plugin for MuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load);
    }
}

/// How long a flash shows (CS's are one or two frames).
pub const FLASH_SECONDS: f32 = 0.045;

#[derive(Resource)]
pub struct FlashLook {
    pub quad: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
}

/// A hot centre with a few soft spikes: yellow-white fading to orange.
fn flash_image(n: u32) -> Image {
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = (dx * dx + dy * dy).sqrt() / c;
            let ang = dy.atan2(dx);
            let spikes = 0.55 + 0.45 * (ang * 3.0).cos().abs().powf(6.0);
            let a = ((spikes - r) / spikes).clamp(0.0, 1.0).powf(1.5);
            let hot = (1.0 - r * 1.6).clamp(0.0, 1.0);
            let col = [255.0, 200.0 + 55.0 * hot, 90.0 + 150.0 * hot];
            px.extend_from_slice(&[col[0] as u8, col[1] as u8, col[2] as u8, (a * 255.0) as u8]);
        }
    }
    rgba_image(n, n, px, false, TextureFormat::Rgba8UnormSrgb)
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let material = materials.add(StandardMaterial { base_color_texture: Some(images.add(flash_image(64))), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    commands.insert_resource(FlashLook { quad: meshes.add(Rectangle::new(1.0, 1.0)), material });
}

/// A hidden flash quad (shown and placed by its owner when firing).
pub fn spawn(commands: &mut Commands, look: &FlashLook) -> Entity {
    commands.spawn((Mesh3d(look.quad.clone()), MeshMaterial3d(look.material.clone()), Transform::default(), Visibility::Hidden)).id()
}

/// Whether this weapon flashes (not knives, grenades, the bomb or silenced guns).
pub fn flashes(weapon: sim::weapons::Id, silenced: bool) -> bool {
    sim::weapons::is_gun(weapon) && !silenced
}
