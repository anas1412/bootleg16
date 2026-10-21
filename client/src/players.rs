//! Other players: their CS model for their team, animated like GoldSrc does it (legs
//! from a gait sequence, upper body aiming up/down with the weapon's pose), shooting and
//! flinching when hit, and a corpse playing a death animation when they die.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::{map, models};
use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use net::protocol::Team;
use sim::hitbox::HitGroup;
use sim::weapons::Id;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct PlayersPlugin;

impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerAnim>()
            .add_systems(Startup, load_looks)
            .add_systems(OnEnter(AppState::InGame), |mut l: ResMut<OtherPlayers>| {
                l.dying.clear();
                l.last_hit.clear();
            })
            .add_systems(Update, (clear_corpses, anim_events, draw_others, animate).chain().in_set(Frame::World));
    }
}

/// A new round: the bodies are gone (CS clears them at round start).
fn clear_corpses(mut commands: Commands, mut news: MessageReader<crate::effects::GameNews>, corpses: Query<Entity, With<Corpse>>) {
    if news.read().any(|n| matches!(n.0, net::protocol::GameEvent::RoundStart { .. })) {
        for e in &corpses {
            commands.entity(e).despawn();
        }
    }
}

/// Something that changes how a player is animated (sent by `effects::events`).
#[derive(Message)]
pub enum PlayerAnim {
    /// They fired (silenced: no muzzle flash).
    Shot(u8, bool),
    Hurt(u8, HitGroup),
    Died(u8),
}

/// Corpses kept at once (CS clears them each round).
const MAX_CORPSES: usize = 16;

#[derive(Resource)]
struct OtherPlayers {
    /// T looks, then CT looks.
    looks: Vec<Arc<models::Loaded>>,
    t_count: usize,
    /// Died, but the snapshot we draw from may still show them alive: don't redraw them yet.
    dying: HashSet<u8>,
    /// Where each player was last hit (picks the death animation).
    last_hit: HashMap<u8, HitGroup>,
}

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<crate::studio::StudioMaterial>>) {
    // The CS 1.6 classes (characters.rs), in `Score::looks` order.
    let mut load = |classes: &[(&str, &str)]| -> Vec<_> { classes.iter().filter_map(|(n, _)| cache.get(&format!("player/{n}/{n}.mdl"), &mut images, &mut materials)).collect() };
    let mut looks = load(&crate::characters::T_CLASSES);
    let t_count = looks.len();
    looks.extend(load(&crate::characters::CT_CLASSES));
    commands.insert_resource(OtherPlayers { looks, t_count, dying: HashSet::new(), last_hit: HashMap::new() });
}

impl OtherPlayers {
    /// Which look a player gets: the character they picked for their side (`Score::looks`).
    fn look_for(&self, looks: [u8; 2], team: Team) -> Option<usize> {
        let (start, n, pick) = match team {
            Team::Ct => (self.t_count, self.looks.len() - self.t_count, looks[1]),
            _ => (0, self.t_count, looks[0]),
        };
        (n > 0).then(|| start + pick as usize % n)
    }
}

/// A live player's model. Children are the body parts, then the weapon parts.
#[derive(Component)]
struct OtherPlayer {
    id: u8,
    look: usize,
    weapon: Id,
    body: Vec<Entity>,
    /// The p_ model in their hand and its bones (it borrows the body's pose by bone name).
    gun: Option<(Arc<models::Loaded>, models::SpawnedSkin)>,
    /// Legs, upper body and spine, as CS animates them (shared with the server's hitboxes).
    anim: sim::anim::Anim,
    crouched: bool,
    /// Muzzle flash quad (a child) and until when it shows.
    flash: Entity,
    flash_until: f32,
}

/// A dead player playing their death animation (then lying still).
#[derive(Component)]
struct Corpse {
    look: usize,
    body: Vec<Entity>,
    seq: usize,
    started: f32,
    done: bool,
}

fn seq(model: &goldsrc::mdl::Model, name: &str) -> Option<usize> {
    model.sequence(name)
}

fn anim_events(
    mut commands: Commands,
    mut msgs: MessageReader<PlayerAnim>,
    mut look: ResMut<OtherPlayers>,
    time: Res<Time>,
    mut live: Query<(Entity, &mut OtherPlayer)>,
    corpses: Query<(Entity, &Corpse)>,
) {
    let now = time.elapsed_secs();
    for m in msgs.read() {
        match *m {
            PlayerAnim::Shot(id, _) | PlayerAnim::Hurt(id, _) => {
                if let PlayerAnim::Hurt(_, g) = *m {
                    look.last_hit.insert(id, g);
                }
                let Some((_, mut p)) = live.iter_mut().find(|(_, p)| p.id == id) else { continue };
                if let PlayerAnim::Shot(_, silenced) = *m {
                    if crate::muzzle::flashes(p.weapon, silenced) {
                        p.flash_until = now + crate::muzzle::FLASH_SECONDS;
                    }
                }
                match *m {
                    PlayerAnim::Hurt(_, g) => p.anim.hurt(g, now),
                    _ => p.anim.shot(now),
                }
            }
            PlayerAnim::Died(id) => {
                look.dying.insert(id);
                let Some((e, p)) = live.iter().find(|(_, p)| p.id == id) else { continue };
                // The corpse keeps the body; the gun drops (the server puts it on the ground).
                if let Some((_, g)) = &p.gun {
                    g.despawn(&mut commands);
                }
                commands.entity(p.flash).despawn();
                let model = &look.looks[p.look].model;
                let name = match look.last_hit.get(&id) {
                    _ if p.crouched => "crouch_die",
                    Some(HitGroup::Head) => "head",
                    Some(HitGroup::Stomach) => "gutshot",
                    Some(HitGroup::LeftArm) => "left",
                    Some(HitGroup::RightArm) => "right",
                    _ => ["death1", "death2", "death3", "back", "forward"][(id as usize + now as usize) % 5],
                };
                let s = seq(model, name).unwrap_or(0);
                commands.entity(e).remove::<OtherPlayer>().insert(Corpse { look: p.look, body: p.body.clone(), seq: s, started: now, done: false });
                // Old corpses go first.
                let mut old: Vec<_> = corpses.iter().map(|(e, c)| (c.started, e)).collect();
                if old.len() >= MAX_CORPSES {
                    old.sort_by(|a, b| a.0.total_cmp(&b.0));
                    for (_, e) in &old[..old.len() + 1 - MAX_CORPSES] {
                        commands.entity(*e).despawn();
                    }
                }
            }
        }
    }
}

fn place(o: &net::client::Remote) -> Transform {
    Transform::from_translation(Vec3::from(map::to_y_up(o.origin))).with_rotation(Quat::from_rotation_y(o.yaw.to_radians()))
}

/// Spawn models for players that appeared (or changed team or weapon), remove ones that left.
#[allow(clippy::too_many_arguments)]
fn draw_others(
    mut commands: Commands,
    w: Res<Walker>,
    mut look: ResMut<OtherPlayers>,
    mut cache: ResMut<models::ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<crate::studio::StudioMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    flash_look: Res<crate::muzzle::FlashLook>,
    drawn: Query<(Entity, &OtherPlayer)>,
) {
    let all = w.client.others();
    // Dead in the snapshot now: they can be drawn again once they respawn.
    look.dying.retain(|id| all.iter().any(|o| o.id == *id && o.alive));
    // Their characters from the scoreboard (until it arrives, varied by id like the server does).
    let looks_of = |id: u8| w.client.scores.iter().find(|s| s.id == id).map(|s| s.looks).unwrap_or([id % net::protocol::CLASSES; 2]);
    let others: Vec<_> = all.into_iter().filter(|o| o.alive && !look.dying.contains(&o.id)).filter_map(|o| Some((look.look_for(looks_of(o.id), o.team)?, o))).collect();
    for (e, p) in &drawn {
        if !others.iter().any(|(li, o)| o.id == p.id && *li == p.look && o.weapon == p.weapon) {
            commands.entity(e).despawn();
        }
    }
    for (li, o) in &others {
        if drawn.iter().any(|(_, d)| d.id == o.id && d.look == *li && d.weapon == o.weapon) {
            continue;
        }
        let body = look.looks[*li].clone();
        let parent = commands.spawn((place(o), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        let joints = body.spawn_skinned(&mut commands, parent, None, &mut meshes, &mut bindposes, &mut materials).joints;
        let gun = cache.get(&models::weapon_model("p", o.weapon), &mut images, &mut materials).map(|wm| {
            let s = wm.spawn_skinned(&mut commands, parent, None, &mut meshes, &mut bindposes, &mut materials);
            (wm, s)
        });
        let flash = crate::muzzle::spawn(&mut commands, &flash_look);
        commands.entity(parent).add_child(flash);
        commands.entity(parent).insert(OtherPlayer { id: o.id, look: *li, weapon: o.weapon, body: joints, gun, anim: sim::anim::Anim::new(o.yaw), crouched: o.crouched, flash, flash_until: 0.0 });
    }
}

/// Pose every player's (and corpse's) bones for this frame; the GPU does the rest.
fn animate(
    mut commands: Commands,
    flash_look: Res<crate::muzzle::FlashLook>,
    w: Res<Walker>,
    look: Res<OtherPlayers>,
    time: Res<Time>,
    mut live: Query<(Entity, &mut OtherPlayer, &mut Transform), (Without<Corpse>, Without<ChildOf>)>,
    mut corpses: Query<&mut Corpse>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
    mut vis: Query<&mut Visibility>,
    cam: Single<&GlobalTransform, With<crate::game::FlyCam>>,
    watch: Res<crate::spectate::Watch>,
) {
    // Watching someone through their eyes: their own body isn't drawn.
    let in_eye = crate::spectate::in_eye(&w, &watch).map(|o| o.id);
    let cam_rot = cam.compute_transform().rotation;
    let (now, dt) = (time.elapsed_secs(), time.delta_secs().max(1e-4));
    let others = w.client.others();
    for (e, mut p, mut tf) in &mut live {
        let Some(o) = others.iter().find(|o| o.id == p.id) else { continue };
        if let Ok(mut v) = vis.get_mut(e) {
            v.set_if_neq(if in_eye == Some(p.id) { Visibility::Hidden } else { Visibility::Inherited });
        }
        let model = &look.looks[p.look].model;
        p.crouched = o.crouched;
        let d = sim::anim::Drive { origin: o.origin, yaw: o.yaw, pitch: o.pitch, crouched: o.crouched, airborne: o.airborne, reloading: o.reloading, weapon: p.weapon };
        p.anim.step(model, &d, now, dt);
        let posed = p.anim.pose(model, &d, now);
        *tf = Transform::from_translation(Vec3::from(map::to_y_up(o.origin))).with_rotation(Quat::from_rotation_y(posed.yaw.to_radians()));
        let pose = posed.bones;
        models::set_pose(&p.body, &pose, &mut bones);
        let lit = now < p.flash_until;
        if let Some((wm, g)) = &p.gun {
            let gun_pose = wm.model.borrow_pose(model, &pose);
            models::set_pose(&g.joints, &gun_pose, &mut bones);
            // Muzzle flash at the p_ model's muzzle, facing the camera.
            if let (true, Some(at)) = (lit, wm.model.attachment(0, &gun_pose)) {
                if let Ok(mut t) = bones.get_mut(p.flash) {
                    t.translation = Vec3::from(map::to_y_up(at));
                    t.rotation = tf.rotation.inverse() * cam_rot * Quat::from_rotation_z((now * 7919.0).fract() * std::f32::consts::TAU);
                    let (material, width) = flash_look.pick(p.weapon, (p.flash_until * 7.31).fract());
                    t.scale = Vec3::splat(width);
                    commands.entity(p.flash).insert(MeshMaterial3d(material));
                }
            }
        }
        if let Ok(mut v) = vis.get_mut(p.flash) {
            v.set_if_neq(if lit { Visibility::Inherited } else { Visibility::Hidden });
        }
    }
    for mut c in &mut corpses {
        if c.done {
            continue;
        }
        let model = &look.looks[c.look].model;
        let sq = &model.sequences[c.seq];
        let frame = (now - c.started) * sq.fps;
        c.done = frame >= sq.frames as f32 - 1.0;
        models::set_pose(&c.body, &model.pose_at(c.seq, frame, 0.5), &mut bones);
    }
}
