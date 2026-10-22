//! Dead bodies as the server has them (`sim::bodies`): each plays its death animation where
//! it fell and then lies there, the whole match. One carried as a shield hangs limp in front
//! of its carrier (yours in front of you), arms down, leaning back against them, and falls
//! again when let go.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::players::OtherPlayers;
use crate::{map, models};
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use bevy::prelude::*;
use std::sync::Arc;

pub struct BodiesPlugin;

impl Plugin for BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw.in_set(Frame::World).run_if(resource_exists::<OtherPlayers>).run_if(resource_exists::<Walker>));
    }
}

#[derive(Component)]
struct Drawn {
    id: u16,
    character: Arc<crate::characters::look::Look>,
    body: Vec<Entity>,
    seq: usize,
    /// When (game clock) its fall started: death, or being let go.
    fell: f32,
    carried: bool,
}

/// A carried body's pose: the first frame of "death3" (standing, arms hanging at its sides),
/// leaning back against its carrier this much.
const CARRIED_SEQ: &str = "death3";
const CARRIED_LEAN: f32 = 0.25;

#[allow(clippy::too_many_arguments)]
fn draw(
    mut commands: Commands,
    w: Res<Walker>,
    mut look: ResMut<OtherPlayers>,
    mut cache: ResMut<models::ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::studio::StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    characters: Res<crate::characters::Characters>,
    mut drawn: Query<(Entity, &mut Drawn, &mut Transform), Without<ChildOf>>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
) {
    let c = &w.client;
    let (now, dt) = (time.elapsed_secs(), time.delta_secs());
    for (e, d, _) in &drawn {
        if !c.bodies.iter().any(|b| b.id == d.id) {
            commands.entity(e).despawn();
        }
    }
    let hz = c.rate().hz() as f32;
    let mut people = c.others_at(fixed.overstep_fraction());
    people.extend(c.me_as_remote());
    let forced = std::env::var("BOOTLEG16_CHARACTER").ok();
    for b in &c.bodies {
        // where it is: in front of its carrier (as drawn), else where the server has it
        let carrier = b.carrier.and_then(|id| people.iter().find(|p| p.id == id));
        let (at, yaw) = match carrier {
            Some(p) => (sim::bodies::carried_at(&c.world, p.origin, p.yaw), p.yaw),
            None => (b.origin, b.yaw),
        };
        let target = Vec3::from(map::to_y_up(at));
        let rot = Quat::from_rotation_y(yaw.to_radians()) * if carrier.is_some() { Quat::from_rotation_z(CARRIED_LEAN) } else { Quat::IDENTITY };
        if let Some((_, mut d, mut tf)) = drawn.iter_mut().find(|(_, d, _)| d.id == b.id) {
            // let go: it falls again
            if d.carried && carrier.is_none() {
                d.fell = now;
            }
            d.carried = carrier.is_some();
            // carried: stuck to its carrier; else eased to the server's (it moves at snapshot rate)
            let k = if d.carried || tf.translation.distance(target) > 64.0 { 1.0 } else { (dt * 20.0).min(1.0) };
            tf.translation = tf.translation.lerp(target, k);
            tf.rotation = rot;
            let model = &look.cs.model;
            let pose = match model.sequence(CARRIED_SEQ).filter(|_| d.carried) {
                Some(held) => model.pose_at(held, 0.0, 0.5),
                None => {
                    let sq = &model.sequences[d.seq];
                    model.pose_at(d.seq, ((now - d.fell) * sq.fps).min(sq.frames as f32 - 1.0), 0.5)
                }
            };
            d.character.set_pose(&d.body, &pose, &mut bones);
            continue;
        }
        // new to us: its owner's character
        let pick = forced.as_deref().unwrap_or_else(|| c.scores.iter().find(|s| s.id == b.owner).map(|s| s.character.as_str()).unwrap_or(""));
        let team = c.scores.iter().find(|s| s.id == b.owner).map(|s| s.team).unwrap_or(net::protocol::Team::T);
        let Some(li) = characters.for_player(pick, b.owner, team) else { continue };
        let Some(character) = look.look(li, &characters, &mut cache, &mut meshes, &mut bindposes, &mut images, &mut materials) else { continue };
        let name = sim::bodies::DEATHS.get(b.death as usize).copied().unwrap_or("death2");
        let seq = look.cs.model.sequence(name).unwrap_or(0);
        let parent = commands.spawn((Transform::from_translation(target).with_rotation(rot), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        let body = character.spawn(&mut commands, parent, &mut materials);
        // seen dying now, or long dead (already lying)
        let age = c.server_tick().saturating_sub(b.born) as f32 / hz;
        commands.entity(parent).insert(Drawn { id: b.id, character, body, seq, fell: now - age, carried: false });
    }
}
