//! HUD icons cut from the install's own `sprites/hud.txt` sprites (CS 1.6 assets for now),
//! the buy menu's weapon pictures, and which icon each weapon uses.
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
}

impl FromWorld for Icons {
    fn from_world(world: &mut World) -> Self {
        // Add-on first, then cstrike, then valve (per file).
        let dirs = crate::gamefiles::paths("sprites");
        let read = |file: &str| dirs.iter().find_map(|d| std::fs::read_to_string(d.join(file)).ok());
        let Some(text) = read("hud.txt") else { return Icons { table: HashMap::new(), sheets: HashMap::new() } };
        let mut table = hud_table(&text);
        // Buy menu art: each `weapon_<name>.txt`'s big "weapon" picture, as `gun_<name>`.
        let mut guns = Vec::new();
        for name in GUN_ART {
            if let Some(e) = read(&format!("weapon_{name}.txt")).and_then(|t| hud_table(&t).remove("weapon")) {
                guns.push(e.clone());
                table.insert(format!("gun_{name}"), e);
            }
        }
        let mut images = world.resource_mut::<Assets<Image>>();
        let mut sheets = HashMap::new();
        for e in table.values() {
            if sheets.contains_key(&e.sprite) {
                continue;
            }
            if let Ok(mut s) = dirs.iter().find_map(|d| Sprite::load(d.join(format!("{}.spr", e.sprite))).ok()).ok_or(()) {
                if guns.iter().any(|g| g.sprite == e.sprite) {
                    clean_gun_sheet(&mut s, &guns);
                }
                sheets.insert(e.sprite.clone(), images.add(rgba_image(s.width, s.height, s.rgba, false, TextureFormat::Rgba8UnormSrgb)));
            }
        }
        Icons { table, sheets }
    }
}

impl Icons {
    /// The named icon (`d_ak47`, `cross`...) `height` tall at 1080p, tinted `color`.
    /// An empty node when the game files don't have it.
    pub fn node(&self, name: &str, height: f32, color: Color) -> impl Bundle {
        let found = self.table.get(name).and_then(|e| Some((e, self.sheets.get(&e.sprite)?)));
        let (image, rect, aspect) = match found {
            Some((e, img)) => (img.clone(), Some(Rect::new(e.x as f32, e.y as f32, (e.x + e.w) as f32, (e.y + e.h) as f32)), e.w as f32 / e.h as f32),
            None => (Handle::default(), None, 1.0),
        };
        (ImageNode { image, rect, color, ..default() }, Node { height: vh(height), aspect_ratio: Some(aspect), ..default() })
    }
}

/// Weapons with buy menu art (`sprites/weapon_<name>.txt`).
const GUN_ART: [&str; 17] = ["glock18", "usp", "deagle", "mac10", "tmp", "mp5navy", "ump45", "p90", "galil", "famas", "ak47", "m4a1", "scout", "awp", "flashbang", "hegrenade", "smokegrenade"];

/// The big weapon pictures sit on a dim box with the slot number printed bottom left:
/// drop the box (dark pixels) and the label so only the gun is left.
fn clean_gun_sheet(s: &mut Sprite, guns: &[HudEntry]) {
    for px in s.rgba.chunks_mut(4) {
        px[3] = ((px[3] as i32 - 195).clamp(0, 60) * 255 / 60) as u8;
    }
    for g in guns {
        for y in g.y + 29..(g.y + g.h).min(s.height) {
            for x in g.x..(g.x + 110).min(s.width) {
                s.rgba[((y * s.width + x) * 4 + 3) as usize] = 0;
            }
        }
    }
}

/// The buy menu picture for a weapon, if it has one.
pub fn gun_art(id: Id) -> Option<&'static str> {
    Some(match id {
        Id::Glock => "gun_glock18",
        Id::Usp => "gun_usp",
        Id::Deagle => "gun_deagle",
        Id::Mac10 => "gun_mac10",
        Id::Tmp => "gun_tmp",
        Id::Mp5 => "gun_mp5navy",
        Id::Ump45 => "gun_ump45",
        Id::P90 => "gun_p90",
        Id::Galil => "gun_galil",
        Id::Famas => "gun_famas",
        Id::Ak47 => "gun_ak47",
        Id::M4a1 => "gun_m4a1",
        Id::Scout => "gun_scout",
        Id::Awp => "gun_awp",
        Id::Flashbang => "gun_flashbang",
        Id::HeGrenade => "gun_hegrenade",
        Id::Smoke => "gun_smokegrenade",
        _ => return None,
    })
}

/// The kill-feed icon for a weapon.
pub fn weapon_icon(id: Id) -> &'static str {
    match id {
        Id::Knife => "d_knife",
        Id::Glock => "d_glock18",
        Id::Usp => "d_usp",
        Id::P228 => "d_p228",
        Id::Deagle => "d_deagle",
        Id::FiveSeven => "d_fiveseven",
        Id::Elite => "d_elite",
        Id::M3 => "d_m3",
        Id::Xm1014 => "d_xm1014",
        Id::Mac10 => "d_mac10",
        Id::Tmp => "d_tmp",
        Id::Mp5 => "d_mp5navy",
        Id::Ump45 => "d_ump45",
        Id::P90 => "d_p90",
        Id::Galil => "d_galil",
        Id::Famas => "d_famas",
        Id::Ak47 => "d_ak47",
        Id::M4a1 => "d_m4a1",
        Id::Sg552 => "d_sg552",
        Id::Aug => "d_aug",
        Id::Scout => "d_scout",
        Id::Awp => "d_awp",
        Id::G3sg1 => "d_g3sg1",
        Id::Sg550 => "d_sg550",
        Id::M249 => "d_m249",
        Id::HeGrenade => "d_grenade",
        Id::Flashbang => "d_flashbang",
        Id::Smoke => "d_skull",
        Id::C4 => "c4",
    }
}

/// The sheet rectangle of a named icon.
pub(super) fn icons_rect(icons: &Icons, name: &str) -> (Option<Rect>, f32) {
    icons.table.get(name).map(|e| (Some(Rect::new(e.x as f32, e.y as f32, (e.x + e.w) as f32, (e.y + e.h) as f32)), e.w as f32 / e.h as f32)).unwrap_or((None, 1.0))
}

