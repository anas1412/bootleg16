//! Characters: every folder in `gamedata/characters/` is one, so adding a character is
//! adding a folder (a Source player model on the ValveBiped skeleton, its materials, a
//! `character.cfg` with its name and side, and optionally a `portrait.png` for the menu).
//! Each side (Bureau, Strays) has its own characters and you pick one per side; you wear the
//! one for the side you're on and everyone in the match sees it. All characters play the CS
//! animations (`rig`) and share one hitbox.
//! - `rig`: posing a character from the CS skeleton.
//! - `look`: a character's meshes and materials, ready to draw.
pub mod look;
pub mod rig;

use crate::menu::Settings;
use crate::ui::{chip, chips, kit_text, page_title, vh, Choice, Kit, UiAction, TX1, TX2, TX3};
use bevy::prelude::*;
use net::protocol::Team;
use std::path::PathBuf;

/// One character folder.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The folder's name: what's saved and sent over the network.
    pub id: String,
    /// Shown in the menu (`name = "..."` in `character.cfg`; the id when missing).
    pub name: String,
    pub dir: PathBuf,
    /// `portrait.png` in the folder: the character's picture in the menu (portrait shaped, about 3:4).
    pub portrait: Option<Handle<Image>>,
    /// `side = "bureau"` (counter-terrorists) or `"strays"` (terrorists); none: either side.
    pub side: Option<Team>,
    /// `voice = "female"`: their hurt and death sounds come from `sound/player/<voice>/`
    /// where it has them (none: CS's own).
    pub voice: Option<String>,
}

/// Every character found, sorted by id (the same list on every machine with the same data).
#[derive(Resource, Default)]
pub struct Characters(pub Vec<Entry>);

impl Characters {
    pub fn find(&self, id: &str) -> Option<usize> {
        self.0.iter().position(|e| e.id == id)
    }

    /// The voice of the character player `id` wears (from the scoreboard).
    pub fn voice_of(&self, client: &net::Client, id: u8) -> Option<String> {
        let s = client.scores.iter().find(|s| s.id == id)?;
        self.0.get(self.for_player(&s.character, id, s.team)?)?.voice.clone()
    }

    /// The characters a side may wear, in order.
    pub fn of_side(&self, team: Team) -> Vec<usize> {
        let mine: Vec<usize> = (0..self.0.len()).filter(|&i| self.0[i].side.is_none_or(|s| s == team)).collect();
        // a side nobody made characters for wears anyone
        if mine.is_empty() { (0..self.0.len()).collect() } else { mine }
    }

    /// The character a player wears: their pick, or (none yet, or not installed here) one of
    /// their side's, chosen by their player number so a team doesn't look all alike.
    pub fn for_player(&self, pick: &str, player: u8, team: Team) -> Option<usize> {
        self.find(pick).or_else(|| {
            let side = self.of_side(team);
            (!side.is_empty()).then(|| side[player as usize % side.len()])
        })
    }
}

/// Reads `key = "..."` from a `character.cfg`.
fn cfg(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        (k.trim() == key).then(|| v.trim().trim_matches('"').to_string()).filter(|v| !v.is_empty())
    })
}

fn cfg_side(text: &str) -> Option<Team> {
    match cfg(text, "side")?.to_ascii_lowercase().as_str() {
        "bureau" | "ct" => Some(Team::Ct),
        "strays" | "t" => Some(Team::T),
        _ => None,
    }
}

/// A CS 1.6 class this character is (`cs = "urban"` in its character.cfg), if it is one.
pub fn cs_class(dir: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("character.cfg")).ok()?;
    cfg(&text, "cs").filter(|c| valid_id(c))
}

/// A valid id: lowercase letters, digits, `-` and `_`, at most 32 (it travels over the network).
pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

fn scan(images: &mut Assets<Image>) -> Characters {
    let mut list: Vec<Entry> = Vec::new();
    for root in crate::gamefiles::paths("characters") {
        let Ok(dirs) = std::fs::read_dir(&root) else { continue };
        for dir in dirs.flatten().map(|d| d.path()).filter(|d| d.join("model.mdl").is_file() || cs_class(d).is_some()) {
            let Some(id) = dir.file_name().and_then(|n| n.to_str()).map(str::to_string).filter(|id| valid_id(id)) else { continue };
            if list.iter().any(|e| e.id == id) {
                continue; // the first data folder wins, like every other file
            }
            let text = std::fs::read_to_string(dir.join("character.cfg")).unwrap_or_default();
            // `hidden = "yes"`: kept in the files, not offered or worn (the CS classes, for now)
            if cfg(&text, "hidden").is_some_and(|v| v == "yes") {
                continue;
            }
            let name = cfg(&text, "name").unwrap_or_else(|| id.clone());
            let side = cfg_side(&text);
            let voice = cfg(&text, "voice").filter(|v| valid_id(v));
            let portrait = std::fs::read(dir.join("portrait.png")).ok().and_then(|bytes| {
                use bevy::image::{CompressedImageFormats, ImageSampler, ImageType};
                Image::from_buffer(&bytes, ImageType::Extension("png"), CompressedImageFormats::NONE, true, ImageSampler::linear(), bevy::asset::RenderAssetUsages::default()).ok()
            }).map(|img| images.add(img));
            list.push(Entry { id, name, dir, portrait, side, voice });
        }
    }
    // our own characters first, then CS 1.6's classes, each by id
    list.sort_by_key(|e| (cs_class(&e.dir).is_some(), e.id.clone()));
    info!("characters: {}", list.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join(", "));
    Characters(list)
}

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Characters>()
            .add_systems(Startup, |mut commands: Commands, mut images: ResMut<Assets<Image>>| commands.insert_resource(scan(&mut images)))
            .add_systems(Update, wear.run_if(resource_exists::<crate::game::Walker>));
    }
}

/// Which character a side wears now: your pick if it's one of the side's, else its first.
pub fn picked(characters: &Characters, settings: &Settings, team: Team) -> Option<usize> {
    let side = characters.of_side(team);
    settings.character_for(team).and_then(|id| characters.find(id)).filter(|i| side.contains(i)).or(side.first().copied())
}

/// The next (or previous) character for a side, as an id to save.
pub fn cycle(characters: &Characters, settings: &Settings, team: Team, step: i32) -> Option<String> {
    let side = characters.of_side(team);
    let now = picked(characters, settings, team).and_then(|i| side.iter().position(|&x| x == i)).unwrap_or(0) as i32;
    let n = side.len() as i32;
    (n > 0).then(|| characters.0[side[(now + step).rem_euclid(n) as usize]].id.clone())
}

/// In a match: tell the server the character for the side you're on (again when you switch).
fn wear(mut w: ResMut<crate::game::Walker>, settings: Res<Settings>, characters: Res<Characters>, mut sent: Local<Option<(Team, String)>>) {
    let team = w.client.team();
    if team == Team::Spectator || w.client.id().is_none() {
        return;
    }
    let id = picked(&characters, &settings, team).map(|i| characters.0[i].id.clone()).unwrap_or_default();
    if sent.as_ref() != Some(&(team, id.clone())) {
        w.client.set_character(&id);
        *sent = Some((team, id));
    }
}

/// The page: a tab per side and that side's character, big, with arrows (or the arrow keys)
/// to go through them. What's shown is what you wear on that side.
pub fn page(page: &mut ChildSpawnerCommands, kit: &Kit, settings: &Settings, characters: &Characters, side: Team) {
    page_title(page, kit, "Who you are on each side · everyone in your match sees it", "Characters", "キャラ");
    page.spawn(Node { margin: UiRect::top(vh(24.0)), ..default() }).with_children(|r| {
        chips(r, kit, "chr.side", vec![Choice::new("Bureau", UiAction::CharacterSide(Team::Ct), side == Team::Ct), Choice::new("Strays", UiAction::CharacterSide(Team::T), side == Team::T)]);
    });
    let list = characters.of_side(side);
    let Some(now) = picked(characters, settings, side) else {
        page.spawn((kit_text("NO CHARACTERS FOUND IN THE GAME DATA", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
        return;
    };
    let c = &characters.0[now];
    let at = list.iter().position(|&x| x == now).unwrap_or(0);
    page.spawn(Node { align_items: AlignItems::Center, column_gap: vh(24.0), margin: UiRect::top(vh(24.0)), ..default() }).with_children(|row| {
        chip(row, kit, "‹", UiAction::CycleCharacter(-1), false);
        row.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(12.0), ..default() }).with_children(|col| {
            // the character's picture, or a quiet frame of the same size until it has one
            let frame = Node { width: vh(330.0), height: vh(440.0), ..default() };
            match &c.portrait {
                Some(img) => _ = col.spawn((ImageNode::new(img.clone()), frame, Pickable::IGNORE)),
                None => _ = col.spawn((frame, BackgroundColor(Color::BLACK.with_alpha(0.35)), Pickable::IGNORE)),
            }
            col.spawn(kit_text(c.name.to_uppercase(), 40.0, TX1, &kit.display));
            col.spawn(kit_text(format!("{} / {}", at + 1, list.len()), 13.0, TX2, &kit.num));
        });
        chip(row, kit, "›", UiAction::CycleCharacter(1), false);
    });
    page.spawn((kit_text("← → CHANGE · WORN WHEN YOU PLAY ON THIS SIDE", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(16.0)), ..default() }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_names() {
        assert!(valid_id("lisa") && valid_id("t_white-2"));
        assert!(!valid_id("") && !valid_id("Lisa") && !valid_id("../x") && !valid_id(&"a".repeat(33)));
        assert_eq!(cfg("name = \"Tammy\"\n", "name"), Some("Tammy".into()));
        assert_eq!(cfg("# nothing\n", "name"), None);
        assert_eq!(cfg_side("side = \"strays\"\n"), Some(Team::T));
        assert_eq!(cfg_side("side = \"bureau\"\n"), Some(Team::Ct));
    }
}
