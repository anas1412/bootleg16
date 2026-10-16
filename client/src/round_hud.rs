//! The round on screen: score and clock at the top, money (and what it just changed by),
//! round announcements, plant and defuse progress, and the buy-zone and bomb icons.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{panel, text, CT_COLOR, MUTED, TEXT, T_COLOR};
use bevy::prelude::*;
use net::protocol::{BombNet, GameEvent, Team};
use net::rounds::{EndReason, Phase};

pub struct RoundHudPlugin;

impl Plugin for RoundHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RoundNews>()
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(Update, (top_bar, money, news, progress).in_set(Frame::Ui));
    }
}

/// A round event for the announcements (sent by `effects::events`).
#[derive(Message)]
pub struct RoundNews(pub GameEvent);

const MONEY: Color = Color::srgb(0.45, 0.95, 0.45);
const LOSS: Color = Color::srgb(1.0, 0.45, 0.4);
const BOMB: Color = Color::srgb(1.0, 0.3, 0.25);

#[derive(Component)]
struct ScoreT;
#[derive(Component)]
struct ScoreCt;
#[derive(Component)]
struct Clock;
#[derive(Component)]
struct RoundNo;
#[derive(Component)]
struct Money;
#[derive(Component)]
struct MoneyChange;
#[derive(Component)]
struct Icons;
#[derive(Component)]
struct Headline;
#[derive(Component)]
struct Subline;
#[derive(Component)]
struct Bar;
#[derive(Component)]
struct BarFill;
#[derive(Component)]
struct BarLabel;

fn abs(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

fn spawn(mut commands: Commands) {
    let game = DespawnOnExit(AppState::InGame);
    // Top centre: T score | clock | CT score, round number under it.
    commands
        .spawn((game.clone(), abs(Node { width: percent(100), top: px(8), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })))
        .with_children(|col| {
            col.spawn(panel(Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: px(18), padding: UiRect::axes(px(16), px(4)), ..default() })).with_children(|row| {
                row.spawn((ScoreT, text("0", 26.0, T_COLOR)));
                row.spawn((Clock, text("", 26.0, TEXT), Node { min_width: px(84), justify_content: JustifyContent::Center, ..default() }));
                row.spawn((ScoreCt, text("0", 26.0, CT_COLOR)));
            });
            col.spawn((RoundNo, text("", 13.0, MUTED)));
        });
    // Bottom left, above health: money, what it just changed by, buy zone / bomb icons.
    commands
        .spawn((game.clone(), abs(Node { left: px(24), bottom: px(70), flex_direction: FlexDirection::Row, align_items: AlignItems::Baseline, column_gap: px(14), ..default() })))
        .with_children(|row| {
            row.spawn((Money, text("", 28.0, MONEY)));
            row.spawn((MoneyChange, text("", 18.0, MONEY)));
            row.spawn((Icons, text("", 16.0, MONEY)));
        });
    // Announcements, upper middle.
    commands
        .spawn((game.clone(), abs(Node { width: percent(100), top: percent(22), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })))
        .with_children(|col| {
            col.spawn((Headline, text("", 34.0, TEXT)));
            col.spawn((Subline, text("", 18.0, MUTED)));
        });
    // Plant / defuse progress, low middle.
    commands
        .spawn((Bar, game, Visibility::Hidden, abs(Node { width: percent(100), bottom: percent(24), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: px(4), ..default() })))
        .with_children(|col| {
            col.spawn((BarLabel, text("", 16.0, TEXT)));
            col.spawn((Node { width: px(260), height: px(8), border_radius: BorderRadius::all(px(4)), ..default() }, BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)))).with_children(|b| {
                b.spawn((BarFill, Node { width: percent(0), height: percent(100), border_radius: BorderRadius::all(px(4)), ..default() }, BackgroundColor(TEXT)));
            });
        });
}

/// Change a text only when it really changes.
fn set(t: &mut Mut<Text>, s: String) {
    if t.0 != s {
        t.0 = s;
    }
}

fn mmss(secs: f32) -> String {
    let s = secs.max(0.0).ceil() as u32;
    format!("{}:{:02}", s / 60, s % 60)
}

#[allow(clippy::type_complexity)]
fn top_bar(
    w: Res<Walker>,
    mut st: Single<&mut Text, (With<ScoreT>, Without<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    mut sc: Single<&mut Text, (With<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    mut clock: Single<(&mut Text, &mut TextColor), (With<Clock>, Without<RoundNo>)>,
    mut round: Single<&mut Text, With<RoundNo>>,
) {
    let r = w.client.round;
    let practice = w.client.practice;
    set(&mut st, if practice { String::new() } else { r.score[0].to_string() });
    set(&mut sc, if practice { String::new() } else { r.score[1].to_string() });
    let planted = matches!(w.client.bomb, BombNet::Planted { .. });
    let (t, color) = match r.phase {
        _ if practice => ("PRACTICE".to_string(), MUTED),
        Phase::Warmup => ("WARMUP".to_string(), MUTED),
        Phase::Live if planted => ("BOMB".to_string(), BOMB),
        Phase::Freeze => (mmss(r.left), Color::srgb(1.0, 0.85, 0.3)),
        Phase::Live if r.left <= 10.0 => (mmss(r.left), BOMB),
        Phase::Live => (mmss(r.left), TEXT),
        Phase::Over | Phase::GameOver => (mmss(r.left), MUTED),
    };
    set(&mut clock.0, t);
    if clock.1 .0 != color {
        clock.1 .0 = color;
    }
    let label = match r.phase {
        _ if practice || r.phase == Phase::Warmup => String::new(),
        Phase::Freeze => format!("ROUND {}  -  FREEZE TIME", r.round),
        _ => format!("ROUND {}", r.round),
    };
    set(&mut round, label);
}

/// Money, and for a few seconds what it just changed by.
#[allow(clippy::type_complexity)]
fn money(
    w: Res<Walker>,
    time: Res<Time>,
    mut last: Local<Option<(i32, f32, i32)>>,
    mut m: Single<&mut Text, (With<Money>, Without<MoneyChange>, Without<Icons>)>,
    mut change: Single<(&mut Text, &mut TextColor), (With<MoneyChange>, Without<Icons>)>,
    mut icons: Single<&mut Text, With<Icons>>,
) {
    let alive = w.client.vitals.alive() && w.client.id().is_some();
    let now = time.elapsed_secs();
    let money = w.client.me_info.money;
    if w.client.practice {
        set(&mut m, String::new());
    } else {
        set(&mut m, format!("${money}"));
        match *last {
            Some((before, _, _)) if before != money => *last = Some((money, now, money - before)),
            None => *last = Some((money, -10.0, 0)),
            _ => {}
        }
    }
    let (_, at, d) = last.unwrap_or((0, -10.0, 0));
    if now - at < 3.0 && d != 0 {
        set(&mut change.0, if d > 0 { format!("+{d}") } else { format!("{d}") });
        let c = if d > 0 { MONEY } else { LOSS };
        if change.1 .0 != c {
            change.1 .0 = c;
        }
    } else {
        set(&mut change.0, String::new());
    }
    // "$" in a buy zone during buy time; "C4" when carrying it (bright inside a bomb site).
    let mut s = String::new();
    if alive && w.client.me_info.can_buy && !w.client.practice {
        s += "[ $ BUY ]  ";
    }
    if alive && w.client.state.loadout.c4 {
        let site = w.client.world.zones.in_bomb_site(w.client.me().origin);
        s += if site { "[ C4 - BOMB SITE ]" } else { "[ C4 ]" };
    }
    set(&mut icons, s);
}

/// Round announcements, shown for a few seconds.
#[allow(clippy::type_complexity)]
fn news(
    mut msgs: MessageReader<RoundNews>,
    time: Res<Time>,
    w: Res<Walker>,
    mut shown_at: Local<f32>,
    mut head: Single<(&mut Text, &mut TextColor), (With<Headline>, Without<Subline>)>,
    mut sub: Single<&mut Text, With<Subline>>,
) {
    let now = time.elapsed_secs();
    let team_name = |t: Team| if t == Team::Ct { "Counter-Terrorists" } else { "Terrorists" };
    let team_color = |t: Team| if t == Team::Ct { CT_COLOR } else { T_COLOR };
    for RoundNews(e) in msgs.read() {
        let (h, s, c) = match e {
            GameEvent::RoundEnd { winner, reason } => {
                let why = match reason {
                    EndReason::Eliminated => "Enemy eliminated",
                    EndReason::BombExploded => "Target bombed",
                    EndReason::BombDefused => "The bomb has been defused",
                    EndReason::TimeUp => "Target saved",
                };
                (format!("{} Win", team_name(*winner)), why.to_string(), team_color(*winner))
            }
            GameEvent::MatchOver { winner } => {
                let r = w.client.round.score;
                match winner {
                    Some(t) => (format!("{} win the match", team_name(*t)), format!("{} - {}", r[0], r[1]), team_color(*t)),
                    None => ("Draw".into(), format!("{} - {}", r[0], r[1]), TEXT),
                }
            }
            GameEvent::SidesSwapped => ("Switching sides".into(), String::new(), TEXT),
            GameEvent::RoundStart { round: 1 } => ("Match started".into(), "Good luck, have fun".into(), TEXT),
            GameEvent::BombPlanted { .. } => ("The bomb has been planted".into(), String::new(), BOMB),
            _ => continue,
        };
        set(&mut head.0, h);
        set(&mut sub, s);
        head.1 .0 = c;
        *shown_at = now;
    }
    if now - *shown_at > 4.0 {
        set(&mut head.0, String::new());
        set(&mut sub, String::new());
    }
}

/// Planting (your own) or defusing (anyone you see) progress.
#[allow(clippy::type_complexity)]
fn progress(
    w: Res<Walker>,
    mut bar: Single<&mut Visibility, With<Bar>>,
    mut fill: Single<&mut Node, With<BarFill>>,
    mut label: Single<&mut Text, With<BarLabel>>,
    mut defuse_total: Local<f32>,
) {
    let me = w.client.id();
    let l = &w.client.state.loadout;
    let mut shown: Option<(String, f32)> = None;
    if let Some(start) = l.planting {
        // Own command clock (input number x tick), as the weapon code counts it.
        let elapsed = w.client.cmd_time() - start;
        shown = Some(("Planting the bomb".into(), (elapsed / sim::weapons::C4_PLANT_TIME) as f32));
    }
    if let BombNet::Planted { defuse: Some((id, left)), .. } = w.client.bomb {
        if *defuse_total < left {
            *defuse_total = left; // the first we hear of it is close to the full time
        }
        let who = if Some(id) == me { "Defusing the bomb".to_string() } else { "Bomb being defused".to_string() };
        shown = Some((who, 1.0 - left / defuse_total.max(0.01)));
    } else {
        *defuse_total = 0.0;
    }
    let vis = if shown.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if **bar != vis {
        **bar = vis;
    }
    if let Some((s, f)) = shown {
        set(&mut label, s);
        fill.width = percent(f.clamp(0.0, 1.0) * 100.0);
    }
}
