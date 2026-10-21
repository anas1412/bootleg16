//! Characters: every folder in `gamedata/characters/` is one, so adding a character is
//! adding a folder (a Source player model on the ValveBiped skeleton, its materials, and a
//! `character.cfg` with its name). You pick one in the menu and wear it on both sides; everyone
//! in the match sees it. All characters play the CS animations (`rig`) and share one hitbox.
//! - `rig`: posing a character from the CS skeleton.
//! - `look`: a character's meshes and materials, ready to draw.
pub mod look;
pub mod rig;

use crate::menu::Settings;
use crate::ui::{card, clickable_keep_border, kit_text, page_title, vh, Kit, UiAction, BG1, TX1, TX2, TX3};
use bevy::prelude::*;
use std::path::PathBuf;

/// One character folder.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The folder's name: what's saved and sent over the network.
    pub id: String,
    /// Shown in the menu (`name = "..."` in `character.cfg`; the id when missing).
    pub name: String,
    pub dir: PathBuf,
}

/// Every character found, sorted by id (the same list on every machine with the same data).
#[derive(Resource, Default)]
pub struct Characters(pub Vec<Entry>);

impl Characters {
    pub fn find(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|e| e.id == id)
    }

    /// The character a player wears: their pick, or (none yet, or not installed here) one
    /// chosen by their player number so a team doesn't look all alike.
    pub fn for_player(&self, pick: &str, player: u8) -> Option<usize> {
        self.find(pick).or_else(|| (!self.0.is_empty()).then(|| player as usize % self.0.len()))
    }
}

/// Reads `name = "..."` from a `character.cfg`.
fn cfg_name(text: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == "name").then(|| v.trim().trim_matches('"').to_string()).filter(|v| !v.is_empty())
    })
}

/// A valid id: lowercase letters, digits, `-` and `_`, at most 32 (it travels over the network).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn scan() -> Characters {
    let mut list: Vec<Entry> = Vec::new();
    for root in crate::gamefiles::paths("characters") {
        let Ok(dirs) = std::fs::read_dir(&root) else { continue };
        for dir in dirs.flatten().map(|d| d.path()).filter(|d| d.join("model.mdl").is_file()) {
            let Some(id) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string).filter(|id| valid_id(id)) else { continue };
            if list.iter().any(|e| e.id == id) {
                continue; // the first data folder wins, like every other file
            }
            let name = std::fs::read_to_string(dir.join("character.cfg")).ok().and_then(|t| cfg_name(&t)).unwrap_or_else(|| id.clone());
            list.push(Entry { id, name, dir });
        }
    }
    list.sort_by(|a, b| a.id.cmp(&b.id));
    info!("characters: {}", list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join(", "));
    Characters(list)
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(scan());
    }
}

/// The page: one card per character; the one you wear is framed.
pub fn page(page: &mut ChildSpawnerCommands, kit: &Kit, settings: &Settings, characters: &Characters) {
    page_title(page, kit, "Who you are · both sides · everyone in your match sees it", "Characters", "キャラ");
    page.spawn(Node { flex_wrap: FlexWrap::Wrap, column_gap: vh(16.0), row_gap: vh(16.0), margin: UiRect::top(vh(32.0)), ..default() }).with_children(|grid| {
        for c in &characters.0 {
            let picked = settings.character.as_deref() == Some(c.id.as_str());
            let mut card_e = grid.spawn(card(Node { width: vh(220.0), height: vh(96.0), flex_direction: FlexDirection::Column, justify_content: JustifyContent::FlexEnd, padding: UiRect::all(vh(16.0)), ..default() }, picked));
            clickable_keep_border(&mut card_e, UiAction::SetCharacter(c.id.clone()), BG1);
            card_e.with_children(|n| {
                n.spawn((kit_text(c.name.to_uppercase(), 22.0, if picked { TX1 } else { TX2 }, &kit.text), Pickable::IGNORE));
            });
        }
    });
    if characters.0.is_empty() {
        page.spawn((kit_text("NO CHARACTERS FOUND IN THE GAME DATA", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
    } else if settings.character.is_none() {
        page.spawn((kit_text("NOT PICKED YET: YOU GET ONE BY YOUR PLAYER NUMBER", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_names() {
        assert!(valid_id("lisa") && valid_id("t_white-2"));
        assert!(!valid_id("") && !valid_id("Lisa") && !valid_id("../x") && !valid_id(&"a".repeat(33)));
        assert_eq!(cfg_name("name = \"Tammy\"\n"), Some("Tammy".into()));
        assert_eq!(cfg_name("# nothing\n"), None);
    }
}
