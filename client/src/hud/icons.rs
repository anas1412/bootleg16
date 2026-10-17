//! HUD icons: our own white PNGs (`gfx/hud/<name>.png`, tinted by the game), with the install's
//! `sprites/hud.txt` sprites as the fallback, and which icon each weapon uses.
use crate::level::rgba_image;
use crate::ui::vh;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use goldsrc::spr::{hud_table, HudEntry, Sprite};
use sim::weapons::Id;
use std::collections::HashMap;

/// The HUD's icon sheets and where each icon sits on them.
#[derive(Resource)]
pub struct Icons {
    table: HashMap<String, HudEntry>,
    sheets: HashMap<String, Handle<Image>>,
    /// Our own icons: the picture and its aspect.
    pictures: HashMap<String, (Handle<Image>, f32)>,
}

impl FromWorld for Icons {
    fn from_world(world: &mut World) -> Self {
        let pictures = load_pictures(world);
        // From the game data folder.
        let dirs = crate::gamefiles::paths("sprites");
        let read = |file: &str| dirs.iter().find_map(|d| std::fs::read_to_string(d.join(file)).ok());
        let Some(text) = read("hud.txt") else { return Icons { table: HashMap::new(), sheets: HashMap::new(), pictures } };
        let table = hud_table(&text);
        let mut images = world.resource_mut::<Assets<Image>>();
        let mut sheets = HashMap::new();
        for e in table.values() {
            if sheets.contains_key(&e.sprite) {
                continue;
            }
            if let Some(s) = dirs.iter().find_map(|d| Sprite::load(d.join(format!("{}.spr", e.sprite))).ok()) {
                sheets.insert(e.sprite.clone(), images.add(rgba_image(s.width, s.height, s.rgba, false, TextureFormat::Rgba8UnormSrgb)));
            }
        }
        Icons { table, sheets, pictures }
    }
}

impl Icons {
    /// The named icon (`d_ak47`, `cross`...) `height` tall at 1080p, tinted `color`.
    /// An empty node when the game files don't have it.
    pub fn node(&self, name: &str, height: f32, color: Color) -> impl Bundle {
        let (image, rect, aspect) = self.pick(name);
        (ImageNode { image, rect, color, ..default() }, Node { height: vh(height), aspect_ratio: Some(aspect), ..default() })
    }

    /// Like `node`, but no wider than `max_aspect` times `height`: long icons shrink to fit.
    pub fn node_in(&self, name: &str, height: f32, max_aspect: f32, color: Color) -> impl Bundle {
        let aspect = self.pick(name).2;
        self.node(name, height * (max_aspect / aspect).min(1.0), color)
    }

    /// The picture, the part of it to show (sprite sheets) and its aspect ratio.
    pub(super) fn pick(&self, name: &str) -> (Handle<Image>, Option<Rect>, f32) {
        if let Some((image, aspect)) = self.pictures.get(name) {
            return (image.clone(), None, *aspect);
        }
        match self.table.get(name).and_then(|e| Some((e, self.sheets.get(&e.sprite)?))) {
            Some((e, img)) => (img.clone(), Some(Rect::new(e.x as f32, e.y as f32, (e.x + e.w) as f32, (e.y + e.h) as f32)), e.w as f32 / e.h as f32),
            None => (Handle::default(), None, 1.0),
        }
    }
}

/// A PNG from the game files (add-on first).
fn read_png(rel: &str) -> Option<Image> {
    use bevy::asset::RenderAssetUsages;
    use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
    let bytes = crate::gamefiles::paths(rel).iter().find_map(|p| std::fs::read(p).ok())?;
    Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::linear(), RenderAssetUsages::default()).ok()
}

/// HUD icons redrawn as white PNGs (the game tints them); the sprite of the same name is the fallback.
const HUD_ICONS: [&str; 28] = [
    "cross", "suit_full", "suithelmet_full", "defuser", "c4", "buyzone", "stopwatch", "d_headshot", "d_skull", "d_knife", "d_glock18", "d_usp", "d_p228", "d_deagle", "d_mac10", "d_tmp", "d_mp5navy", "d_ump45", "d_p90", "d_galil", "d_famas", "d_ak47", "d_m4a1", "d_scout", "d_awp", "d_flashbang", "d_grenade", "d_smoke",
];

/// Our own icons that exist (the rest use the sprite sheets).
fn load_pictures(world: &mut World) -> HashMap<String, (Handle<Image>, f32)> {
    let mut images = world.resource_mut::<Assets<Image>>();
    let mut out = HashMap::new();
    // The HUD's own icons (`gfx/hud/<name>.png`), under the sprite's name.
    for name in HUD_ICONS {
        let Some(img) = read_png(&format!("gfx/hud/{name}.png")) else { continue };
        let aspect = img.width() as f32 / img.height() as f32;
        out.insert(name.to_string(), (images.add(img), aspect));
    }
    out
}

/// The kill-feed icon for a weapon.
pub fn weapon_icon(id: Id) -> &'static str {
    match id {
        Id::Knife => "d_knife",
        Id::Glock => "d_glock18",
        Id::Usp => "d_usp",
        Id::P228 => "d_p228",
        Id::Deagle => "d_deagle",
        Id::Mac10 => "d_mac10",
        Id::Tmp => "d_tmp",
        Id::Mp5 => "d_mp5navy",
        Id::Ump45 => "d_ump45",
        Id::P90 => "d_p90",
        Id::Galil => "d_galil",
        Id::Famas => "d_famas",
        Id::Ak47 => "d_ak47",
        Id::M4a1 => "d_m4a1",
        Id::Scout => "d_scout",
        Id::Awp => "d_awp",
        Id::HeGrenade => "d_grenade",
        Id::Flashbang => "d_flashbang",
        Id::Smoke => "d_smoke",
        Id::C4 => "c4",
    }
}


