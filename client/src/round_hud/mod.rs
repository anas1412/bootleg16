//! The round on screen (Interference, design/mockups/ui "In game"): score, clock and alive
//! ticks top centre; `money`: money, what it just changed by and the status icons (buy zone,
//! kit, bomb) top left; `news`: round announcements and plant / defuse progress.
//! In the last 10 s the clock turns red and every second swells and ticks.
mod money;
mod news;

use crate::game::{Frame, Walker};
use crate::hud::SAFE;
use crate::menu::AppState;
use crate::ui::{edge, hud_panel, kit_text, vh, Anim, Edge, EdgeColor, HudLayer, Kit, UiSound, BAD, CT_COLOR, EDGE_REST, LINE_2, TX1, TX2, T_COLOR};
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
            col.spawn(Node { width: vh(332.0), justify_content: JustifyContent::SpaceBetween, ..default() }).with_children(|row| {
                for team in [Team::Ct, Team::T] {
                    row.spawn(Node { column_gap: vh(4.0), ..default() }).with_children(|t| {
                        for i in 0..5 {
                            t.spawn((Tick(team, i), Node { width: vh(28.0), height: vh(4.0), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(Color::NONE)));
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

/// Buy time left under this many seconds shows amber.
pub const BUY_HURRY: f32 = 5.0;

/// The buy time left, as the top bar and the shop both show it ("0:14"), and whether it's
/// nearly out. None where buying has no limit (practice, deathmatch, warmup) or it has ended.
pub fn buy_time(w: &Walker) -> Option<(String, bool)> {
    let left = w.client.me_info.buy_left.filter(|l| *l > 0.0)?;
    Some((mmss(left), left < BUY_HURRY))
}

/// The last seconds swell and tick; the last five tick out loud.
const URGENT: f32 = 10.0;
const URGENT_SOUND: f32 = 5.0;

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn top_bar(
    w: Res<Walker>,
    mut st: Single<&mut Text, (With<ScoreT>, Without<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    mut sc: Single<&mut Text, (With<ScoreCt>, Without<Clock>, Without<RoundNo>)>,
    clock: Single<(Entity, &mut Text, &mut TextColor), (With<Clock>, Without<RoundNo>)>,
    mut round: Single<&mut Text, With<RoundNo>>,
    mut edge: Single<&mut Edge, With<ClockEdge>>,
    mut last_second: Local<u32>,
    mut sounds: MessageWriter<UiSound>,
    mut commands: Commands,
) {
    let r = w.client.round;
    let practice = w.client.practice;
    // Rounds won: [terrorists, counter-terrorists].
    set(&mut st, if practice { "-".into() } else { r.score[0].to_string() });
    set(&mut sc, if practice { "-".into() } else { r.score[1].to_string() });
    let planted = matches!(w.client.bomb, BombNet::Planted { .. });
    let urgent = !practice && r.phase == Phase::Live && !planted && r.left <= URGENT;
    let (t, color) = match r.phase {
        _ if practice => ("--:--".to_string(), TX2),
        Phase::Warmup => ("--:--".to_string(), TX2),
        Phase::Live if planted => ("BOMB".to_string(), BAD),
        Phase::Live if urgent => (mmss(r.left), BAD),
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
    // each second in the last ten: the clock swells and splits, and the last five tick
    let second = r.left.max(0.0).ceil() as u32;
    if urgent && second != *last_second && second > 0 {
        commands.entity(clock_e).insert(Anim::tick());
        if r.left <= URGENT_SOUND {
            sounds.write(UiSound::Tick);
        }
    }
    *last_second = second;
    let label = match r.phase {
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
fn ticks(w: Res<Walker>, mut ticks: Query<(&Tick, &mut BackgroundColor)>) {
    for (Tick(team, i), mut bg) in &mut ticks {
        let c = match w.client.scores.iter().filter(|s| s.team == *team).nth(*i) {
            Some(p) if p.alive => side_color(*team),
            Some(_) => LINE_2,
            None => Color::NONE,
        };
        bg.set_if_neq(BackgroundColor(c));
    }
}
