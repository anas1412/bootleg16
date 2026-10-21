//! Other players: their character (`characters`), animated like GoldSrc does it (legs from
//! a gait sequence, upper body aiming up/down with the weapon's pose), shooting and flinching
//! when hit, and a corpse playing a death animation when they die. The animation runs on the
//! CS skeleton the server's hitboxes use (arctic.mdl, never drawn) and each character copies
//! it (`characters::rig`), so every body moves exactly like its hitboxes.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::{map, models};
use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
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
                l.blasted.clear();
            })
            .add_systems(Update, (clear_corpses, anim_events, draw_others, animate).chain().in_set(Frame::World).run_if(resource_exists::<OtherPlayers>));
    }
}

/// Bodies stay the whole match (our choice; CS clears them each round): gone when a new one
/// starts (round 1 after the warmup) or a deathmatch game ends.
fn clear_corpses(mut commands: Commands, mut news: MessageReader<crate::effects::GameNews>, corpses: Query<Entity, With<Corpse>>) {
    if news.read().any(|n| matches!(n.0, net::protocol::GameEvent::RoundStart { round: 1 } | net::protocol::GameEvent::KillLimit { .. })) {
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
    /// That hurt was an explosion here (a grenade or the bomb).
    Blasted(u8, [f32; 3]),
    Died(u8),
}

/// Corpses kept at once (a whole match's: 18 rounds of 10 players is 180).
const MAX_CORPSES: usize = 256;

#[derive(Resource)]
struct OtherPlayers {
    /// The CS skeleton everyone is animated on (the server's hitbox model).
    cs: Arc<models::Loaded>,
    /// Characters, loaded the first time someone wears one (by `Characters` index).
    looks: Vec<Option<Arc<crate::characters::look::Look>>>,
    /// Died, but the snapshot we draw from may still show them alive: don't redraw them yet.
    dying: HashSet<u8>,
    /// Where each player was last hit (picks the death animation).
    last_hit: HashMap<u8, HitGroup>,
    /// Where the explosion that last hurt them was, if the last hurt was one.
    blasted: HashMap<u8, [f32; 3]>,
}

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<crate::studio::StudioMaterial>>) {
    let Some(cs) = cache.get("player/arctic/arctic.mdl", &mut images, &mut materials) else {
        error!("the game data is missing models/player/arctic/arctic.mdl: players can't be drawn");
        return;
    };
    commands.insert_resource(OtherPlayers { cs, looks: Vec::new(), dying: HashSet::new(), last_hit: HashMap::new(), blasted: HashMap::new() });
}

impl OtherPlayers {
    /// Character `i`, loaded on first use (None if its files don't load).
    #[allow(clippy::too_many_arguments)]
    fn look(&mut self, i: usize, characters: &crate::characters::Characters, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, images: &mut Assets<Image>, materials: &mut Assets<crate::studio::StudioMaterial>) -> Option<Arc<crate::characters::look::Look>> {
        // the list is read at startup and may not have existed yet when this was made
        if self.looks.len() < characters.0.len() {
            self.looks.resize(characters.0.len(), None);
        }
        if self.looks.get(i)?.is_none() {
            let entry = characters.0.get(i)?;
            match crate::characters::look::Look::load(&entry.dir, &self.cs.model, meshes, bindposes, images, materials) {
                Ok(l) => self.looks[i] = Some(Arc::new(l)),
                Err(e) => {
                    error!("character {}: {e}", entry.id);
                    return None;
                }
            }
        }
        self.looks[i].clone()
    }
}

/// A live player's model. Children are the body parts, then the weapon parts.
#[derive(Component)]
struct OtherPlayer {
    id: u8,
    look: usize,
    character: Arc<crate::characters::look::Look>,
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
    character: Arc<crate::characters::look::Look>,
    body: Vec<Entity>,
    seq: usize,
    started: f32,
    done: bool,
}

/// CS's death activities as the player models name them: ACT_DIE_HEADSHOT, _CHESTSHOT,
/// _GUTSHOT, _BACKSHOT.
const DIE_HEAD: &str = "head";
const DIE_CHEST: &str = "death1";
const DIE_GUT: &str = "gutshot";
const DIE_BACK: &str = "death3";

/// ACT_DIESIMPLE: one of its sequences by their weights (death2 twice as likely).
fn die_simple(roll: u32) -> &'static str {
    ["death2", "death2", "left", "right"][roll as usize % 4]
}

fn seq(model: &goldsrc::mdl::Model, name: &str) -> Option<usize> {
    model.sequence(name)
}

fn anim_events(
    mut commands: Commands,
    mut msgs: MessageReader<PlayerAnim>,
    mut look: ResMut<OtherPlayers>,
    time: Res<Time>,
    mut live: Query<(Entity, &mut OtherPlayer, &Transform)>,
    corpses: Query<(Entity, &Corpse)>,
) {
    let now = time.elapsed_secs();
    for m in msgs.read() {
        match *m {
            PlayerAnim::Shot(id, _) | PlayerAnim::Hurt(id, _) => {
                if let PlayerAnim::Hurt(_, g) = *m {
                    look.last_hit.insert(id, g);
                    look.blasted.remove(&id);
                }
                let Some((_, mut p, _)) = live.iter_mut().find(|(_, p, _)| p.id == id) else { continue };
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
            PlayerAnim::Blasted(id, at) => {
                look.blasted.insert(id, at);
            }
            PlayerAnim::Died(id) => {
                look.dying.insert(id);
                let Some((e, p, tf)) = live.iter().find(|(_, p, _)| p.id == id) else { continue };
                // The corpse keeps the body; the gun drops (the server puts it on the ground).
                if let Some((_, g)) = &p.gun {
                    g.despawn(&mut commands);
                }
                commands.entity(p.flash).despawn();
                let model = &look.cs.model;
                // CS 1.6 `CBasePlayer::SetAnimation(PLAYER_DIE)`: by where the last hit landed.
                let roll = (now.to_bits() ^ (id as u32).wrapping_mul(2654435761)).wrapping_mul(2246822519) >> 8;
                let blast = look.blasted.get(&id).map(|b| {
                    // the blast behind them throws them forward ("left"), in front of them back
                    let o = tf.translation;
                    let (dx, dy) = (o.x - b[0], -o.z - b[1]);
                    let (s, c) = p.anim.gait_yaw.to_radians().sin_cos();
                    c * dx + s * dy > 0.0
                });
                let name = match (p.crouched, blast, look.last_hit.get(&id)) {
                    (true, _, _) => "crouch_die",
                    (_, Some(true), _) => "left",
                    (_, Some(false), _) => if roll % 2 == 0 { DIE_HEAD } else { "crouch_die" },
                    (_, _, Some(HitGroup::Head)) => DIE_HEAD,
                    (_, _, Some(HitGroup::Chest)) => DIE_CHEST,
                    (_, _, Some(HitGroup::Stomach)) => DIE_GUT,
                    (_, _, Some(HitGroup::LeftArm)) => "left",
                    (_, _, Some(HitGroup::RightArm)) => "right",
                    (_, _, Some(HitGroup::Generic) | None) => match roll % 9 {
                        0 | 8 => DIE_HEAD,
                        1 | 7 => DIE_GUT,
                        2 => DIE_BACK,
                        4 => "back",
                        5 => "forward",
                        6 => DIE_CHEST,
                        _ => die_simple(roll / 9),
                    },
                    _ => die_simple(roll / 9),
                };
                let s = seq(model, name).unwrap_or(0);
                commands.entity(e).remove::<OtherPlayer>().insert(Corpse { character: p.character.clone(), body: p.body.clone(), seq: s, started: now, done: false });
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
    characters: Res<crate::characters::Characters>,
) {
    let all = w.client.others();
    // Dead in the snapshot now: they can be drawn again once they respawn.
    look.dying.retain(|id| all.iter().any(|o| o.id == *id && o.alive));
    // Their character, from the scoreboard (not picked, or not installed here: one by player number).
    // screenshot runs: BOOTLEG16_CHARACTER=<id> dresses everyone else as that character
    let forced = std::env::var("BOOTLEG16_CHARACTER").ok();
    let pick = |id: u8| forced.as_deref().unwrap_or_else(|| w.client.scores.iter().find(|s| s.id == id).map(|s| s.character.as_str()).unwrap_or(""));
    let others: Vec<_> = all.into_iter().filter(|o| o.alive && !look.dying.contains(&o.id)).filter_map(|o| Some((characters.for_player(pick(o.id), o.id, o.team)?, o))).collect();
    for (e, p) in &drawn {
        if !others.iter().any(|(li, o)| o.id == p.id && *li == p.look && o.weapon == p.weapon) {
            commands.entity(e).despawn();
        }
    }
    for (li, o) in &others {
        if drawn.iter().any(|(_, d)| d.id == o.id && d.look == *li && d.weapon == o.weapon) {
            continue;
        }
        let Some(character) = look.look(*li, &characters, &mut meshes, &mut bindposes, &mut images, &mut materials) else { continue };
        let parent = commands.spawn((place(o), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        let joints = character.spawn(&mut commands, parent, &mut materials);
        let gun = cache.get(&models::weapon_model("p", o.weapon), &mut images, &mut materials).map(|wm| {
            let s = wm.spawn_skinned(&mut commands, parent, None, &mut meshes, &mut bindposes, &mut materials);
            (wm, s)
        });
        let flash = crate::muzzle::spawn(&mut commands, &flash_look);
        commands.entity(parent).add_child(flash);
        commands.entity(parent).insert(OtherPlayer { id: o.id, look: *li, character, weapon: o.weapon, body: joints, gun, anim: sim::anim::Anim::new(o.yaw), crouched: o.crouched, flash, flash_until: 0.0 });
    }
}

/// Pose every player's (and corpse's) bones for this frame; the GPU does the rest.
fn animate(
    mut commands: Commands,
    flash_look: Res<crate::muzzle::FlashLook>,
    w: Res<Walker>,
    look: Res<OtherPlayers>,
    time: Res<Time>,
    fixed: Res<Time<Fixed>>,
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
    let others = w.client.others_at(fixed.overstep_fraction());
    for (e, mut p, mut tf) in &mut live {
        let Some(o) = others.iter().find(|o| o.id == p.id) else { continue };
        // spawn protection (deathmatch): they shimmer until they move or shoot
        let glow = if o.protected { 0.35 + 0.25 * (now * std::f32::consts::TAU * 2.0).sin() } else { 0.0 };
        commands.entity(e).insert(crate::studio::Glow(glow));
        if let Ok(mut v) = vis.get_mut(e) {
            v.set_if_neq(if in_eye == Some(p.id) { Visibility::Hidden } else { Visibility::Inherited });
        }
        let model = &look.cs.model;
        p.crouched = o.crouched;
        let d = sim::anim::Drive { origin: o.origin, yaw: o.yaw, pitch: o.pitch, crouched: o.crouched, airborne: o.airborne, reloading: o.reloading, weapon: p.weapon };
        p.anim.step(model, &d, now, dt);
        let posed = p.anim.pose(model, &d, now);
        *tf = Transform::from_translation(Vec3::from(map::to_y_up(o.origin))).with_rotation(Quat::from_rotation_y(posed.yaw.to_radians()));
        let pose = posed.bones;
        p.character.set_pose(&p.body, &pose, &mut bones);
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
        let model = &look.cs.model;
        let sq = &model.sequences[c.seq];
        let frame = (now - c.started) * sq.fps;
        c.done = frame >= sq.frames as f32 - 1.0;
        c.character.set_pose(&c.body, &model.pose_at(c.seq, frame, 0.5), &mut bones);
    }
}
