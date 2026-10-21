//! The round on screen (Interference, design/mockups/ui "In game"): score, clock and alive
//! ticks top centre; `money`: money, what it just changed by and the status icons (buy zone,
//! kit, bomb) top left; `news`: round announcements and plant / defuse progress.
//! In the last 10 s the clock turns red and every second swells and ticks.
mod money;
mod news;

use crate::game::{Frame, Walker};
use crate::hud::SAFE;
use crate::menu::AppState;
use crate::ui::{edge, hud_panel, kit_text, vh, Anim, Edge, EdgeColor, HudLayer, Kit, BAD, CT_COLOR, EDGE_REST, LINE_2, TX1, TX2, T_COLOR};
use bevy::prelude::*;
use net::protocol::{BombNet, GameEvent, Team};
use net::rounds::Phase;

pub struct RoundHudPlugin;

impl Plugin for RoundHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<RoundNews>().add_systems(OnEnter(AppState::InGame), (spawn, money::spawn, news::spawn)).add_systems(Update, (top_bar, ticks, money::money, news::news, news::progress).in_set(Frame::Ui));
    }
}

/// A round event for the announcements (sent by `effects::events`).
#[derive(Message)]
pub struct RoundNews(pub GameEvent);

#[derive(Component)]
struct ScoreT;
#[derive(Component)]
struct ScoreCt;
#[derive(Component)]
struct Clock;
#[derive(Component)]
struct ClockEdge;
#[derive(Component)]
struct RoundNo;
/// One "player alive" tick under the score: team and index.
#[derive(Component)]
struct Tick(Team, usize);

fn abs(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

fn shadow() -> TextShadow {
    TextShadow { offset: Vec2::splat(1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) }
}

/// Player-facing side names (the code still says Ct/T): Bureau defends, Strays attack.
fn side_name(t: Team) -> &'static str {
    if t == Team::Ct { "Bureau" } else { "Strays" }
}

fn side_color(t: Team) -> Color {
    match t {
        Team::Ct => CT_COLOR,
        Team::T => T_COLOR,
        Team::Spectator => TX1,
    }
}

fn spawn(mut commands: Commands, kit: Res<Kit>) {
    // Top centre: CT score | clock and round | T score, alive ticks under each team.
    commands
        .spawn((DespawnOnExit(AppState::InGame), HudLayer, abs(Node { width: percent(100), top: vh(SAFE), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(12.0), ..default() })))
        .with_children(|col| {
            col.spawn(Node { column_gap: vh(4.0), ..default() }).with_children(|row| {
                let score = |row: &mut ChildSpawnerCommands, color: Color, mark: bool| {
                    row.spawn(hud_panel(Node { width: vh(88.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() })).with_children(|p| {
                        p.spawn(edge(EdgeColor::Fixed(color), 1.0));
                        let mut t = p.spawn(kit_text("0", 32.0, color, &kit.num));
                        if mark {
                            t.insert(ScoreCt);
                        } else {
                            t.insert(ScoreT);
                        }
                    });
                };
                score(row, CT_COLOR, true);
                row.spawn(hud_panel(Node { width: vh(148.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, padding: UiRect::vertical(vh(10.0)), row_gap: vh(6.0), ..default() })).with_children(|c| {
                    c.spawn((ClockEdge, edge(EdgeColor::Team, EDGE_REST)));
                    c.spawn((Clock, kit_text("", 32.0, TX1, &kit.num)));
                    c.spawn((RoundNo, kit_text("", 13.0, TX2, &kit.text)));
                });
                score(row, T_COLOR, false);
            });
            // each team's ticks centred under its own score (same widths as the boxes above)
            col.spawn(Node { column_gap: vh(4.0), ..default() }).with_children(|row| {
                for (k, team) in [Team::Ct, Team::T].into_iter().enumerate() {
                    if k == 1 {
                        row.spawn(Node { width: vh(148.0), ..default() });
                    }
                    row.spawn(Node { width: vh(88.0), justify_content: JustifyContent::Center, column_gap: vh(4.0), ..default() }).with_children(|t| {
                        for i in 0..5 {
                            t.spawn((Tick(team, i), Node { width: vh(14.0), height: vh(4.0), border_radius: BorderRadius::all(px(2)), display: Display::None, ..default() }, BackgroundColor(Color::NONE)));
                        }
                    });
                }
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

/// A deathmatch game: the server counts its 10 minutes down in the warmup phase (a real
/// warmup has no time).
pub fn deathmatch(w: &Walker) -> bool {
    w.client.round.phase == Phase::Warmup && w.client.round.left > 0.0
}

/// Buy time left under this many seconds shows amber.
pub const BUY_HURRY: f32 = 5.0;

/// The buy time left, as the top bar and the shop both show it ("0:14"), and whether it's
/// nearly out. None where buying has no limit (practice, deathmatch, warmup) or it has ended.
pub fn buy_time(w: &Walker) -> Option<(String, bool)> {
    let left = w.client.me_info.buy_left.filter(|l| *l > 0.0)?;
    Some((mmss(left), left < BUY_HURRY))
}

/// The last 30 seconds warn (CS 1.6, CHudRoundTimer::Draw): the clock flips to red and back,
/// faster as time runs out, red for good at 0. Silent, as in CS. The last ten also swell.
const WARN: f32 = 30.0;
const SWELL: f32 = 10.0;
/// The last seconds of freeze time swell like a round's last ten.
const FREEZE_SWELL: f32 = 5.0;

/// How long each colour lasts at this many seconds left (CS 1.6's steps).
fn flip_period(left: f32) -> f32 {
    match left {
        l if l <= 2.0 => 0.05,
        l if l <= 5.0 => 0.1,
        l if l <= 10.0 => 0.2,
        l if l <= 20.0 => 0.4,
        _ => 0.8,
    }
}

/// Whether the warning clock shows red now.
fn warn_red(left: f32) -> bool {
    left <= 0.0 || (left / flip_period(left)).floor() as u32 % 2 == 0
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn top_bar(
    w: Res<Walker>,
    mut st: Single<&mut Text, (With<ScoreT>, Without<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    mut sc: Single<&mut Text, (With<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    clock: Single<(Entity, &mut Text, &mut TextColor), (With<Clock>, Without<RoundNo>)>,
    mut round: Single<&mut Text, With<RoundNo>>,
    mut edge: Single<&mut Edge, With<ClockEdge>>,
    mut last_second: Local<u32>,
    mut last_phase: Local<Option<Phase>>,
    mut sounds: MessageWriter<crate::ui::UiSound>,
    mut commands: Commands,
) {
    let r = w.client.round;
    let practice = w.client.practice;
    // Rounds won: [terrorists, counter-terrorists].
    set(&mut st, if practice { "-".into() } else { r.score[0].to_string() });
    set(&mut sc, if practice { "-".into() } else { r.score[1].to_string() });
    let planted = matches!(w.client.bomb, BombNet::Planted { .. });
    let urgent = !practice && r.phase == Phase::Live && !planted && r.left <= WARN;
    let dm = deathmatch(&w);
    let (t, color) = match r.phase {
        // deathmatch runs on practice rules, so it's asked first: its game clock
        _ if dm => (mmss(r.left), TX1),
        // practice: the bomb's own countdown once it's planted
        _ if practice => match w.client.bomb {
            BombNet::Planted { left, .. } => (mmss(left), BAD),
            _ => ("--:--".to_string(), TX2),
        },
        Phase::Warmup => ("--:--".to_string(), TX2),
        Phase::Live if planted => ("BOMB".to_string(), BAD),
        Phase::Live if urgent => (mmss(r.left), if warn_red(r.left) { BAD } else { TX1 }),
        Phase::Freeze | Phase::Live => (mmss(r.left), TX1),
        Phase::Over | Phase::GameOver => (mmss(r.left), TX2),
    };
    let (clock_e, mut text, mut tc) = clock.into_inner();
    set(&mut text, t);
    if tc.0 != color {
        tc.0 = color;
    }
    edge.alert = urgent || (r.phase == Phase::Live && planted);
    edge.rest = if edge.alert { 1.0 } else { crate::ui::EDGE_REST };
    // each second in the last ten of a round, and the last five of freeze time: the clock
    // swells and splits (no sound, as in CS)
    let second = r.left.max(0.0).ceil() as u32;
    let freeze_ending = !practice && !dm && r.phase == Phase::Freeze && r.left <= FREEZE_SWELL;
    if ((urgent && r.left <= SWELL) || freeze_ending) && second != *last_second && second > 0 {
        commands.entity(clock_e).insert(Anim::tick());
    }
    *last_second = second;
    // freeze time over: go
    if *last_phase == Some(Phase::Freeze) && r.phase == Phase::Live && !practice && !dm {
        sounds.write(crate::ui::UiSound::Go);
    }
    *last_phase = Some(r.phase);
    let label = match r.phase {
        _ if dm => "DEATHMATCH".into(),
        _ if practice => "PRACTICE".into(),
        Phase::Warmup => "WARMUP".into(),
        // while buying is open (freeze time and the start of the round): how long is left
        Phase::Freeze | Phase::Live => match buy_time(&w) {
            Some((t, _)) => format!("ROUND {} · BUY {t}", r.round),
            None => format!("ROUND {}", r.round),
        },
        Phase::Over => "ROUND OVER".into(),
        Phase::GameOver => "MATCH OVER".into(),
    };
    set(&mut round, label);
}

/// One tick per player under each score, lit while they're alive.
fn ticks(w: Res<Walker>, mut ticks: Query<(&Tick, &mut BackgroundColor, &mut Node)>) {
    for (Tick(team, i), mut bg, mut node) in &mut ticks {
        let player = w.client.scores.iter().filter(|s| s.team == *team).nth(*i);
        // no player: no tick, so the ones there stay centred
        let display = if player.is_some() { Display::Flex } else { Display::None };
        if node.display != display {
            node.display = display;
        }
        bg.set_if_neq(BackgroundColor(if player.is_some_and(|p| p.alive) { side_color(*team) } else { LINE_2 }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warning_flips_faster_as_time_runs_out() {
        assert_eq!(flip_period(25.0), 0.8);
        assert_eq!(flip_period(15.0), 0.4);
        assert_eq!(flip_period(8.0), 0.2);
        assert_eq!(flip_period(4.0), 0.1);
        assert_eq!(flip_period(1.0), 0.05);
        assert!(warn_red(0.0));
        // red and white take turns
        assert_ne!(warn_red(25.0), warn_red(25.8));
    }
}
