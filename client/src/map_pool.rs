//! Maps page (menu): the official map pool, each with its CS 1.6 radar image, the map's own
//! briefing (`maps/<map>.txt`) and whether you have it (matches only use maps everyone has).
//! Stands in until our own maps and art replace them.
use crate::ui::{card, kit_text, page_title, vh, Kit, AMBER, BAD, OK, TX1, TX2, TX3};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

/// One pool map: radar image, briefing title and lines, and whether it's installed.
pub struct PoolMap {
    pub name: &'static str,
    pub radar: Option<Handle<Image>>,
    pub title: String,
    pub brief: Vec<String>,
    pub installed: bool,
}

#[derive(Resource, Default)]
pub struct Pool(pub Vec<PoolMap>);

pub struct MapPoolPlugin;

impl Plugin for MapPoolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pool>().add_systems(Startup, load);
    }
}

fn load(mut pool: ResMut<Pool>, mut images: ResMut<Assets<Image>>) {
    let dir = crate::game_dir();
    pool.0 = net::config::MAP_POOL
        .iter()
        .map(|&name| {
            let radar = std::fs::read(dir.join("overviews").join(format!("{name}.bmp")))
                .ok()
                .and_then(|d| crate::radar::load_bmp(&d))
                .map(|(w, h, rgba)| images.add(crate::level::rgba_image(w, h, rgba, false, TextureFormat::Rgba8UnormSrgb)));
            // The briefing: a title line, then paragraphs (we keep the side objectives).
            let text = std::fs::read_to_string(dir.join("maps").join(format!("{name}.txt"))).unwrap_or_default();
            let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
            let title = lines.next().unwrap_or(name).to_string();
            let brief = text.split("\n\n").skip(1).map(|p| p.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|p| p.starts_with("Counter-Terrorists") || p.starts_with("Terrorists")).collect();
            PoolMap { name, radar, title, brief, installed: dir.join("maps").join(format!("{name}.bsp")).exists() }
        })
        .collect();
}

/// The page: a card per map, radar image on the left, briefing on the right.
pub fn page(page: &mut ChildSpawnerCommands, kit: &Kit, pool: &Pool) {
    page_title(page, kit, "The official pool · casual matches and custom games play these", "Maps", "マップ");
    page.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vh(12.0), margin: UiRect::top(vh(24.0)), ..default() }).with_children(|list| {
        for m in &pool.0 {
            list.spawn(card(Node { column_gap: vh(20.0), padding: UiRect::all(vh(12.0)), ..default() }, false)).with_children(|c| {
                // CS radar images are 4:3.
                let (w, h) = (152.0, 114.0);
                match &m.radar {
                    Some(img) => _ = c.spawn((ImageNode::new(img.clone()), Node { width: vh(w), height: vh(h), flex_shrink: 0.0, ..default() })),
                    None => _ = c.spawn((Node { width: vh(w), height: vh(h), flex_shrink: 0.0, ..default() }, BackgroundColor(Color::BLACK))),
                }
                c.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vh(6.0), flex_grow: 1.0, ..default() }).with_children(|t| {
                    t.spawn(Node { column_gap: vh(12.0), align_items: AlignItems::Baseline, ..default() }).with_children(|r| {
                        r.spawn(kit_text(m.name.to_uppercase(), 22.0, TX1, &kit.text));
                        r.spawn(kit_text(m.title.to_uppercase(), 13.0, AMBER, &kit.text));
                    });
                    for p in &m.brief {
                        t.spawn(kit_text(p.clone(), 14.0, TX2, &kit.text));
                    }
                    let (status, color) = if m.installed { ("INSTALLED", OK) } else { ("MISSING: YOU CAN'T BE MATCHED ON IT", BAD) };
                    t.spawn((kit_text(status, 12.0, color, &kit.text), Node { margin: UiRect::top(vh(4.0)), ..default() }));
                });
            });
        }
        list.spawn(kit_text("MATCHES ONLY USE MAPS EVERY PLAYER HAS", 12.0, TX3, &kit.text));
    });
}
