//! Drawing the Play screen (design/mockups/ui, Play): your party's slots, mode and the main
//! button on the left, Steam friends on the right, the identity card top right, and the invite
//! toast (over any menu page). Specs are at 1080p.
use crate::menu::{MenuState, Page, Settings};
use crate::play::{Lobby, LobbyAction};
use crate::steam::{Presence, Steam};
use crate::ui::{card, chip, chip_w, chips, kit_text, page_title, primary_button, vh, Choice, Kit, UiAction, AMBER, BAR, BG1, BG2, BG3, CT_COLOR, LINE, LINE_2, OK, TX1, TX2, TX3, T_COLOR};
use bevy::prelude::*;
use net::lobby::{Member, Mode, MAX_CUSTOM, MAX_PARTY};
use net::server::Group;

fn abs(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

fn label(p: &mut ChildSpawnerCommands, kit: &Kit, s: impl Into<String>, color: Color) {
    p.spawn((kit_text(s.into().to_uppercase(), 13.0, color, &kit.text), Pickable::IGNORE));
}

/// A square avatar: the Steam picture, or a letter tile without one. `dot`: presence colour.
fn avatar(p: &mut ChildSpawnerCommands, kit: &Kit, size: f32, picture: Option<&Handle<Image>>, name: &str, dot: Option<Color>) {
    let mut e = p.spawn((Node { width: vh(size), height: vh(size), flex_shrink: 0.0, justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() }, BackgroundColor(BG3), Pickable::IGNORE));
    e.with_children(|a| {
        match picture {
            Some(img) => _ = a.spawn((ImageNode::new(img.clone()), Node { width: percent(100), height: percent(100), ..default() }, Pickable::IGNORE)),
            None => {
                let initial: String = name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
                a.spawn((kit_text(initial, size * 0.4, TX1, &kit.num), Pickable::IGNORE));
            }
        }
        if let Some(c) = dot {
            a.spawn((abs(Node { right: vh(-3.0), bottom: vh(-3.0), width: vh(10.0), height: vh(10.0), ..default() }), BackgroundColor(c), Pickable::IGNORE));
        }
    });
}

fn picture<'a>(steam: Option<&'a Steam>, id: u64) -> Option<&'a Handle<Image>> {
    steam.and_then(|s| s.avatars.get(&id))
}

/// Top right: who you are (Steam avatar and name, or a letter tile without Steam).
pub fn identity(root: &mut ChildSpawnerCommands, kit: &Kit, settings: &Settings, steam: Option<&Steam>, top: f32) {
    let (name, sub) = match steam {
        Some(s) => (s.name.clone(), "Online · via Steam"),
        None => (settings.name.clone(), "Playing without Steam"),
    };
    root.spawn((abs(Node { right: vh(96.0), top: vh(top), height: vh(68.0), align_items: AlignItems::Center, column_gap: vh(16.0), padding: UiRect { left: vh(12.0), right: vh(24.0), ..default() }, ..default() }), BackgroundColor(BG1))).with_children(|c| {
        avatar(c, kit, 44.0, steam.and_then(|s| picture(Some(s), s.me)), &name, steam.map(|_| OK));
        c.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vh(4.0), ..default() }).with_children(|t| {
            t.spawn(kit_text(name, 18.0, TX1, &kit.text));
            label(t, kit, sub, TX2);
        });
    });
}

/// An invite to someone's party (in the notes under your name, `found::notes`): Accept /
/// Ignore and the time left.
pub fn toast(root: &mut ChildSpawnerCommands, kit: &Kit, lobby: &Lobby, steam: Option<&Steam>) {
    let Some(inv) = lobby.shown.as_ref().and_then(|s| s.invites.iter().rev().find(|i| !lobby.ignored.contains(&i.code) && i.code != s.code)) else { return };
    root.spawn((
        // on the right edge: the amber bar on the outer (right) side
        crate::ui::soft(Node { width: vh(480.0), flex_direction: FlexDirection::Column, row_gap: vh(16.0), padding: UiRect::axes(vh(20.0), vh(16.0)), border: UiRect::right(vh(BAR)), ..default() }),
        BackgroundColor(BG2),
        BorderColor::all(AMBER),
        crate::ui::lift(),
    ))
    .with_children(|t| {
        t.spawn(Node { column_gap: vh(16.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            avatar(r, kit, 44.0, picture(steam, inv.steam), &inv.from, None);
            r.spawn(Node { flex_direction: FlexDirection::Column, row_gap: vh(6.0), ..default() }).with_children(|c| {
                label(c, kit, "Party invite", AMBER);
                c.spawn(kit_text(format!("{} invited you to their party", inv.from), 18.0, TX1, &kit.text));
            });
        });
        t.spawn(Node { column_gap: vh(8.0), align_items: AlignItems::Center, padding: UiRect::left(vh(60.0)), ..default() }).with_children(|r| {
            chip(r, kit, "Accept", UiAction::Lobby(LobbyAction::Accept(inv.code.clone())), true);
            chip(r, kit, "Ignore", UiAction::Lobby(LobbyAction::Ignore(inv.code.clone())), false);
            r.spawn((Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd, ..default() }, Pickable::IGNORE)).with_children(|c| {
                let s = inv.left.max(0.0) as u32;
                label(c, kit, format!("{}:{:02}", s / 60, s % 60), TX3);
            });
        });
    });
}

/// A framed choice; without an action, a framed label in `fg` (the mode, for members).
fn small_button(p: &mut ChildSpawnerCommands, kit: &Kit, text: &str, fg: Color, action: Option<UiAction>) {
    match action {
        Some(a) => chip(p, kit, text, a, false),
        None => {
            p.spawn((Node { height: vh(44.0), padding: UiRect::horizontal(vh(18.0)), align_items: AlignItems::Center, border: UiRect::all(px(1)), ..default() }, BorderColor::all(LINE_2))).with_children(|b| label(b, kit, text, fg));
        }
    }
}

/// The whole Play screen.
pub fn screen(root: &mut ChildSpawnerCommands, kit: &Kit, st: &MenuState, lobby: &Lobby, settings: &Settings, steam: Option<&Steam>) {
    let a = |x: LobbyAction| UiAction::Lobby(x);
    let mode = lobby.shown.as_ref().map(|s| s.mode).unwrap_or(Mode::Casual);
    root.spawn(abs(Node { left: vh(96.0), top: vh(72.0), flex_direction: FlexDirection::Column, ..default() })).with_children(|t| {
        page_title(t, kit, if mode == Mode::Custom { "Play · Custom game" } else { "Play · Casual 5v5" }, "Party", "パーティー");
    });
    identity(root, kit, settings, steam, 72.0);
    root.spawn((abs(Node { left: vh(96.0), right: vh(96.0), top: vh(160.0), height: px(1), ..default() }), BackgroundColor(LINE)));
    // back to the main menu (Esc does the same)
    root.spawn(abs(Node { left: vh(96.0), bottom: vh(48.0), ..default() })).with_children(|b| chip(b, kit, "Back · Esc", UiAction::Page(Page::Main), false));
    friends(root, kit, lobby, steam);

    // Left column: party, mode, the big button, joining by code.
    root.spawn(abs(Node { left: vh(96.0), top: vh(200.0), width: vh(1200.0), flex_direction: FlexDirection::Column, ..default() })).with_children(|col| {
        let Some(s) = &lobby.shown else {
            label(col, kit, format!("Connecting to the lobby at {}...", lobby.address), TX2);
            field(col, kit, st, "Or connect to a server address (testing)");
            return;
        };
        let leader = s.leader == s.you;
        let max = if s.mode == Mode::Custom { MAX_CUSTOM } else { MAX_PARTY };
        col.spawn(Node { margin: UiRect::bottom(vh(16.0)), column_gap: vh(16.0), align_items: AlignItems::Center, ..default() }).with_children(|r| {
            label(r, kit, format!("Your party · {} of {max} · code {}", s.members.len(), s.code), TX2);
            if s.members.len() > 1 {
                small_button(r, kit, "Leave party", TX2, Some(a(LobbyAction::LeaveParty)));
            }
        });
        match s.mode {
            Mode::Casual => slots(col, kit, s, &s.members.iter().collect::<Vec<_>>(), MAX_PARTY, 280.0, None, steam),
            Mode::Custom => {
                for (group, title, color) in [(Group::A, "Team A · starts Strays", T_COLOR), (Group::B, "Team B · starts Bureau", CT_COLOR)] {
                    col.spawn(Node { margin: UiRect::vertical(vh(8.0)), ..default() }).with_children(|r| label(r, kit, title, color));
                    let members: Vec<&Member> = s.members.iter().filter(|m| m.group == group).collect();
                    slots(col, kit, s, &members, 5, 196.0, Some(group), steam);
                }
            }
        }
        label_block(col, kit, "Mode", 40.0);
        col.spawn(Node { ..default() }).with_children(|r| {
            if leader {
                chips(r, kit, "play.mode", vec![Choice::new("Casual queue", a(LobbyAction::SetMode(Mode::Casual)), s.mode == Mode::Casual), Choice::new("Custom game · 10 players", a(LobbyAction::SetMode(Mode::Custom)), s.mode == Mode::Custom)]);
            } else {
                small_button(r, kit, if s.mode == Mode::Custom { "Custom game · 10 players" } else { "Casual queue" }, AMBER, None);
            }
        });
        col.spawn(Node { column_gap: vh(24.0), align_items: AlignItems::Center, margin: UiRect::top(vh(40.0)), ..default() }).with_children(|r| match s.searching {
            Some(t) => {
                let t = t as u32;
                r.spawn(kit_text(format!("SEARCHING {}:{:02}", t / 60, t % 60), 30.0, OK, &kit.display));
                if leader {
                    small_button(r, kit, "Cancel", TX1, Some(a(LobbyAction::Cancel)));
                }
            }
            None => {
                let (text, hint) = if s.mode == Mode::Custom { ("Start game", "Knife round first: the winners pick their side") } else { ("Find match", "10 players make a match, your party stays together") };
                if let Some(left) = s.cooldown {
                    crate::found::cooldown(r, kit, text, left);
                } else if leader {
                    primary_button(r, kit, text, "Enter", a(LobbyAction::Play));
                    label(r, kit, hint, TX3);
                } else {
                    label(r, kit, "Only the party leader can start the queue", TX3);
                }
            }
        });
        if !lobby.notice.is_empty() {
            col.spawn((kit_text(lobby.notice.clone(), 15.0, AMBER, &kit.text), Node { margin: UiRect::top(vh(16.0)), ..default() }));
        }
        field(col, kit, st, "Join with a party code, Enter");
    });
}

fn label_block(p: &mut ChildSpawnerCommands, kit: &Kit, s: &str, top: f32) {
    p.spawn(Node { margin: UiRect { top: vh(top), bottom: vh(16.0), ..default() }, ..default() }).with_children(|b| label(b, kit, s, TX2));
}

/// A row of party slots: members first, then empty "invite" slots up to `count`.
fn slots(p: &mut ChildSpawnerCommands, kit: &Kit, s: &net::lobby::LobbyState, members: &[&Member], count: usize, height: f32, group: Option<Group>, steam: Option<&Steam>) {
    let leader = s.leader == s.you;
    let side = if height > 200.0 { 96.0 } else { 64.0 };
    p.spawn(Node { column_gap: vh(16.0), ..default() }).with_children(|row| {
        for i in 0..count {
            let base = Node { flex_grow: 1.0, flex_basis: px(0), height: vh(height), flex_direction: FlexDirection::Column, row_gap: vh(16.0), padding: UiRect::all(vh(24.0)), border: UiRect::all(px(1)), ..default() };
            let Some(m) = members.get(i) else {
                // ponytail: the mockup's dashed outline; Bevy UI has no dashed borders, so solid LINE.
                row.spawn((base, BorderColor::all(LINE))).with_children(|c| {
                    c.spawn((Node { width: vh(side), height: vh(side), justify_content: JustifyContent::Center, align_items: AlignItems::Center, border: UiRect::all(px(1)), ..default() }, BorderColor::all(LINE))).with_children(|b| {
                        b.spawn(kit_text("+", 28.0, TX3, &kit.num));
                    });
                    label(c, kit, "Invite a friend", TX3);
                });
                continue;
            };
            let is_leader = m.id == s.leader;
            row.spawn(card(base, is_leader)).with_children(|c| {
                avatar(c, kit, side, picture(steam, m.steam), &m.name, None);
                c.spawn(kit_text(m.name.clone(), if height > 200.0 { 28.0 } else { 22.0 }, TX1, &kit.text));
                let you = if m.id == s.you { " · you" } else { "" };
                if is_leader {
                    label(c, kit, format!("Leader{you}"), AMBER);
                } else {
                    label(c, kit, format!("In party{you}"), TX2);
                }
                // Custom games: the leader moves people between the teams.
                if let (Some(g), true) = (group, leader) {
                    let (text, to) = if g == Group::A { ("To team B", Group::B) } else { ("To team A", Group::A) };
                    small_button(c, kit, text, TX2, Some(UiAction::Lobby(LobbyAction::Move(m.id, to))));
                }
            });
        }
    });
}

/// The text field: a party code (or, for testing, a server address).
fn field(p: &mut ChildSpawnerCommands, kit: &Kit, st: &MenuState, title: &str) {
    label_block(p, kit, title, 40.0);
    p.spawn((Node { width: vh(520.0), height: vh(56.0), align_items: AlignItems::Center, padding: UiRect::horizontal(vh(18.0)), border: UiRect::left(vh(BAR)), ..default() }, BackgroundColor(BG1), BorderColor::all(LINE_2)))
        .with_children(|f| {
            f.spawn(kit_text(format!("{}_", st.address), 22.0, TX1, &kit.num));
        });
}

/// Right column: Steam friends, in game / online / offline, with invite buttons.
fn friends(root: &mut ChildSpawnerCommands, kit: &Kit, lobby: &Lobby, steam: Option<&Steam>) {
    root.spawn((abs(Node { left: vh(1344.0), right: vh(96.0), top: vh(200.0), bottom: vh(120.0), flex_direction: FlexDirection::Column, overflow: Overflow::clip(), ..default() }), BackgroundColor(BG1))).with_children(|panel| {
        let list = steam.map(|s| s.friends.as_slice()).unwrap_or_default();
        let count = |p: Presence| list.iter().filter(|f| f.presence == p).count();
        panel.spawn((Node { align_items: AlignItems::Baseline, padding: UiRect { left: vh(24.0), right: vh(24.0), top: vh(20.0), bottom: vh(16.0) }, border: UiRect::bottom(px(1)), ..default() }, BorderColor::all(LINE))).with_children(|h| {
            h.spawn(kit_text("FRIENDS", 28.0, TX1, &kit.display));
            h.spawn(Node { flex_grow: 1.0, justify_content: JustifyContent::FlexEnd, ..default() }).with_children(|r| {
                if steam.is_some() {
                    label(r, kit, format!("{} in game · {} online", count(Presence::InGame), count(Presence::Online)), TX2);
                }
            });
        });
        let Some(steam) = steam else {
            panel.spawn(Node { padding: UiRect::all(vh(24.0)), ..default() }).with_children(|t| {
                t.spawn(kit_text("Start Steam to see your friends and invite them. Party codes work without it.", 15.0, TX2, &kit.text));
            });
            return;
        };
        let in_party: Vec<u64> = lobby.shown.as_ref().map(|s| s.members.iter().map(|m| m.steam).collect()).unwrap_or_default();
        let code = lobby.shown.as_ref().map(|s| s.code.as_str()).unwrap_or("");
        panel.spawn(Node { flex_direction: FlexDirection::Column, padding: UiRect::vertical(vh(8.0)), ..default() }).with_children(|rows| {
            for (presence, title, dot) in [(Presence::InGame, "In game", AMBER), (Presence::Online, "Online", OK), (Presence::Offline, "Offline", TX3)] {
                let group: Vec<_> = steam.friends.iter().filter(|f| f.presence == presence).collect();
                if group.is_empty() {
                    continue;
                }
                rows.spawn(Node { padding: UiRect { left: vh(24.0), right: vh(24.0), top: vh(16.0), bottom: vh(8.0) }, ..default() }).with_children(|h| label(h, kit, format!("{title} · {}", group.len()), TX3));
                // ponytail: no scrolling yet; offline friends are capped so the panel fits.
                let shown = if presence == Presence::Offline { 6 } else { 40 };
                for f in group.into_iter().take(shown) {
                    let offline = presence == Presence::Offline;
                    let (name_c, status_c) = if offline { (TX3, TX3) } else { (TX1, TX2) };
                    rows.spawn(Node { height: vh(64.0), flex_shrink: 0.0, align_items: AlignItems::Center, column_gap: vh(16.0), padding: UiRect::horizontal(vh(24.0)), ..default() }).with_children(|r| {
                        avatar(r, kit, 44.0, picture(Some(steam), f.id), &f.name, Some(if f.away { TX2 } else { dot }));
                        r.spawn(Node { flex_grow: 1.0, flex_direction: FlexDirection::Column, row_gap: vh(4.0), overflow: Overflow::clip(), ..default() }).with_children(|t| {
                            t.spawn(kit_text(f.name.clone(), 18.0, name_c, &kit.text));
                            let status = if in_party.contains(&f.id) { "In your party".to_string() } else { f.status.clone() };
                            t.spawn(kit_text(status, 13.0, status_c, &kit.text));
                        });
                        if !offline {
                            r.spawn(Node { width: vh(112.0), justify_content: JustifyContent::Center, ..default() }).with_children(|b| {
                                if in_party.contains(&f.id) {
                                    label(b, kit, "In party", TX3);
                                } else if lobby.invited.contains_key(&f.id) {
                                    label(b, kit, "Invited", TX3);
                                } else if !code.is_empty() {
                                    chip_w(b, kit, "Invite", UiAction::Lobby(LobbyAction::Invite(f.id, presence == Presence::InGame)), 112.0);
                                }
                            });
                        }
                    });
                }
            }
        });
    });
}
