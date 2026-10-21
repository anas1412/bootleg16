//! Round announcements in the upper middle (design/mockups/ui `announce`): the headline, a
//! short rule in the side's colour, and a readable line under it. Each one tunes in and,
//! after a few seconds, tunes out. Low middle: plant and defuse progress.
use super::{abs, set, shadow, side_color, side_name, RoundNews};
use crate::game::Walker;
use crate::menu::AppState;
use crate::ui::{kit_text, meter, vh, Anim, Kit, Meter, BAD, TX1};
use bevy::prelude::*;
use net::protocol::{BombNet, GameEvent, Team};
use net::rounds::EndReason;

/// How long an announcement stays.
const HOLD: f32 = 4.0;

#[derive(Component)]
pub(super) struct NewsRoot;
/// The announcement on screen, and when it came.
#[derive(Component)]
pub(super) struct Shown(f32);
#[derive(Component)]
pub(super) struct Bar;
#[derive(Component)]
pub(super) struct BarMeter;
#[derive(Component)]
pub(super) struct BarLabel;

pub(super) fn spawn(mut commands: Commands, kit: Res<Kit>) {
    commands.spawn((NewsRoot, crate::hud::Visor, DespawnOnExit(AppState::InGame), abs(Node { width: percent(100), top: percent(26), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })));
    // Plant / defuse progress, low middle.
    commands
        .spawn((Bar, crate::hud::Visor, DespawnOnExit(AppState::InGame), Visibility::Hidden, abs(Node { width: percent(100), bottom: percent(24), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() })))
        .with_children(|col| {
            col.spawn((BarLabel, kit_text("", 16.0, TX1, &kit.text), shadow()));
            let m = meter(col, 320.0, TX1, 0.0);
            col.commands().entity(m).insert(BarMeter);
        });
}

/// What to announce for an event: headline, the line under it, the colour.
fn words(e: &GameEvent, w: &Walker) -> Option<(String, String, Color)> {
    let name = |id: u8| w.client.scores.iter().find(|s| s.id == id).map(|s| (s.name.clone(), s.team));
    Some(match e {
        GameEvent::RoundEnd { winner, reason } => {
            let why = match reason {
                EndReason::Eliminated => "Enemy eliminated",
                EndReason::BombExploded => "Target bombed",
                EndReason::BombDefused => "The bomb has been defused",
                EndReason::TimeUp => "Target saved",
            };
            (format!("{} win", side_name(*winner)), why.into(), side_color(*winner))
        }
        GameEvent::MatchOver { winner } => {
            let r = w.client.round.score;
            match winner {
                Some(t) => (format!("{} win the match", side_name(*t)), format!("{} to {}", r[0].max(r[1]), r[0].min(r[1])), side_color(*t)),
                None => ("Draw".into(), format!("{} to {}", r[0], r[1]), TX1),
            }
        }
        GameEvent::SidesSwapped => ("Switching sides".into(), "Halftime".into(), TX1),
        GameEvent::RoundStart { round: 1 } => ("Match started".into(), "Good luck, have fun".into(), TX1),
        GameEvent::BombPlanted { .. } => ("The bomb has been planted".into(), "Forty seconds".into(), BAD),
        GameEvent::Surrendered { team } => {
            let other = if *team == Team::Ct { Team::T } else { Team::Ct };
            (format!("{} surrendered", side_name(*team)), format!("{} win the match", side_name(other)), side_color(*team))
        }
        GameEvent::KillLimit { player } => {
            let (who, team) = name(*player)?;
            // deathmatch ends (time or kill limit): the top scorer wins, then a new game
            (format!("{who} wins the deathmatch"), "Most points · scores start over".into(), side_color(team))
        }
        GameEvent::VotedOut { player } => {
            let (who, team) = name(*player)?;
            let left = w.client.scores.iter().filter(|s| s.team == team && s.id != *player).count();
            (format!("{who} was voted out"), format!("{} plays on with {left}", side_name(team)), side_color(team))
        }
        _ => return None,
    })
}

/// A new announcement replaces the one up; one held long enough tunes out.
pub(super) fn news(mut commands: Commands, mut msgs: MessageReader<RoundNews>, time: Res<Time>, w: Res<Walker>, kit: Res<Kit>, root: Single<Entity, With<NewsRoot>>, shown: Query<(Entity, &Shown)>) {
    let now = time.elapsed_secs();
    let mut fresh = None;
    for RoundNews(e) in msgs.read() {
        if let Some(words) = words(e, &w) {
            fresh = Some(words);
        }
    }
    let leave = |commands: &mut Commands, e: Entity| {
        commands.entity(e).remove::<Shown>().insert(Anim::tune_out());
    };
    if let Some((head, sub, color)) = fresh {
        for (e, _) in &shown {
            leave(&mut commands, e);
        }
        commands.entity(*root).with_children(|col| {
            col.spawn((Shown(now), Anim::tune_in(0.0), Node { position_type: PositionType::Absolute, flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(10.0), ..default() })).with_children(|a| {
                a.spawn((kit_text(head.to_uppercase(), 52.0, TX1, &kit.display), shadow()));
                a.spawn((Node { width: vh(72.0), height: vh(3.0), border_radius: BorderRadius::all(px(2)), ..default() }, BackgroundColor(color), BoxShadow(vec![ShadowStyle { color: color.with_alpha(0.6), x_offset: px(0), y_offset: px(0), spread_radius: px(0), blur_radius: vh(10.0) }])));
                if !sub.is_empty() {
                    a.spawn((kit_text(sub, 20.0, TX1, &kit.text), shadow()));
                }
            });
        });
        return;
    }
    for (e, s) in &shown {
        if now - s.0 > HOLD {
            leave(&mut commands, e);
        }
    }
}

/// Planting (your own) or defusing (anyone you see) progress.
#[allow(clippy::type_complexity)]
pub(super) fn progress(w: Res<Walker>, mut bar: Single<&mut Visibility, With<Bar>>, mut m: Single<&mut Meter, With<BarMeter>>, mut label: Single<&mut Text, With<BarLabel>>, mut defuse_total: Local<f32>) {
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
    bar.set_if_neq(if shown.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    if let Some((s, f)) = shown {
        set(&mut label, s);
        m.value = f.clamp(0.0, 1.0);
    }
}
