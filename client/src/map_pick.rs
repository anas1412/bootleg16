//! The map of a custom game, on the Play page: one row under the mode. The leader picks a map
//! from the pool, or Random, or starts a veto (it needs someone on each team). During a veto
//! the teams' captains (each team's first member) ban in turn until one map is left; banned
//! maps stay in the row, struck out, and the row says whose turn it is.
use crate::play::LobbyAction;
use crate::ui::{chip, kit_text, status_label, vh, Kit, UiAction, AMBER, CT_COLOR, LINE_2, TX2, TX3, T_COLOR};
use bevy::prelude::*;
use net::lobby::LobbyState;
use net::server::Group;

/// Team A starts Strays (pink), team B Bureau (cyan): their colours on the Play page.
fn group_color(g: Group) -> Color {
    if g == Group::A { T_COLOR } else { CT_COLOR }
}

/// The map row.
pub fn row(col: &mut ChildSpawnerCommands, kit: &Kit, s: &LobbyState) {
    let a = |x: LobbyAction| UiAction::Lobby(x);
    let leader = s.leader == s.you;
    let pool = net::config::MAP_POOL;
    col.spawn(Node { margin: UiRect { top: vh(32.0), bottom: vh(16.0), ..default() }, column_gap: vh(16.0), align_items: AlignItems::Center, ..default() }).with_children(|h| {
        h.spawn(kit_text("MAP", 13.0, TX2, &kit.text));
        if let Some(v) = &s.veto {
            // whose turn: the captain is that team's first member
            let captain = s.members.iter().find(|m| m.group == v.turn).map(|m| m.id);
            let team = if v.turn == Group::A { "Team A" } else { "Team B" };
            let yours = captain == Some(s.you);
            status_label(h, kit, &format!("Veto · {team} bans{}", if yours { " · your turn" } else { "" }), if yours { AMBER } else { group_color(v.turn) });
        }
    });
    col.spawn(Node { column_gap: vh(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| match &s.veto {
        Some(v) => {
            let my_turn = s.members.iter().find(|m| m.group == v.turn).map(|m| m.id) == Some(s.you);
            for m in pool {
                if !v.left.iter().any(|l| l == m) {
                    // banned: quiet and struck out
                    r.spawn((Node { height: vh(44.0), padding: UiRect::horizontal(vh(18.0)), align_items: AlignItems::Center, border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(crate::ui::RADIUS_S)), ..default() }, BorderColor::all(LINE_2.with_alpha(0.12))))
                        .with_children(|b| {
                            b.spawn((kit_text(m.to_uppercase(), 13.0, TX3, &kit.text), Strikethrough));
                        });
                } else if my_turn {
                    chip(r, kit, &format!("Ban {m}"), a(LobbyAction::Ban(m.to_string())), false);
                } else {
                    // still in, not yours to ban: shown, not pressable
                    r.spawn((Node { height: vh(44.0), padding: UiRect::horizontal(vh(18.0)), align_items: AlignItems::Center, border: UiRect::all(px(1)), border_radius: BorderRadius::all(vh(crate::ui::RADIUS_S)), ..default() }, BorderColor::all(LINE_2)))
                        .with_children(|b| {
                            b.spawn(kit_text(m.to_uppercase(), 13.0, TX2, &kit.text));
                        });
                }
            }
        }
        None if leader => {
            chip(r, kit, "Random", a(LobbyAction::SetMap(None)), s.map.is_none());
            for m in pool {
                chip(r, kit, m, a(LobbyAction::SetMap(Some(m.to_string()))), s.map.as_deref() == Some(*m));
            }
            // a veto needs someone on each team
            let both = [Group::A, Group::B].iter().all(|g| s.members.iter().any(|m| m.group == *g));
            if both {
                r.spawn(Node { width: px(1), height: vh(28.0), margin: UiRect::horizontal(vh(8.0)), ..default() }).insert(BackgroundColor(LINE_2));
                chip(r, kit, "Map veto", a(LobbyAction::StartVeto), false);
            }
        }
        None => {
            r.spawn(kit_text(s.map.clone().unwrap_or_else(|| "Random".into()).to_uppercase(), 16.0, AMBER, &kit.text));
            r.spawn(kit_text("THE LEADER PICKS THE MAP", 13.0, TX3, &kit.text));
        }
    });
}

/// Screenshot runs: BOOTLEG16_FAKE_LOBBY=veto shows a custom game mid-veto (you lead team A and
/// it's your turn), BOOTLEG16_FAKE_LOBBY=custom the same party before the veto.
pub fn fake() -> Option<LobbyState> {
    let kind = std::env::var("BOOTLEG16_FAKE_LOBBY").ok()?;
    let member = |id: u32, name: &str, group: Group| net::lobby::Member { id, name: name.into(), group, steam: 0 };
    Some(LobbyState {
        you: 1,
        code: "KX7-42Q".into(),
        leader: 1,
        mode: net::lobby::Mode::Custom,
        members: vec![member(1, "anas", Group::A), member(2, "kova", Group::A), member(3, "vera", Group::B), member(4, "rook", Group::B)],
        searching: None,
        ready: None,
        cooldown: None,
        found: None,
        rejoin: None,
        notice: String::new(),
        invites: Vec::new(),
        map: None,
        veto: (kind == "veto").then(|| net::lobby::Veto { left: vec!["de_dust2".into(), "de_inferno".into(), "de_train".into()], turn: Group::A }),
        regions: Vec::new(),
    })
}
