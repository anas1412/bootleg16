//! Drawing the main menu and its pages (Interference, design/mockups/ui): the numbered list on
//! the left, the wordmark bottom left, you top right, the open page on the right.
//! The frame stays while pages swap inside it: on a page change the old page tunes out and the
//! new one tunes in, its rows staggered. Any other change rebuilds in place, where selections
//! slide and values roll (`UiMemory`). Play is a full screen of its own.
use crate::assistant::{ASSISTANTS, OFF};
use crate::menu::{AppState, MenuDirty, MenuRoot, MenuState, Page, Settings};
use crate::ui::{chips, kit_text, nav, page_title, primary_button, setting_row, stepper, vh, Anim, Choice, Kit, NavItem, UiAction, UiMemory, AMBER, AMBER_INK, T_OUT, TX1, TX2, TX3};
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
    binds: Res<crate::binds::Binds>,
    characters: Res<crate::characters::Characters>,
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

    // The frame: the right side (your name, invites, the far 16) and the left side (list,
    // wordmark).
    commands.spawn((MenuRoot, DespawnOnExit(AppState::Menu), full.clone())).with_children(|root| {
        // the far 16: the wordmark's number, huge and faint behind everything on the right
        root.spawn((abs(Node { right: vh(-60.0), top: vh(-140.0), ..default() }), kit_text("16", 1150.0, TX1.with_alpha(0.007), &kit.display), LineHeight::RelativeToFont(0.8), Pickable::IGNORE));
        // Top right: who you are; under it your running match and invites.
        crate::play::identity(root, &kit, &settings, steam, 80.0);
        crate::found::notes(root, &kit, &lobby, steam, 80.0 + 68.0 + 16.0);
    });
    commands.spawn((MenuRoot, DespawnOnExit(AppState::Menu), full.clone())).with_children(|root| {
        // 160 from the top: seven items and an error line still clear the wordmark
        let mut list = root.spawn(abs(Node { left: vh(80.0), top: vh(160.0), width: vh(560.0), flex_direction: FlexDirection::Column, ..default() }));
        if tune_frame {
            list.insert(Anim::stagger(0));
        }
        list.with_children(|col| {
            let item = |num, label, jp, action, selected| NavItem { num, label, jp, action, selected };
            nav(col, &kit, "menu.nav", vec![
                item("01", "Play", "プレイ", UiAction::Page(Page::Join), false),
                item("02", "Practice", "練習", UiAction::Page(Page::Practice), st.page == Page::Practice),
                item("03", "Host", "ホスト", UiAction::Page(Page::Host), st.page == Page::Host),
                item("04", "Characters", "キャラ", UiAction::Page(Page::Characters), st.page == Page::Characters),
                item("05", "Settings", "設定", UiAction::Page(Page::Settings), st.page == Page::Settings),
                item("06", "Voice", "音声", UiAction::Page(Page::Voice), st.page == Page::Voice),
                item("07", "Quit", "終了", UiAction::Quit, false),
            ], false);
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
    });
    // over all, flat: a match found
    crate::found::overlay(&mut commands, &kit, &lobby, &mut memory);

    // The page.
    let mut page = commands.spawn((MenuPage, DespawnOnExit(AppState::Menu), abs(Node { left: vh(720.0), right: vh(96.0), top: vh(200.0), bottom: vh(96.0), flex_direction: FlexDirection::Column, ..default() })));
    if tune_page {
        page.insert(tune(2));
    }
    page.with_children(|page| match st.page {
        Page::Main | Page::Join => {}
        Page::Practice => practice(page, &kit, &mut memory, st, &lobby),
        Page::Host => host(page, &kit, st),
        Page::Settings => settings_page(page, &kit, &faces, &mut memory, &settings, steam, &binds, st.settings_tab, tune_page.then_some(delay)),
        Page::Voice => crate::voice::page(page, &kit, &mut memory, &voice, tune_page.then_some(delay)),
        Page::Characters => crate::characters::page(page, &kit, &settings, &characters, st.character_side),
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

/// Host: a match (or deathmatch) on your PC that Steam friends join.
fn host(page: &mut ChildSpawnerCommands, kit: &Kit, st: &MenuState) {
    page_title(page, kit, "On your PC · friends join from Steam", "Host", "ホスト");
    label(page, kit, "Mode");
    chips(page, kit, "host.mode", vec![Choice::new("Match · 5 v 5", UiAction::HostMode(false), !st.host_dm), Choice::new("Deathmatch", UiAction::HostMode(true), st.host_dm)]);
    label(page, kit, "Map");
    chips(page, kit, "host.map", st.maps.iter().map(|m| Choice::new(m, UiAction::PickMap(m.clone()), *m == st.map)).collect());
    let port = crate::host::PORT;
    for (i, line) in [
        "Friends join from your name in Steam: Join game".to_string(),
        "Or they type your address under Play".to_string(),
        format!("Friends outside your home: forward UDP port {port} on your router"),
    ]
    .into_iter()
    .enumerate()
    {
        page.spawn((kit_text(line, 18.0, TX2, &kit.text), Node { margin: UiRect::top(vh(if i == 0 { 40.0 } else { 12.0 })), ..default() }));
    }
    page.spawn(Node { margin: UiRect::top(vh(40.0)), ..default() }).with_children(|b| {
        primary_button(b, kit, &format!("Host · {}", st.map), "Enter", UiAction::StartHost);
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

/// The chip that shows an action's key and rebinds it when clicked (Keys, Voice).
pub(crate) fn key_chip(r: &mut ChildSpawnerCommands, kit: &Kit, binds: &crate::binds::Binds, a: crate::binds::Action) {
    let waiting = binds.capture == Some(a);
    let label = if waiting { "Press a key".to_string() } else { binds.get(a).label(binds.layout) };
    crate::ui::key_chip(r, kit, &label, UiAction::Rebind(a), waiting);
}

/// Every binding in three columns (move, weapons, game and voice): click a key, press the new
/// one (Esc cancels); a key already used swaps over.
fn keys_section(page: &mut ChildSpawnerCommands, kit: &Kit, binds: &crate::binds::Binds, tune: Option<f32>) {
    let mut cols = page.spawn(Node { margin: UiRect::top(vh(24.0)), column_gap: vh(48.0), ..default() });
    if let Some(d) = tune {
        cols.insert(Anim::tune_in(d + crate::ui::STAGGER));
    }
    cols.with_children(|cols| {
        for (title, from, to) in crate::binds::GROUPS {
            cols.spawn(Node { flex_direction: FlexDirection::Column, flex_grow: 1.0, flex_basis: px(0), ..default() }).with_children(|col| {
                col.spawn((kit_text(title.to_uppercase(), 13.0, TX2, &kit.text), Node { margin: UiRect::bottom(vh(8.0)), ..default() }));
                for (a, _, label, _) in &crate::binds::ACTIONS[from..to] {
                    col.spawn((Node { height: vh(52.0), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, border: UiRect::top(px(1)), ..default() }, BorderColor::all(crate::ui::LINE))).with_children(|r| {
                        r.spawn(kit_text(label.to_uppercase(), 16.0, TX1, &kit.text));
                        key_chip(r, kit, binds, *a);
                    });
                }
            });
        }
    });
    page.spawn(Node { margin: UiRect::top(vh(24.0)), column_gap: vh(16.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
        crate::ui::chip(r, kit, "Reset to defaults", UiAction::ResetBinds, false);
        r.spawn(kit_text(if binds.capture.is_some() { "PRESS A KEY OR MOUSE BUTTON · ESC CANCELS" } else { "CLICK A KEY TO CHANGE IT" }, 13.0, TX3, &kit.text));
    });
}

/// Settings, in four sections picked by chips under the title; a new section's rows tune in.
#[allow(clippy::too_many_arguments)]
fn settings_page(page: &mut ChildSpawnerCommands, kit: &Kit, faces: &crate::assistant::view::Faces, memory: &mut UiMemory, s: &Settings, steam: Option<&crate::steam::Steam>, binds: &crate::binds::Binds, tab: u8, tune: Option<f32>) {
    page_title(page, kit, "Saved as you change them", "Settings", "設定");
    // straight on the page (a wrapper would shrink the group and wrap it)
    page.spawn(Node { height: vh(24.0), ..default() });
    chips(page, kit, "set.tab", ["General", "Crosshair", "AI assistant", "Keys"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::SettingsTab(i as u8), tab == i as u8)).collect());
    let tune = if memory.changed("set.tab.shown", &tab.to_string()) { Some(0.0) } else { tune };
    let on_off = |key: &str, on: bool, a: fn(bool) -> UiAction| (key.to_string(), on, a);
    let toggle = |rows: &mut ChildSpawnerCommands, name: &str, (key, on, a): (String, bool, fn(bool) -> UiAction)| setting_row(rows, kit, name, |r| chips(r, kit, &key, vec![Choice::new("On", a(true), on), Choice::new("Off", a(false), !on)]));
    match tab {
        3 => keys_section(page, kit, binds, tune),
        1 => {
            rows(page, tune, |rows| {
                vec![
                    setting_row(rows, kit, "Colour", |r| {
                        chips(r, kit, "set.xcolor", crate::hud::CROSSHAIR_COLORS.iter().enumerate().map(|(i, (l, _))| Choice::new(l, UiAction::CrosshairColor(i as u8), s.crosshair_color == i as u8)).collect());
                    }),
                    setting_row(rows, kit, "Size", |r| {
                        chips(r, kit, "set.xsize", ["Small", "Medium", "Large"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::CrosshairSize(i as u8), s.crosshair_size == i as u8)).collect());
                    }),
                    toggle(rows, "Opens with spread", on_off("set.xdyn", s.crosshair_dynamic, UiAction::CrosshairDynamic)),
                    toggle(rows, "Centre dot", on_off("set.xdot", s.crosshair_dot, UiAction::CrosshairDot)),
                    // the HUD bends like a visor (hud::visor); 0 is flat
                    setting_row(rows, kit, "HUD curve", |r| stepper(r, kit, memory, "set.curve", &if s.hud_curve == 0 { "Flat".to_string() } else { format!("{}%", s.hud_curve) }, UiAction::HudCurve(-1), UiAction::HudCurve(1))),
                ]
            });
            // what it looks like, on a wall
            page.spawn((Node { width: vh(320.0), height: vh(180.0), margin: UiRect::top(vh(32.0)), border_radius: BorderRadius::all(vh(crate::ui::RADIUS)), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(Color::srgb(0.62, 0.50, 0.36)))).with_children(|b| {
                b.spawn(Node { width: px(0), height: px(0), ..default() }).with_children(|c| crate::hud::draw_crosshair(c, s, crate::hud::STATIC_GAP));
            });
        }
        2 => rows(page, tune, |rows| {
            let mut ids = vec![setting_row(rows, kit, "AI assistant", |r| {
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
            })];
            ids.push(setting_row(rows, kit, "Messages", |r| {
                chips(r, kit, "set.chat", ["Quiet", "Normal", "Chatty"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::Chattiness(i as u8), s.chattiness == i as u8)).collect());
            }));
            ids.push(toggle(rows, "Relay team info", on_off("set.relay", s.relay, UiAction::SetRelay)));
            ids.push(toggle(rows, "During rounds", on_off("set.inround", s.assistant_in_round, UiAction::SetAssistantInRound)));
            ids
        }),
        _ => rows(page, tune, |rows| {
            // your Steam name; typed only when playing without Steam
            let mut ids = vec![setting_row(rows, kit, "Name", |r| match steam {
                Some(st) => {
                    r.spawn(kit_text(st.name.clone(), 20.0, TX1, &kit.text));
                    r.spawn(kit_text("FROM STEAM", 13.0, TX3, &kit.text));
                }
                None => {
                    r.spawn(kit_text(format!("{}_", s.name), 20.0, TX1, &kit.num));
                    r.spawn(kit_text("TYPE TO CHANGE", 13.0, TX3, &kit.text));
                }
            })];
            ids.push(setting_row(rows, kit, "Sensitivity", |r| stepper(r, kit, memory, "set.sens", &format!("{:.2}", s.sensitivity), UiAction::Sensitivity(-0.1), UiAction::Sensitivity(0.1))));
            ids.push(setting_row(rows, kit, "Volume", |r| stepper(r, kit, memory, "set.vol", &format!("{:.0}%", s.volume * 100.0), UiAction::Volume(-0.05), UiAction::Volume(0.05))));
            let cap = if s.fps_max == 0 { "Off".to_string() } else { s.fps_max.to_string() };
            ids.push(setting_row(rows, kit, "Frame rate cap", |r| stepper(r, kit, memory, "set.fps", &cap, UiAction::FpsMax(-1), UiAction::FpsMax(1))));
            ids.push(toggle(rows, "VSync", on_off("set.vsync", s.vsync, UiAction::SetVsync)));
            ids.push(setting_row(rows, kit, "Display", |r| {
                chips(r, kit, "set.display", ["Windowed", "Borderless"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::SetBorderless(i == 1), s.borderless == (i == 1))).collect());
            }));
            // (a borderless window is the size of the screen: no size to choose)
            if !s.borderless {
                let labels: Vec<(u8, String)> = crate::window::sizes_for_screen().into_iter().map(|i| (i, format!("{} x {}", crate::window::RESOLUTIONS[i as usize].0, crate::window::RESOLUTIONS[i as usize].1))).collect();
                ids.push(setting_row(rows, kit, "Window size", |r| {
                    chips(r, kit, "set.res", labels.iter().map(|(i, l)| Choice::new(l, UiAction::SetResolution(*i), s.resolution == *i)).collect());
                }));
            }
            ids.push(setting_row(rows, kit, "Team IDs through walls", |r| {
                chips(r, kit, "set.ids", ["Off", "Pips", "Pips + names"].into_iter().enumerate().map(|(i, l)| Choice::new(l, UiAction::SetTeamIds(i as u8), s.team_ids == i as u8)).collect());
            }));
            ids
        }),
    }
}

/// A small caps label above a control.
fn label(p: &mut ChildSpawnerCommands, kit: &Kit, s: &str) {
    p.spawn((kit_text(s.to_uppercase(), 13.0, TX2, &kit.text), Node { margin: UiRect { top: vh(32.0), bottom: vh(12.0), ..default() }, ..default() }));
}
