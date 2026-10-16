//! CS2's "Show Team ID Through Walls": a team-coloured pip, and optionally the name, over
//! each teammate's head, seen through walls, so you know where your team is without a radar.
//! Settings: off / pips / names (`Settings::team_ids`).
//!
//! Safe against "make everyone a teammate" memory hacks: the server decides who your team is
//! and never sends enemies you can't see (server/visibility.rs), so there's nothing hidden to mark.
use crate::game::{FlyCam, Frame, Walker};
use crate::map;
use crate::menu::{AppState, Settings};
use crate::ui::{text, CT_COLOR, T_COLOR};
use bevy::prelude::*;
use net::protocol::Team;

pub struct TeammatesPlugin;

impl Plugin for TeammatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, team_ids.in_set(Frame::Ui));
    }
}

/// One teammate's overhead marker: a pip with their name under it.
#[derive(Component)]
struct TeamId(u8);

#[derive(Component)]
struct TeamName;

#[allow(clippy::type_complexity)]
fn team_ids(
    mut commands: Commands,
    w: Res<Walker>,
    settings: Res<Settings>,
    cam: Single<(&Camera, &GlobalTransform), With<FlyCam>>,
    mut ids: Query<(Entity, &TeamId, &mut Node, &mut Visibility, &Children)>,
    mut names: Query<&mut Visibility, (With<TeamName>, Without<TeamId>)>,
) {
    let mine = w.client.team();
    let on = settings.team_ids > 0 && mine != Team::Spectator;
    let mates: Vec<_> = if on { w.client.others().into_iter().filter(|o| o.alive && o.team == mine).collect() } else { Vec::new() };
    for (e, id, mut node, mut v, children) in &mut ids {
        let Some(o) = mates.iter().find(|o| o.id == id.0) else {
            commands.entity(e).despawn();
            continue;
        };
        let head = Vec3::from(map::to_y_up([o.origin[0], o.origin[1], o.origin[2] + 44.0]));
        match cam.0.world_to_viewport(cam.1, head) {
            Ok(p) => {
                node.left = px(p.x - 50.0);
                node.top = px(p.y - 24.0);
                v.set_if_neq(Visibility::Inherited);
            }
            Err(_) => {
                v.set_if_neq(Visibility::Hidden); // behind you
            }
        }
        for c in children {
            if let Ok(mut nv) = names.get_mut(*c) {
                nv.set_if_neq(if settings.team_ids >= 2 { Visibility::Inherited } else { Visibility::Hidden });
            }
        }
    }
    let color = if mine == Team::Ct { CT_COLOR } else { T_COLOR };
    for o in &mates {
        if ids.iter().any(|(_, t, ..)| t.0 == o.id) {
            continue;
        }
        let name = w.client.scores.iter().find(|s| s.id == o.id).map(|s| s.name.clone()).unwrap_or_default();
        commands
            .spawn((
                TeamId(o.id),
                Node { position_type: PositionType::Absolute, width: px(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
                Visibility::Hidden,
                DespawnOnExit(AppState::InGame),
            ))
            .with_children(|c| {
                // Dark shadow (from `text`) and outline so they read on any wall.
                c.spawn((TeamName, text(name, 15.0, color)));
                // The pip: a small diamond pointing down at them.
                c.spawn((
                    Node { width: px(10), height: px(10), margin: UiRect::top(px(3)), border: UiRect::all(px(2)), ..default() },
                    BackgroundColor(color),
                    BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                    UiTransform::from_rotation(Rot2::degrees(45.0)),
                ));
            });
    }
}
