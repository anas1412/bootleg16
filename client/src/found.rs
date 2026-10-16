//! Matchmaking on screen (design/mockups/ui: Match found, Accepted, Reconnect, Play):
//! - `overlay`: a match found, over any menu page: ten slots filling as players accept (yours
//!   solid amber), the time to accept running down, ACCEPT breathing and Decline; once you've
//!   accepted, how many are still to accept. It tunes in once with a chirp.
//! - `notes`: top right under your name: your running match (Reconnect), then a party invite.
//! - `cooldown`: Find match while you may not queue: why, and for how long.
use crate::play::{Lobby, LobbyAction};
use crate::steam::Steam;
use crate::ui::{chip, kit_text, lift, meter, primary_button, soft, t, vh, Anim, Kit, Type, UiAction, UiMemory, UiSound, AMBER, AMBER_WASH_2, BACKDROP, BAR, BG1, BG2, LINE_2, TX1, TX2, TX3};
use bevy::prelude::*;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, (chirp, breathe));
}

/// Seconds a match waits for everyone (the lobby's ready check).
const READY_SECONDS: f32 = 20.0;

fn mmss(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The accept button: its glow breathes.
#[derive(Component)]
struct Breathe;

fn breathe(time: Res<Time>, mut q: Query<&mut BoxShadow, With<Breathe>>) {
    let s = (time.elapsed_secs() * std::f32::consts::TAU / 1.4).sin() * 0.5 + 0.5;
    for mut b in &mut q {
        b.0 = vec![ShadowStyle { color: AMBER.with_alpha(0.12 + 0.18 * s), x_offset: px(0), y_offset: px(0), spread_radius: px(0), blur_radius: vh(8.0 + 10.0 * s) }];
    }
}

/// A match found: three rising notes, once.
fn chirp(lobby: Res<Lobby>, mut was: Local<bool>, mut sounds: MessageWriter<UiSound>) {
    let ready = lobby.shown.as_ref().is_some_and(|s| s.ready.is_some());
    if ready && !*was {
        sounds.write(UiSound::Found);
    }
    *was = ready;
}

/// The match found screen, over everything on the menu.
/// Its own root over everything (`MenuRoot`: the next rebuild replaces it).
pub fn overlay(commands: &mut Commands, kit: &Kit, lobby: &Lobby, memory: &mut UiMemory) {
    // screenshot runs: BOOTLEG16_FOUND shows a match found (3 of 10 in)
    let fake = std::env::var_os("BOOTLEG16_FOUND").is_some().then_some(net::lobby::ReadyNet { accepted: 3, total: 10, left: 14.0, you: false });
    let ready = lobby.shown.as_ref().and_then(|s| s.ready).or(fake);
    // tune in when it appears; later redraws (the count, the time) stay put
    let appeared = memory.previous("found.open") != Some("1");
    memory.changed("found.open", if ready.is_some() { "1" } else { "0" });
    let Some(r) = ready else { return };
    commands
        .spawn((crate::menu::MenuRoot, DespawnOnExit(crate::menu::AppState::Menu), Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(BACKDROP), GlobalZIndex(30)))
        .with_children(|full| {
            let mut panel = full.spawn((soft(Node { width: vh(640.0), padding: UiRect::axes(vh(48.0), vh(40.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, border: UiRect::all(px(1)), ..default() }), BackgroundColor(BG1.with_alpha(0.96)), BorderColor::all(LINE_2), lift()));
            if appeared {
                panel.insert(Anim::tune_in(0.0));
            }
            panel.with_children(|p| {
                p.spawn(t(kit, Type::Label, "Casual 5v5 · a match is ready", TX2));
                p.spawn((kit_text("MATCH FOUND", 96.0, TX1, &kit.display), Node { margin: UiRect::top(vh(12.0)), ..default() }));
                // one slot per player: yours solid, the others amber as they accept
                p.spawn(Node { column_gap: vh(8.0), margin: UiRect::top(vh(28.0)), ..default() }).with_children(|row| {
                    let others = r.accepted.saturating_sub(r.you as u8);
                    for i in 0..r.total.max(1) {
                        let (bg, border, w) = if i == 0 && r.you {
                            (AMBER, AMBER, 2.0)
                        } else if i >= r.you as u8 && i < r.you as u8 + others {
                            (AMBER_WASH_2, AMBER, 2.0)
                        } else {
                            (Color::NONE, LINE_2, 1.0)
                        };
                        let mut slot = row.spawn((Node { width: vh(40.0), height: vh(40.0), border: UiRect::all(px(w)), border_radius: BorderRadius::all(vh(4.0)), ..default() }, BackgroundColor(bg), BorderColor::all(border)));
                        if memory.changed(&format!("found.slot{i}"), &format!("{w}")) {
                            slot.insert(Anim::burst());
                        }
                    }
                });
                let m = meter(p, 544.0, AMBER, r.left / READY_SECONDS);
                p.commands().entity(m).entry::<Node>().and_modify(|mut n| n.margin = UiRect::top(vh(24.0)));
                p.spawn((t(kit, Type::Label, &format!("{} to accept", mmss(r.left)), TX2), Node { margin: UiRect::top(vh(8.0)), ..default() }));
                p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(12.0), margin: UiRect::top(vh(28.0)), ..default() }).with_children(|b| {
                    if r.you {
                        let waiting = r.total.saturating_sub(r.accepted);
                        b.spawn((soft(Node { height: vh(64.0), min_width: vh(520.0), padding: UiRect::horizontal(vh(24.0)), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween, border: UiRect { left: vh(6.0), right: px(1), top: px(1), bottom: px(1) }, ..default() }), BorderColor::all(AMBER))).with_children(|o| {
                            o.spawn(kit_text("ACCEPTED", 18.0, TX1, &kit.text));
                            o.spawn(t(kit, Type::Label, &format!("Waiting for {waiting}"), TX2));
                        });
                    } else {
                        let accept = primary_button(b, kit, "Accept", "Enter", UiAction::Lobby(LobbyAction::AcceptMatch));
                        b.commands().entity(accept).insert((Breathe, BoxShadow::default())).entry::<Node>().and_modify(|mut n| n.min_width = vh(520.0));
                        chip(b, kit, "Decline", UiAction::Lobby(LobbyAction::DeclineMatch), false);
                    }
                });
            });
        });
}

/// Top right under your name, stacked: your running match, then an invite.
pub fn notes(root: &mut ChildSpawnerCommands, kit: &Kit, lobby: &Lobby, steam: Option<&Steam>, top: f32) {
    root.spawn((Node { position_type: PositionType::Absolute, right: vh(96.0), top: vh(top), flex_direction: FlexDirection::Column, row_gap: vh(16.0), ..default() }, GlobalZIndex(10))).with_children(|col| {
        if lobby.shown.as_ref().is_some_and(|s| s.rejoin.is_some()) || std::env::var_os("BOOTLEG16_FOUND").is_some() {
            col.spawn((soft(Node { width: vh(480.0), flex_direction: FlexDirection::Column, row_gap: vh(4.0), padding: UiRect::axes(vh(20.0), vh(16.0)), border: UiRect::right(vh(BAR)), ..default() }), BackgroundColor(BG2), BorderColor::all(AMBER), lift())).with_children(|c| {
                c.spawn(t(kit, Type::Label, "Your match is still on", AMBER));
                c.spawn(kit_text("Your team is waiting: get back in before it ends", 18.0, TX1, &kit.text));
                c.spawn(Node { margin: UiRect::top(vh(12.0)), ..default() }).with_children(|b| {
                    primary_button(b, kit, "Reconnect", "", UiAction::Lobby(LobbyAction::Reconnect));
                });
            });
        }
        crate::play_page::toast(col, kit, lobby, steam);
    });
}

/// Find match while you may not queue: the button quiet, how long, and why.
pub fn cooldown(p: &mut ChildSpawnerCommands, kit: &Kit, label: &str, left: f32) {
    crate::ui::primary_off(p, kit, label, &format!("Queue again in {}", mmss(left)));
    p.spawn(t(kit, Type::Label, "After a missed or abandoned match", TX3));
}
