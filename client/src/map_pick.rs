//! The map of a custom game, on the Play page: one row under the mode. The leader picks a map
//! from the pool, or Random, and starts. No veto.
use crate::play::LobbyAction;
use crate::ui::{chip, kit_text, vh, Kit, UiAction, AMBER, TX2, TX3};
use bevy::prelude::*;
use net::lobby::LobbyState;
use net::server::Group;

/// The map row: the leader picks a pool map (or Random) and starts; the others see the pick.
pub fn row(col: &mut ChildSpawnerCommands, kit: &Kit, s: &LobbyState) {
    let a = |x: LobbyAction| UiAction::Lobby(x);
    col.spawn(Node { margin: UiRect { top: vh(32.0), bottom: vh(16.0), ..default() }, ..default() }).with_children(|h| {
        h.spawn(kit_text("MAP", 13.0, TX2, &kit.text));
    });
    col.spawn(Node { column_gap: vh(8.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
        if s.leader == s.you {
            chip(r, kit, "Random", a(LobbyAction::SetMap(None)), s.map.is_none());
            for m in net::config::MAP_POOL {
                chip(r, kit, m, a(LobbyAction::SetMap(Some(m.to_string()))), s.map.as_deref() == Some(*m));
            }
        } else {
            r.spawn(kit_text(s.map.clone().unwrap_or_else(|| "Random".into()).to_uppercase(), 16.0, AMBER, &kit.text));
            r.spawn(kit_text("THE LEADER PICKS THE MAP", 13.0, TX3, &kit.text));
        }
    });
}

/// Screenshot runs: BOOTLEG16_FAKE_LOBBY=custom shows a custom game you lead.
pub fn fake() -> Option<LobbyState> {
    std::env::var_os("BOOTLEG16_FAKE_LOBBY")?;
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
        regions: Vec::new(),
    })
}
