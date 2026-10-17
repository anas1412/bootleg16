//! Characters page (menu): the character you wear on each side, picked from CS 1.6's classes
//! with their own portraits (`gfx/vgui`). Everyone sees your pick (`Score::looks`). Stands in
//! until our own characters replace them.
use crate::menu::Settings;
use crate::ui::{card, clickable_keep_border, kit_text, page_title, vh, Kit, UiAction, BG1, CT_COLOR, T_COLOR, TX1, TX2, TX3};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// The classes per side, in `Score::looks` order (players.rs loads the same models).
pub const T_CLASSES: [(&str, &str); 4] = [("terror", "Phoenix Connexion"), ("leet", "Elite Crew"), ("arctic", "Arctic Avengers"), ("guerilla", "Guerilla Warfare")];
pub const CT_CLASSES: [(&str, &str); 4] = [("urban", "Seal Team 6"), ("gsg9", "GSG-9"), ("sas", "SAS"), ("gign", "GIGN")];

/// The portraits, loaded once.
#[derive(Resource, Default)]
pub struct Portraits(pub [[Option<Handle<Image>>; 4]; 2]);

pub struct CharactersPlugin;

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Portraits>().add_systems(Startup, load);
    }
}

fn load(mut portraits: ResMut<Portraits>, mut images: ResMut<Assets<Image>>) {
    for (side, list) in [T_CLASSES, CT_CLASSES].iter().enumerate() {
        for (i, (file, _)) in list.iter().enumerate() {
            let path = crate::game_dir().join("cstrike/gfx/vgui").join(format!("{file}.tga"));
            portraits.0[side][i] = std::fs::read(path).ok().and_then(|d| goldsrc::tga::decode(&d)).map(|t| images.add(crate::level::rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb)));
        }
    }
}

/// The page: a row of four portraits per side; the one you wear is framed.
pub fn page(page: &mut ChildSpawnerCommands, kit: &Kit, settings: &Settings, portraits: &Portraits) {
    page_title(page, kit, "Who you are on each side · everyone in your match sees it", "Characters", "キャラ");
    for (side, (name, color, list)) in [("Terrorists", T_COLOR, T_CLASSES), ("Counter-Terrorists", CT_COLOR, CT_CLASSES)].into_iter().enumerate() {
        page.spawn((kit_text(name.to_uppercase(), 13.0, color, &kit.text), Node { margin: UiRect { top: vh(32.0), bottom: vh(12.0), ..default() }, ..default() }));
        page.spawn(Node { column_gap: vh(16.0), ..default() }).with_children(|row| {
            for (i, (_, label)) in list.iter().enumerate() {
                let picked = settings.looks.is_some_and(|l| l[side] == i as u8);
                let mut c = row.spawn(card(Node { width: vh(220.0), flex_direction: FlexDirection::Column, padding: UiRect::all(vh(12.0)), row_gap: vh(10.0), ..default() }, picked));
                clickable_keep_border(&mut c, UiAction::SetLook(side as u8, i as u8), BG1);
                c.with_children(|c| {
                    // CS's class art is 256x196.
                    let (w, h) = (196.0, 196.0 * 196.0 / 256.0);
                    match &portraits.0[side][i] {
                        Some(img) => _ = c.spawn((ImageNode::new(img.clone()), Node { width: vh(w), height: vh(h), ..default() }, Pickable::IGNORE)),
                        None => _ = c.spawn((Node { width: vh(w), height: vh(h), ..default() }, BackgroundColor(Color::BLACK), Pickable::IGNORE)),
                    }
                    c.spawn((kit_text(label.to_uppercase(), 15.0, if picked { TX1 } else { TX2 }, &kit.text), Pickable::IGNORE));
                });
            }
        });
    }
    if settings.looks.is_none() {
        page.spawn((kit_text("NOT PICKED YET: YOU GET ONE AT RANDOM EACH MATCH", 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
    }
}
