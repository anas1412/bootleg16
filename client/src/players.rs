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
            .add_systems(Update, (anim_events, draw_others, animate).chain().in_set(Frame::World));
    }
}

/// Something that changes how a player is animated (sent by `effects::events`).
#[derive(Message)]
pub enum PlayerAnim {
    Shot(u8),
    Hurt(u8, HitGroup),
    Died(u8),
}

/// The player model files for each side (CS 1.6 classes).
const T_MODELS: [&str; 4] = ["terror", "leet", "arctic", "guerilla"];
const CT_MODELS: [&str; 4] = ["urban", "gsg9", "sas", "gign"];
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

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut load = |names: &[&str]| -> Vec<_> { names.iter().filter_map(|n| cache.get(&format!("player/{n}/{n}.mdl"), &mut images, &mut materials)).collect() };
    let mut looks = load(&T_MODELS);
    let t_count = looks.len();
    looks.extend(load(&CT_MODELS));
    commands.insert_resource(OtherPlayers { looks, t_count, dying: HashSet::new(), last_hit: HashMap::new() });
}

impl OtherPlayers {
    /// Which look a player gets: by team, varied by id.
    fn look_for(&self, id: u8, team: Team) -> Option<usize> {
        let (start, n) = match team {
            Team::Ct => (self.t_count, self.looks.len() - self.t_count),
            _ => (0, self.t_count),
        };
        (n > 0).then(|| start + id as usize % n)
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
    gait_frame: f32,
    last_origin: [f32; 3],
    speed: f32,
    crouched: bool,
    /// A one-off upper body animation (shooting, flinching): sequence and start time.
    action: Option<(usize, f32)>,
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
            PlayerAnim::Shot(id) | PlayerAnim::Hurt(id, _) => {
                if let PlayerAnim::Hurt(_, g) = *m {
                    look.last_hit.insert(id, g);
                }
                let Some((_, mut p)) = live.iter_mut().find(|(_, p)| p.id == id) else { continue };
                let model = &look.looks[p.look].model;
                let name = match *m {
                    PlayerAnim::Hurt(_, HitGroup::Head) => "head_flinch".to_string(),
                    PlayerAnim::Hurt(..) => "gut_flinch".to_string(),
                    _ => format!("{}_shoot_{}", if p.crouched { "crouch" } else { "ref" }, sim::weapons::data::anim_ext(p.weapon)),
                };
                if let Some(s) = seq(model, &name) {
                    p.action = Some((s, now));
                }
            }
            PlayerAnim::Died(id) => {
                look.dying.insert(id);
                let Some((e, p)) = live.iter().find(|(_, p)| p.id == id) else { continue };
                // The corpse keeps the body; the gun drops (the server puts it on the ground).
                if let Some((_, g)) = &p.gun {
                    g.despawn(&mut commands);
                }
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
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    drawn: Query<(Entity, &OtherPlayer)>,
) {
    let all = w.client.others();
    // Dead in the snapshot now: they can be drawn again once they respawn.
    look.dying.retain(|id| all.iter().any(|o| o.id == *id && o.alive));
    let others: Vec<_> = all.into_iter().filter(|o| o.alive && !look.dying.contains(&o.id)).filter_map(|o| Some((look.look_for(o.id, o.team)?, o))).collect();
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
        let joints = body.spawn_skinned(&mut commands, parent, None, &mut meshes, &mut bindposes).joints;
        let gun = cache.get(&models::weapon_model("p", o.weapon), &mut images, &mut materials).map(|wm| {
            let s = wm.spawn_skinned(&mut commands, parent, None, &mut meshes, &mut bindposes);
            (wm, s)
        });
        commands.entity(parent).insert(OtherPlayer { id: o.id, look: *li, weapon: o.weapon, body: joints, gun, gait_frame: 0.0, last_origin: o.origin, speed: 0.0, crouched: o.crouched, action: None });
    }
}

/// Pose every player's (and corpse's) bones for this frame; the GPU does the rest.
fn animate(
    w: Res<Walker>,
    look: Res<OtherPlayers>,
    time: Res<Time>,
    mut live: Query<(&mut OtherPlayer, &mut Transform), (Without<Corpse>, Without<ChildOf>)>,
    mut corpses: Query<&mut Corpse>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
) {
    let (now, dt) = (time.elapsed_secs(), time.delta_secs().max(1e-4));
    let others = w.client.others();
    for (mut p, mut tf) in &mut live {
        let Some(o) = others.iter().find(|o| o.id == p.id) else { continue };
        *tf = place(o);
        let model = &look.looks[p.look].model;
        // Speed from how far they moved (smoothed).
        let moved = ((o.origin[0] - p.last_origin[0]).powi(2) + (o.origin[1] - p.last_origin[1]).powi(2)).sqrt() / dt;
        p.last_origin = o.origin;
        p.speed = p.speed * 0.8 + moved.min(400.0) * 0.2;
        p.crouched = o.crouched;
        let speed = p.speed;
        let stance = if o.crouched { "crouch" } else { "ref" };
        // Legs: idle, walk or run, played faster the faster they go (GoldSrc ties gait to distance).
        let (gait_name, nominal) = match (o.crouched, speed) {
            (true, s) if s > 10.0 => ("crouchrun", 85.0),
            (true, _) => ("crouch_idle", 0.0),
            (false, s) if s > 150.0 => ("run", 250.0),
            (false, s) if s > 10.0 => ("walk", 130.0),
            _ => ("idle1", 0.0),
        };
        let gait = seq(model, gait_name).unwrap_or(0);
        let gs = &model.sequences[gait];
        let rate = if nominal > 0.0 { (speed / nominal).clamp(0.5, 1.5) } else { 1.0 };
        p.gait_frame = (p.gait_frame + gs.fps * rate * dt) % gs.frames.max(1) as f32;
        // Upper body: a one-off action, else the weapon's aim pose.
        let aim = seq(model, &format!("{stance}_aim_{}", sim::weapons::data::anim_ext(p.weapon))).or_else(|| seq(model, &format!("{stance}_aim_carbine"))).unwrap_or(0);
        let mut upper = (aim, 0.0);
        if let Some((s, t0)) = p.action {
            let sq = &model.sequences[s];
            let frame = (now - t0) * sq.fps;
            if frame < sq.frames as f32 - 1.0 {
                upper = (s, frame);
            } else {
                p.action = None;
            }
        }
        // Aim pitch picks the blend (9 sets from looking up to looking down; GoldSrc pitch is down-positive).
        let blend = 0.5 + o.pitch.clamp(-90.0, 90.0) / 180.0;
        let pose = model.player_pose((gait, p.gait_frame), upper, blend);
        models::set_pose(&p.body, &pose, &mut bones);
        if let Some((wm, g)) = &p.gun {
            models::set_pose(&g.joints, &wm.model.borrow_pose(model, &pose), &mut bones);
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
