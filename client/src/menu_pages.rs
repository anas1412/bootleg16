//! Drawing the main menu and its pages (Interference, design/mockups/ui): the numbered list on
//! the left, the wordmark bottom left, you top right, the open page on the right.
//! The frame stays while pages swap inside it: on a page change the old page tunes out and the
//! new one tunes in, its rows staggered. Any other change rebuilds in place, where selections
//! slide and values roll (`UiMemory`). Play is a full screen of its own.
use crate::assistant::{ASSISTANTS, OFF};
use crate::menu::{AppState, MenuDirty, MenuRoot, MenuState, Page, Settings};
use crate::ui::{chips, kit_text, nav, page_title, primary_button, setting_row, stepper, vh, Anim, Choice, Kit, NavItem, UiAction, UiMemory, AMBER, AMBER_INK, BAD, T_OUT, TX1, TX2, TX3};
use bevy::prelude::*;
use bevy::text::LineHeight;

/// The open page (or the whole Play screen), swapped with a transition.
#[derive(Component)]
pub(crate) struct MenuPage;

#[allow(clippy::too_many_arguments)]
pub fn rebuild_menu(
    mut commands: Commands,
    mut st: ResMut<MenuState>,
    settings: Res<Settings>,
    voice: Res<crate::voice::VoiceSettings>,
    kit: Res<Kit>,
    lobby: Res<crate::play::Lobby>,
    steam: Option<Res<crate::steam::Steam>>,
    mut dirty: ResMut<MenuDirty>,
    mut memory: ResMut<UiMemory>,
    faces: Res<crate::assistant::view::Faces>,
    frames: Query<Entity, With<MenuRoot>>,
    pages: Query<Entity, With<MenuPage>>,
) {
    if !st.rebuild && !dirty.0 {
        return;
    }
    st.rebuild = false;
    dirty.0 = false;
    let st = &*st;
    let first = frames.is_empty() && pages.is_empty();
    let play = st.page == Page::Join;
    let page_changed = memory.changed("menu.page", &format!("{:?}", st.page));
    let kind_changed = memory.changed("menu.kind", if play { "play" } else { "menu" });
    // Leaving parts tune out (and are despawned when done); the rest is rebuilt in place.
    for e in &frames {
        if kind_changed {
            commands.entity(e).remove::<MenuRoot>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    for e in &pages {
        if page_changed {
            commands.entity(e).remove::<MenuPage>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    let tune_frame = first || kind_changed;
    let tune_page = first || page_changed;
    let delay = if page_changed { T_OUT } else { 0.0 };
    let tune = |i: usize| Anim::tune_in(delay + i as f32 * crate::ui::STAGGER);
    let abs = |node: Node| Node { position_type: PositionType::Absolute, ..node };
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    let steam = steam.as_deref();

    if play {
        let mut page = commands.spawn((MenuPage, DespawnOnExit(AppState::Menu), full));
        if tune_page {
            page.insert(tune(0));
        }
        page.with_children(|root| {
            crate::play::screen(root, &kit, st, &lobby, &settings, steam);
            crate::found::notes(root, &kit, &lobby, steam, 72.0 + 68.0 + 16.0);
        });
        crate::found::overlay(&mut commands, &kit, &lobby, &mut memory);
        return;
    }

    // The frame.
    commands.spawn((MenuRoot, DespawnOnExit(AppState::Menu), full.clone())).with_children(|root| {
        let mut list = root.spawn(abs(Node { left: vh(80.0), top: vh(200.0), width: vh(560.0), flex_direction: FlexDirection::Column, ..default() }));
        if tune_frame {
            list.insert(Anim::stagger(0));
        }
        list.with_children(|col| {
            let item = |num, label, jp, action, selected| NavItem { num, label, jp, action, selected };
            nav(col, &kit, "menu.nav", vec![
                item("01", "Practice", "練習", UiAction::Page(Page::Practice), st.page == Page::Practice),
                item("02", "Play", "プレイ", UiAction::Page(Page::Join), false),
                item("03", "Settings", "設定", UiAction::Page(Page::Settings), st.page == Page::Settings),
                item("04", "Voice", "音声", UiAction::Page(Page::Voice), st.page == Page::Voice),
                item("05", "Quit", "終了", UiAction::Quit, false),
            ], false);
            if let Some(e) = &st.error {
                col.spawn((kit_text(e, 16.0, BAD, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
            }
        });
        // Bottom left: the wordmark. The amber 16 block spans BOOTLEG's cap height exactly
        // (cap top to baseline), the 16 inside it with equal space on all sides.
        let mut jp = root.spawn((abs(Node { left: vh(92.0), bottom: vh(262.0), ..default() }), kit_text("ブートレグ・シックスティーン", 16.0, TX2, &kit.jp)));
        if tune_frame {
            jp.insert(Anim::stagger(3));
        }
        const CAP: f32 = 123.0;
        const BASE: f32 = 1.5;
        const NUDGE: f32 = 7.5;
        let mut mark = root.spawn(abs(Node { left: vh(88.0), bottom: vh(96.0), align_items: AlignItems::FlexEnd, column_gap: vh(8.0), ..default() }));
        if tune_frame {
            mark.insert(Anim::stagger(4));
        }
        mark.with_children(|w| {
            w.spawn((kit_text("BOOTLEG", 150.0, TX1, &kit.display), LineHeight::RelativeToFont(0.8)));
            w.spawn((Node { height: vh(CAP), margin: UiRect::bottom(vh(BASE)), padding: UiRect::horizontal(vh(12.0)), align_items: AlignItems::Center, ..default() }, BackgroundColor(AMBER))).with_children(|b| {
                b.spawn((kit_text("16", 112.0, AMBER_INK, &kit.display), LineHeight::RelativeToFont(1.0), Node { margin: UiRect::top(vh(NUDGE)), ..default() }));
            });
        });
        // Top right: who you are; under it your running match and invites; over all, a match found.
        crate::play::identity(root, &kit, &settings, steam, 80.0);
        crate::found::notes(root, &kit, &lobby, steam, 80.0 + 68.0 + 16.0);
    });
    crate::found::overlay(&mut commands, &kit, &lobby, &mut memory);

    // The page.
    let mut page = commands.spawn((MenuPage, DespawnOnExit(AppState::Menu), abs(Node { left: vh(720.0), right: vh(96.0), top: vh(200.0), bottom: vh(96.0), flex_direction: FlexDirection::Column, ..default() })));
    if tune_page {
        page.insert(tune(2));
    }
    page.with_children(|page| match st.page {
        Page::Main | Page::Join => {}
        Page::Practice => practice(page, &kit, &mut memory, st, &lobby),
        Page::Settings => settings_page(page, &kit, &faces, &mut memory, &settings, tune_page.then_some(delay)),
        Page::Voice => crate::voice::page(page, &kit, &mut memory, &voice, tune_page.then_some(delay)),
    });
}

fn practice(page: &mut ChildSpawnerCommands, kit: &Kit, memory: &mut UiMemory, st: &MenuState, lobby: &crate::play::Lobby) {
    page_title(page, kit, if st.deathmatch { "Up to 20 players · drop in any time · instant respawn" } else { "Offline · no rounds · infinite ammo · instant respawn" }, "Practice", "練習");
    label(page, kit, "Mode");
    chips(page, kit, "practice.mode", vec![Choice::new("Solo · offline", UiAction::PracticeMode(false), !st.deathmatch), Choice::new("Deathmatch · online", UiAction::PracticeMode(true), st.deathmatch)]);
    if st.deathmatch {
        // an official server: teams are picked for you, buying is free anywhere, no rounds;
        // at 100 kills the leader is announced and everyone starts over
        for (i, line) in ["Teams are picked for you, respawn at a random spawn", "Buy anything, anywhere, any time", "First to 100 kills wins, then scores start over"].into_iter().enumerate() {
            page.spawn((kit_text(line, 18.0, TX2, &kit.text), Node { margin: UiRect::top(vh(if i == 0 { 40.0 } else { 12.0 })), ..default() }));
        }
        page.spawn(Node { margin: UiRect::top(vh(40.0)), ..default() }).with_children(|b| {
            if lobby.shown.is_some() {
                primary_button(b, kit, "Join deathmatch", "Enter", UiAction::Lobby(crate::play::LobbyAction::Deathmatch));
            } else {
                crate::ui::primary_off(b, kit, "Join deathmatch", "The lobby is offline");
            }
        });
        if !lobby.notice.is_empty() {
            page.spawn((kit_text(lobby.notice.clone(), 15.0, AMBER, &kit.text), Node { margin: UiRect::top(vh(16.0)), ..default() }));
        }
        return;
    }
    label(page, kit, "Map");
    chips(page, kit, "practice.map", st.maps.iter().map(|m| Choice::new(m, UiAction::PickMap(m.clone()), *m == st.map)).collect());
    label(page, kit, "Dummies");
    stepper(page, kit, memory, "practice.dummies", &st.dummies.to_string(), UiAction::Dummies(-1), UiAction::Dummies(1));
    page.spawn(Node { margin: UiRect::top(vh(40.0)), ..default() }).with_children(|b| {
        primary_button(b, kit, &format!("Start · {}", st.map), "Enter", UiAction::StartPractice);
    });
}

/// Setting rows, staggered in when the page tunes in (`tune` is the page's delay).
pub(crate) fn rows(page: &mut ChildSpawnerCommands, tune: Option<f32>, build: impl FnOnce(&mut ChildSpawnerCommands) -> Vec<Entity>) {
    page.spawn(Node { flex_direction: FlexDirection::Column, margin: UiRect::top(vh(24.0)), ..default() }).with_children(|rows| {
        let ids = build(rows);
        if let Some(d) = tune {
            for (i, id) in ids.into_iter().enumerate() {
                rows.commands().entity(id).insert(Anim::tune_in(d + (i + 1) as f32 * crate::ui::STAGGER));
            }
        }
    });
}

fn settings_page(page: &mut ChildSpawnerCommands, kit: &Kit, faces: &crate::assistant::view::Faces, memory: &mut UiMemory, s: &Settings, tune: Option<f32>) {
    page_title(page, kit, "Saved as you change them", "Settings", "設定");
    let on_off = |key: &str, on: bool, a: fn(bool) -> UiAction| (key.to_string(), on, a);
    rows(page, tune, |rows| {
        let mut ids = vec![setting_row(rows, kit, "Name", |r| {
            r.spawn(kit_text(format!("{}_", s.name), 20.0, TX1, &kit.num));
            r.spawn(kit_text("TYPE TO CHANGE", 13.0, TX3, &kit.text));
        })];
        ids.push(setting_row(rows, kit, "Sensitivity", |r| stepper(r, kit, memory, "set.sens", &format!("{:.2}", s.sensitivity), UiAction::Sensitivity(-0.1), UiAction::Sensitivity(0.1))));
        ids.push(setting_row(rows, kit, "Volume", |r| stepper(r, kit, memory, "set.vol", &format!("{:.0}%", s.volume * 100.0), UiAction::Volume(-0.05), UiAction::Volume(0.05))));
        let cap = if s.fps_max == 0 { "Off".to_string() } else { s.fps_max.to_string() };
        ids.push(setting_row(rows, kit, "Frame rate cap", |r| stepper(r, kit, memory, "set.fps", &cap, UiAction::FpsMax(-1), UiAction::FpsMax(1))));
        for (name, (key, on, a)) in [("VSync", on_off("set.vsync", s.vsync, UiAction::SetVsync))] {
            ids.push(setting_row(rows, kit, name, |r| chips(r, kit, &key, vec![Choice::new("On", a(true), on), Choice::new("Off", a(false), !on)])));
        }
        ids.push(setting_row(rows, kit, "Team IDs through walls", |r| {
            chips(r, kit, "set.ids", ["Off", "Pips", "Pips + names"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::SetTeamIds(i as u8), s.team_ids == i as u8)).collect());
        }));
        ids.push(setting_row(rows, kit, "AI assistant", |r| {
            let mut options: Vec<Choice> = ASSISTANTS
                .iter()
                .enumerate()
                .map(|(i, a)| {
                    let color = a.color;
                    let face = i;
                    Choice { label: a.name, action: UiAction::SetAssistant(i as u8), selected: s.assistant == i as u8, lead: Some(Box::new(move |c: &mut ChildSpawnerCommands| crate::assistant::view::chip_portrait(c, faces, face, color))) }
                })
                .collect();
            options.push(Choice::new("Off", UiAction::SetAssistant(OFF), s.assistant == OFF));
            chips(r, kit, "set.ai", options);
        }));
        ids.push(setting_row(rows, kit, "AI assistant messages", |r| {
            chips(r, kit, "set.chat", ["Quiet", "Normal", "Chatty"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::Chattiness(i as u8), s.chattiness == i as u8)).collect());
        }));
        for (name, (key, on, a)) in [("Relay team info", on_off("set.relay", s.relay, UiAction::SetRelay)), ("During rounds", on_off("set.inround", s.assistant_in_round, UiAction::SetAssistantInRound))] {
            ids.push(setting_row(rows, kit, name, |r| chips(r, kit, &key, vec![Choice::new("On", a(true), on), Choice::new("Off", a(false), !on)])));
        }
        ids
    });
}

/// A small caps label above a control.
fn label(p: &mut ChildSpawnerCommands, kit: &Kit, s: &str) {
    p.spawn((kit_text(s.to_uppercase(), 13.0, TX2, &kit.text), Node { margin: UiRect { top: vh(32.0), bottom: vh(12.0), ..default() }, ..default() }));
}
