//! A reserved match on screen: "waiting for players", the knife round, the winners' stay or
//! switch vote (F1 / F2, CS's vote keys), the match paused when a side is empty, the
//! surrender vote once a teammate abandoned (F3 calls it, F1 / F2 answer), and back to the
//! menu when the match is over.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{hud_panel, kit_text, vh, Kit, AMBER, TX1};
use bevy::prelude::*;
use net::protocol::{Stage, Team};

pub struct MatchFlowPlugin;

impl Plugin for MatchFlowPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn).add_systems(Update, (banner, vote, ended).in_set(Frame::Ui));
    }
}

#[derive(Component)]
struct Banner;

fn spawn(mut commands: Commands, kit: Res<Kit>) {
    commands
        .spawn((DespawnOnExit(AppState::InGame), Node { position_type: PositionType::Absolute, width: percent(100), top: vh(190.0), justify_content: JustifyContent::Center, ..default() }))
        .with_children(|row| {
            row.spawn((Banner, Visibility::Hidden, hud_panel(Node { padding: UiRect::axes(vh(24.0), vh(12.0)), border: UiRect::top(vh(crate::ui::BAR)), ..default() }), BorderColor::all(AMBER)))
                .with_children(|p| {
                    p.spawn(kit_text("", 20.0, TX1, &kit.text));
                });
        });
}

fn side(t: Team) -> &'static str {
    if t == Team::Ct { "Bureau" } else { "Strays" }
}

fn banner(w: Res<Walker>, binds: Res<crate::binds::Binds>, mut banner: Query<(&mut Visibility, &Children), With<Banner>>, mut texts: Query<&mut Text>) {
    use crate::binds::Action;
    // the keys as bound, in the player's layout
    let key = |a: Action| binds.get(a).label(binds.layout);
    let Ok((mut vis, kids)) = banner.single_mut() else { return };
    let round = &w.client.round;
    let me = w.client.team();
    let mine = (me == Team::Ct) as usize;
    let msg = match round.stage {
        Stage::Match if round.paused => Some("Match paused: a team has nobody connected. It resumes when someone returns.".to_string()),
        Stage::Match => match round.vote {
            Some(v) if v.team == me && v.kick == w.client.id() => Some(format!("Your team is voting to kick you ({:.0}s)", v.left)),
            // your team's other votes have their own panel (`vote`)
            Some(v) if v.team == me => None,
            None if me != Team::Spectator && round.abandoned[mine] => Some(format!("A teammate abandoned the match · {} calls a surrender vote", key(Action::Surrender))),
            _ => None,
        },
        Stage::Waiting => Some("Waiting for everyone to connect...".to_string()),
        Stage::Knife => Some("KNIFE ROUND · the winners pick their side".to_string()),
        Stage::SideVote { winners, left } if w.client.team() == winners => Some(format!("You won the knife round: {} stay · {} switch sides ({left:.0}s)", key(Action::VoteYes), key(Action::VoteNo))),
        Stage::SideVote { winners, left } => Some(format!("{} are picking sides ({left:.0}s)", side(winners))),
    };
    *vis = if msg.is_some() { Visibility::Inherited } else { Visibility::Hidden };
    if let Some(mut t) = kids.first().and_then(|k| texts.get_mut(*k).ok()) {
        let s = msg.unwrap_or_default();
        if t.0 != s {
            t.0 = s;
        }
    }
}

fn vote(mut w: ResMut<Walker>, keys: Res<ButtonInput<KeyCode>>, mouse: Res<ButtonInput<MouseButton>>, binds: Res<crate::binds::Binds>) {
    use crate::binds::Action;
    let (yes, no) = (binds.just(Action::VoteYes, &keys, &mouse), binds.just(Action::VoteNo, &keys, &mouse));
    if matches!(w.client.round.stage, Stage::SideVote { .. }) {
        if yes {
            w.client.side_vote(false);
        } else if no {
            w.client.side_vote(true);
        }
    } else if w.client.round.vote.is_some_and(|v| v.team == w.client.team()) {
        if yes || no {
            w.client.surrender_vote(yes);
        }
    } else if binds.just(Action::Surrender, &keys, &mouse) {
        w.client.call_surrender();
    }
}

fn ended(mut commands: Commands, w: Res<Walker>, mut next: ResMut<NextState<AppState>>) {
    if w.client.match_ended {
        commands.insert_resource(crate::notice::Notice::info("Match over", "Good games!"));
        next.set(AppState::Menu);
    }
}
