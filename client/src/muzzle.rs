//! Muzzle flashes with CS 1.6's own sprites (`muzzleflash1-4.spr`), for your own gun
//! (`viewmodel.rs`) and other players' (`players.rs`). Each gun's view model says which and how
//! big in its muzzle flash event (5001): the last digit is the sprite, the tens a tenth of its
//! size ("32": muzzleflash3 at 0.3); the engine draws one random frame of it, added on top.
use crate::level::rgba_image;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use sim::weapons::Id;

pub struct MuzzlePlugin;

impl Plugin for MuzzlePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load);
    }
}

/// How long a flash shows (CS's are a frame or two).
pub const FLASH_SECONDS: f32 = 0.045;

#[derive(Resource)]
pub struct FlashLook {
    pub quad: Handle<Mesh>,
    /// A material per frame of each sprite, and each sprite's width (units at scale 1).
    pub sprites: Vec<(Vec<Handle<StandardMaterial>>, f32)>,
    /// Shown until the sprites are known.
    pub material: Handle<StandardMaterial>,
}

impl FlashLook {
    /// The flash for a gun: one random frame (`pick` in 0..1) and its width.
    pub fn pick(&self, weapon: Id, pick: f32) -> (Handle<StandardMaterial>, f32) {
        let (sprite, scale) = style(weapon);
        match self.sprites.get(sprite.min(self.sprites.len().saturating_sub(1))) {
            Some((frames, width)) if !frames.is_empty() => (frames[((pick * frames.len() as f32) as usize).min(frames.len() - 1)].clone(), width * scale),
            _ => (self.material.clone(), 12.0),
        }
    }
}

/// Each gun's flash from its view model's event 5001 options: (sprite 0-3, size).
fn style(weapon: Id) -> (usize, f32) {
    use Id::*;
    let opt = match weapon {
        Ak47 | M4a1 | P90 => 32,
        Awp | Mp5 | Ump45 => 31,
        Deagle => 24,
        Galil | Famas => 22,
        Glock | Usp | P228 | Scout => 21,
        Mac10 => 11,
        _ => 21,
    };
    let (digit, tens) = (opt % 10, opt / 10);
    (digit.max(1) - 1, if tens == 0 { 0.5 } else { tens as f32 * 0.1 })
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let add = |img: Handle<Image>, materials: &mut Assets<StandardMaterial>| materials.add(StandardMaterial { base_color_texture: Some(img), unlit: true, alpha_mode: AlphaMode::Add, ..default() });
    // Additive: the texels are display values (see `explosions`), so no sRGB decode.
    let sprites = (1..=4)
        .filter_map(|i| crate::gamefiles::paths(&format!("sprites/muzzleflash{i}.spr")).iter().find_map(|p| goldsrc::spr::Sprite::load(p).ok()))
        .map(|s| (s.frames.into_iter().map(|f| add(images.add(rgba_image(s.width, s.height, f, false, TextureFormat::Rgba8Unorm)), &mut materials)).collect(), s.width as f32))
        .collect();
    let material = add(images.add(rgba_image(1, 1, vec![255, 200, 120, 255], false, TextureFormat::Rgba8Unorm)), &mut materials);
    commands.insert_resource(FlashLook { quad: meshes.add(Rectangle::new(1.0, 1.0)), sprites, material });
}

/// A hidden flash quad (shown and placed by its owner when firing).
pub fn spawn(commands: &mut Commands, look: &FlashLook) -> Entity {
    commands.spawn((Mesh3d(look.quad.clone()), MeshMaterial3d(look.material.clone()), Transform::default(), Visibility::Hidden)).id()
}

/// Whether this weapon flashes (not knives, grenades, the bomb, silenced guns or the TMP,
/// whose CS model has no muzzle flash).
pub fn flashes(weapon: Id, silenced: bool) -> bool {
    sim::weapons::is_gun(weapon) && !silenced && weapon != Id::Tmp
}
