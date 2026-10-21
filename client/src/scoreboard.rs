//! The scoreboard (hold Tab): match line, the round-history strip (who won each round and how),
//! then one block per side with its colour bar, name and rounds won, and a row per player.
//! Your row has the amber bar; dead players are dimmed. It tunes in when Tab goes down and out
//! when it comes up. Reports: while holding Tab, right-click frees the mouse; click a player,
//! pick a reason. The server takes one report per player and five per match and says nothing
//! back, so the client remembers who it reported.
use crate::game::{Frame, Walker};
use crate::hud::Icons;
use crate::menu::AppState;
use crate::ui::{chip, clickable, kit_text, vh, Anim, Blurs, Kit, UiAction, UiActions, AMBER, BAR, BG1, CT_COLOR, LINE, TX1, TX2, TX3, T_COLOR};
use net::protocol::ReportReason;
use std::collections::HashSet;
use bevy::prelude::*;
use net::protocol::{Score, Team};
use net::rounds::EndReason;

pub struct ScoreboardPlugin;

impl Plugin for ScoreboardPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ScoreMouse>()
            .init_resource::<Reported>()
            .init_resource::<Kicked>()
            .add_systems(OnEnter(AppState::InGame), |mut r: ResMut<Reported>| r.0.clear())
            .add_systems(Update, (mouse, report_actions, scoreboard).chain().in_set(Frame::Ui).run_if(resource_exists::<Walker>));
    }
}

#[derive(Component)]
struct ScoresRoot;

/// While Tab is held: whether the mouse is free (right-click), and whose report menu is open.
#[derive(Resource, Default)]
pub struct ScoreMouse {
    pub on: bool,
    open: Option<u8>,
}

/// The round you last called a kick vote in (one per round).
#[derive(Resource, Default)]
struct Kicked(Option<u32>);

/// Who you reported this match.
#[derive(Resource, Default)]
struct Reported(HashSet<u8>);

/// Right-click while holding the scoreboard frees the mouse for it (overlay.rs moves the mouse).
fn mouse(keys: Res<ButtonInput<KeyCode>>, buttons: Res<ButtonInput<MouseButton>>, binds: Res<crate::binds::Binds>, mut m: ResMut<ScoreMouse>) {
    if binds.held(crate::binds::Action::Scores, &keys, &buttons) {
        if buttons.just_pressed(MouseButton::Right) {
            m.on = !m.on;
        }
    } else if m.on || m.open.is_some() {
        *m = ScoreMouse::default();
    }
}

fn report_actions(mut actions: ResMut<UiActions>, mut m: ResMut<ScoreMouse>, mut reported: ResMut<Reported>, mut kicked: ResMut<Kicked>, mut w: ResMut<Walker>, mut voice: ResMut<crate::voice::VoiceSettings>) {
    actions.0.retain(|a| match a {
        UiAction::Mute(name) => {
            let g = voice.player_gain(name);
            voice.set_player_gain(name, if g == 0.0 { 1.0 } else { 0.0 });
            false
        }
        UiAction::PlayerVolume(name, d) => {
            let g = voice.player_gain(name);
            voice.set_player_gain(name, g + d);
            false
        }
        UiAction::Kick(id) => {
            w.client.call_kick(*id);
            kicked.0 = Some(w.client.round.round);
            m.open = None;
            false
        }
        UiAction::ReportMenu(id) => {
            m.open = if m.open == Some(*id) { None } else { Some(*id) };
            false
        }
        UiAction::Report(id, reason) => {
            w.client.report(*id, *reason);
            reported.0.insert(*id);
            m.open = None;
            false
        }
        _ => true,
    });
}

/// Slots in the strip: a full regulation match (first to 13).
const SLOTS: usize = 24;

/// The stat columns after the name (which grows): label and width at 1080p. Money is only
/// known for your own team (blank for the other).
const COLS: [(&str, f32); 8] = [("K", 56.0), ("D", 56.0), ("A", 56.0), ("ADR", 72.0), ("HS%", 72.0), ("MVP", 64.0), ("$", 96.0), ("PING", 72.0)];

type Shown = (Vec<Score>, [u32; 2], usize, bool, Option<u8>, usize);

#[allow(clippy::too_many_arguments)]
fn scoreboard(mut commands: Commands, (keys, buttons, binds): (Res<ButtonInput<KeyCode>>, Res<ButtonInput<MouseButton>>, Res<crate::binds::Binds>), w: Res<Walker>, kit: Res<Kit>, icons: Res<Icons>, m: Res<ScoreMouse>, (reported, kicked, voice): (Res<Reported>, Res<Kicked>, Res<crate::voice::VoiceSettings>), roots: Query<Entity, With<ScoresRoot>>, mut shown: Local<Option<Shown>>) {
    let auto_show = std::env::var_os("BOOTLEG16_SCORES").is_some();
    let want = (binds.held(crate::binds::Action::Scores, &keys, &buttons) || auto_show).then(|| (w.client.scores.clone(), w.client.round.score, w.client.round_history.len(), m.on, m.open, reported.0.len()));
    // a voice volume changed: redraw too
    if *shown == want && !voice.is_changed() {
        return;
    }
    let opening = shown.is_none();
    *shown = want.clone();
    for e in &roots {
        if want.is_none() {
            commands.entity(e).remove::<(ScoresRoot, Blurs)>().insert(Anim::tune_out());
        } else {
            commands.entity(e).despawn();
        }
    }
    let Some((scores, won, ..)) = want else { return };
    let me = w.client.id();
    let r = &w.client.round;
    let line = if w.client.practice { "PRACTICE".to_string() } else { format!("ROUND {}", r.round.max(1)) };
    // kicking: teammates, reserved matches, one call per round, not while a vote runs
    let can_kick = m.on && w.client.ticket != 0 && kicked.0 != Some(r.round) && r.vote.is_none();
    let ctx = Ctx { me, my_team: w.client.team(), carrier: match w.client.bomb { net::protocol::BombNet::Carried(id) => Some(id), _ => None }, my_kit: w.client.me_info.defuse_kit, can_report: m.on && !w.client.practice, can_report_or_mouse: m.on, can_kick, open: m.open, reported: &reported.0, voice: &voice };
    let tune = |i: usize| opening.then(|| Anim::stagger(i));
    commands
        .spawn((
            ScoresRoot,
            DespawnOnExit(AppState::InGame),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, padding: UiRect::top(vh(160.0)), ..default() },
            Blurs(0.85),
            GlobalZIndex(7),
        ))
        .with_children(|root| {
            root.spawn(Node { width: vh(1200.0), max_width: percent(94.0), flex_direction: FlexDirection::Column, ..default() }).with_children(|col| {
                let mut head = col.spawn(Node { align_items: AlignItems::Center, ..default() });
                if let Some(a) = tune(0) {
                    head.insert(a);
                }
                head.with_children(|h| {
                    h.spawn((kit_text(format!("SCOREBOARD · {line}"), 13.0, TX2, &kit.text), Node { flex_grow: 1.0, ..default() }));
                    if !w.client.practice {
                        h.spawn(kit_text(if m.on { "CLICK A PLAYER: VOICE, REPORT · RIGHT-CLICK TO LOCK THE MOUSE" } else { "RIGHT-CLICK TO USE THE MOUSE" }, 13.0, TX3, &kit.text));
                    }
                });
                if !w.client.practice {
                    let mut s = col.spawn(Node { flex_direction: FlexDirection::Column, ..default() });
                    if let Some(a) = tune(1) {
                        s.insert(a);
                    }
                    s.with_children(|s| strip(s, &icons, &w.client.round_history));
                }
                // Rounds won: [terrorists, counter-terrorists].
                for (i, (team, name, jp, color, rounds)) in [(Team::Ct, "Bureau", "ビューロー", CT_COLOR, won[1]), (Team::T, "Strays", "ストレイズ", T_COLOR, won[0])].into_iter().enumerate() {
                    let players: Vec<&Score> = scores.iter().filter(|s| s.team == team).collect();
                    let mut b = col.spawn(Node { flex_direction: FlexDirection::Column, ..default() });
                    if let Some(a) = tune(2 + i) {
                        b.insert(a);
                    }
                    b.with_children(|b| team_block(b, &kit, name, jp, color, rounds, &players, &ctx));
                }
                let specs: Vec<&str> = scores.iter().filter(|s| s.team == Team::Spectator).map(|s| s.name.as_str()).collect();
                if !specs.is_empty() {
                    col.spawn((kit_text(format!("SPECTATORS · {}", specs.join(", ")), 13.0, TX3, &kit.text), Node { margin: UiRect::top(vh(24.0)), ..default() }));
                }
            });
        });
}

/// What a row needs to know beyond its player.
struct Ctx<'a> {
    me: Option<u8>,
    my_team: Team,
    carrier: Option<u8>,
    my_kit: bool,
    can_report: bool,
    /// The mouse is free (the row menu can open).
    can_report_or_mouse: bool,
    can_kick: bool,
    open: Option<u8>,
    reported: &'a HashSet<u8>,
    voice: &'a crate::voice::VoiceSettings,
}

/// One team: header bar (side colour, name, rounds won), column labels, then the players.
#[allow(clippy::too_many_arguments)]
fn team_block(col: &mut ChildSpawnerCommands, kit: &Kit, name: &str, jp: &str, color: Color, rounds: u32, players: &[&Score], ctx: &Ctx) {
    col.spawn((Node { margin: UiRect::top(vh(32.0)), align_items: AlignItems::Stretch, border: UiRect::left(vh(BAR)), ..default() }, BackgroundColor(BG1), BorderColor::all(color))).with_children(|h| {
        h.spawn(Node { flex_grow: 1.0, align_items: AlignItems::Baseline, column_gap: vh(16.0), padding: UiRect::axes(vh(24.0), vh(14.0)), ..default() }).with_children(|t| {
            t.spawn(kit_text(name.to_uppercase(), 40.0, TX1, &kit.display));
            t.spawn(kit_text(jp, 13.0, TX3, &kit.jp));
            t.spawn(kit_text(match players.len() { 1 => "1 PLAYER".to_string(), n => format!("{n} PLAYERS") }, 13.0, TX2, &kit.text));
        });
        h.spawn(Node { align_items: AlignItems::Center, padding: UiRect::horizontal(vh(24.0)), ..default() }).with_children(|n| {
            n.spawn(kit_text(format!("{rounds:02}"), 32.0, color, &kit.num));
        });
    });
    header(col, kit);
    if players.is_empty() {
        col.spawn((Node { height: vh(48.0), align_items: AlignItems::Center, padding: UiRect::left(vh(120.0)), border: UiRect::top(px(1)), ..default() }, BorderColor::all(LINE))).with_children(|r| {
            r.spawn(kit_text("No one on this side yet", 16.0, TX3, &kit.text));
        });
    }
    for s in players {
        player_row(col, kit, s, ctx);
    }
}

fn header(col: &mut ChildSpawnerCommands, kit: &Kit) {
    col.spawn(Node { height: vh(36.0), align_items: AlignItems::Center, padding: UiRect::horizontal(vh(24.0)), ..default() }).with_children(|r| {
        r.spawn(Node { width: vh(96.0), ..default() });
        r.spawn((kit_text("PLAYER", 13.0, TX2, &kit.text), Node { flex_grow: 1.0, ..default() }));
        for (c, w) in COLS {
            r.spawn((kit_text(c, 13.0, TX2, &kit.text), Node { width: vh(w), justify_content: JustifyContent::FlexEnd, ..default() }));
        }
    });
}

/// One player: status tag, name, kills, deaths. Yours has the amber bar. With the mouse free,
/// other players' rows open the report menu under them.
fn player_row(col: &mut ChildSpawnerCommands, kit: &Kit, s: &Score, ctx: &Ctx) {
    let mine = Some(s.id) == ctx.me;
    let c = if !s.alive { TX3 } else { TX1 };
    // Status: dead, carrying the bomb (only your own team is told), your kit, a bot.
    let (tag, tag_color) = if !s.alive {
        ("DEAD", TX3)
    } else if ctx.carrier == Some(s.id) {
        ("C4", T_COLOR)
    } else if mine && ctx.my_kit {
        ("KIT", CT_COLOR)
    } else if s.bot {
        ("BOT", TX3)
    } else {
        ("", TX3)
    };
    let reported = ctx.reported.contains(&s.id);
    let gain = ctx.voice.player_gain(&s.name);
    let base = if mine { BG1.mix(&AMBER, 0.08) } else { Color::NONE };
    // your bar sits inside the padding, so every row's content lines up
    let node = Node { height: vh(48.0), align_items: AlignItems::Center, padding: UiRect { left: vh(if mine { 24.0 - BAR } else { 24.0 }), right: vh(24.0), ..default() }, border: UiRect { left: vh(if mine { BAR } else { 0.0 }), top: px(1), ..default() }, ..default() };
    let mut e = col.spawn((node, BorderColor { left: AMBER, top: LINE, ..BorderColor::all(Color::NONE) }, BackgroundColor(base)));
    e.with_children(|r| {
        r.spawn((Node { width: vh(96.0), ..default() }, Pickable::IGNORE)).with_children(|t| {
            if !tag.is_empty() {
                crate::ui::status_label(t, kit, tag, tag_color);
            }
        });
        r.spawn((kit_text(&s.name, 19.0, c, &kit.text), Node { flex_grow: 1.0, ..default() }, Pickable::IGNORE));
        if gain == 0.0 {
            r.spawn((kit_text("MUTED", 13.0, TX3, &kit.text), Node { margin: UiRect::right(vh(16.0)), ..default() }, Pickable::IGNORE));
        }
        if reported {
            r.spawn((kit_text("REPORTED", 13.0, TX3, &kit.text), Node { margin: UiRect::right(vh(16.0)), ..default() }, Pickable::IGNORE));
        }
        let pct = |n: u32, of: i32| if of > 0 { format!("{}", n * 100 / of as u32) } else { "0".into() };
        let values = [
            s.kills.to_string(),
            s.deaths.to_string(),
            s.assists.to_string(),
            (s.damage / s.rounds.max(1)).to_string(),
            pct(s.headshots, s.kills),
            s.mvps.to_string(),
            s.money.map(|m| format!("${m}")).unwrap_or_default(),
            if s.bot { String::new() } else { s.ping.to_string() },
        ];
        for (v, (_, w)) in values.into_iter().zip(COLS) {
            r.spawn((kit_text(v, 18.0, c, &kit.num), Node { width: vh(w), justify_content: JustifyContent::FlexEnd, ..default() }, Pickable::IGNORE));
        }
    });
    let report = ctx.can_report && !mine && !s.bot && !reported;
    let kick = ctx.can_kick && !mine && !s.bot && s.team == ctx.my_team;
    // their voice: mute or set how loud (people only)
    let voice = ctx.can_report_or_mouse && !mine && !s.bot;
    if report || kick || voice {
        clickable(&mut e, UiAction::ReportMenu(s.id), base, false);
    }
    if ctx.open == Some(s.id) {
        col.spawn((Node { height: vh(56.0), align_items: AlignItems::Center, column_gap: vh(8.0), padding: UiRect::left(vh(120.0)), ..default() }, Anim::tune_in(0.0))).with_children(|m| {
            if voice {
                m.spawn((kit_text("VOICE", 13.0, TX2, &kit.text), Node { margin: UiRect::right(vh(8.0)), ..default() }));
                chip(m, kit, if gain == 0.0 { "Unmute" } else { "Mute" }, UiAction::Mute(s.name.clone()), gain == 0.0);
                chip(m, kit, "−", UiAction::PlayerVolume(s.name.clone(), -0.25), false);
                m.spawn((kit_text(format!("{:.0}%", gain * 100.0), 16.0, TX1, &kit.num), Node { width: vh(56.0), justify_content: JustifyContent::Center, ..default() }));
                chip(m, kit, "+", UiAction::PlayerVolume(s.name.clone(), 0.25), false);
                m.spawn((Node { width: px(1), height: vh(28.0), margin: UiRect::horizontal(vh(8.0)), ..default() }, BackgroundColor(LINE)));
            }
            if report {
                m.spawn((kit_text(format!("REPORT {} FOR", s.name.to_uppercase()), 13.0, TX2, &kit.text), Node { margin: UiRect::right(vh(8.0)), ..default() }));
                for (label, reason) in [("Cheating", ReportReason::Cheating), ("Griefing", ReportReason::Griefing), ("Abuse", ReportReason::Abuse)] {
                    chip(m, kit, label, UiAction::Report(s.id, reason), false);
                }
            }
            if kick {
                // apart from the reports: it's a team vote, not a review
                m.spawn((Node { width: px(1), height: vh(28.0), margin: UiRect::horizontal(vh(8.0)), ..default() }, BackgroundColor(LINE)));
                chip(m, kit, "Vote to kick", UiAction::Kick(s.id), false);
            }
            chip(m, kit, "Cancel", UiAction::ReportMenu(s.id), false);
        });
    }
}

/// One slot per round: tinted by the winner with an icon for how it ended; the round
/// being played has an amber outline; future rounds are empty frames.
fn strip(col: &mut ChildSpawnerCommands, icons: &Icons, rounds: &[(Team, EndReason)]) {
    col.spawn(Node { margin: UiRect::top(vh(16.0)), column_gap: vh(4.0), ..default() }).with_children(|row| {
        for i in 0..SLOTS {
            let slot = Node { flex_grow: 1.0, flex_basis: px(0), height: vh(40.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() };
            match rounds.get(i) {
                Some(&(winner, reason)) => {
                    let c = if winner == Team::Ct { CT_COLOR } else { T_COLOR };
                    let icon = match reason {
                        EndReason::Eliminated => "d_skull",
                        EndReason::BombExploded => "c4",
                        EndReason::BombDefused => "defuser",
                        EndReason::TimeUp => "stopwatch",
                    };
                    row.spawn((Node { border: UiRect::bottom(vh(BAR)), ..slot }, BackgroundColor(BG1), BorderColor::all(c))).with_children(|s| {
                        s.spawn(icons.node(icon, 24.0, c));
                    });
                }
                None if i == rounds.len() => {
                    row.spawn((Node { border: UiRect::all(vh(2.0)), ..slot }, BorderColor::all(AMBER)));
                }
                None => {
                    row.spawn((Node { border: UiRect::all(px(1)), ..slot }, BorderColor::all(LINE)));
                }
            }
        }
    });
}
