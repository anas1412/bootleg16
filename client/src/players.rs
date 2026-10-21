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
use sim::collision::Hull;
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
    /// Hurt: where, and how badly.
    Hurt(u8, HitGroup, f32),
    /// That hurt was an explosion here (a grenade, or the bomb: true).
    Blasted(u8, [f32; 3], bool),
    /// Died; where their killer was (for which way the body is thrown).
    Died(u8, Option<[f32; 3]>),
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
    /// How much the last hit took (CS: over 20 is a big one).
    last_damage: HashMap<u8, f32>,
    /// Where the explosion that last hurt them was, if the last hurt was one.
    blasted: HashMap<u8, ([f32; 3], bool)>,
}

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<crate::studio::StudioMaterial>>) {
    let Some(cs) = cache.get("player/arctic/arctic.mdl", &mut images, &mut materials) else {
        error!("the game data is missing models/player/arctic/arctic.mdl: players can't be drawn");
        return;
    };
    commands.insert_resource(OtherPlayers { cs, looks: Vec::new(), dying: HashSet::new(), last_hit: HashMap::new(), last_damage: HashMap::new(), blasted: HashMap::new() });
}

impl OtherPlayers {
    /// Character `i`, loaded on first use (None if its files don't load).
    #[allow(clippy::too_many_arguments)]
    fn look(&mut self, i: usize, characters: &crate::characters::Characters, cache: &mut models::ModelCache, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, images: &mut Assets<Image>, materials: &mut Assets<crate::studio::StudioMaterial>) -> Option<Arc<crate::characters::look::Look>> {
        // the list is read at startup and may not have existed yet when this was made
        if self.looks.len() < characters.0.len() {
            self.looks.resize(characters.0.len(), None);
        }
        if self.looks.get(i)?.is_none() {
            let entry = characters.0.get(i)?;
            match crate::characters::look::Look::load(&entry.dir, &self.cs, cache, meshes, bindposes, images, materials) {
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
    /// Still moving (thrown by the shot that killed them), GoldSrc units a second.
    vel: Vec3,
    hull: Hull,
}

/// How a body is thrown when they die (`CBasePlayer::Killed`'s `m_iThrowDirection`).
#[derive(Clone, Copy)]
enum Throw {
    None,
    /// Their own forward or back: 100-200 a second, and up 50-100.
    Forward,
    Back,
    /// Along the killer's aim, 200-300 (and up 200-300; flat for some arm shots).
    Shooter,
    ShooterFlat,
    /// Away from the grenade (500 less its distance, up 1.5 x 350 less it) or the bomb
    /// (2300 less its distance, a quarter of it, up a 2.75th).
    Blast,
}

impl Throw {
    fn velocity(self, at: [f32; 3], yaw: f32, killer: Option<[f32; 3]>, blast: Option<([f32; 3], bool)>, r: impl Fn(u32, f32, f32) -> f32) -> Vec3 {
        let (s, c) = yaw.to_radians().sin_cos();
        let aim = killer.map(|k| Vec3::new(at[0] - k[0], at[1] - k[1], 0.0).normalize_or_zero()).unwrap_or(Vec3::ZERO);
        match self {
            Throw::None => Vec3::ZERO,
            Throw::Forward => Vec3::new(c, s, 0.0) * r(3, 100.0, 200.0) + Vec3::Z * r(5, 50.0, 100.0),
            Throw::Back => Vec3::new(c, s, 0.0) * -r(3, 100.0, 200.0) + Vec3::Z * r(5, 50.0, 100.0),
            Throw::Shooter => aim * r(3, 200.0, 300.0) + Vec3::Z * r(5, 200.0, 300.0),
            Throw::ShooterFlat => aim * r(3, 200.0, 300.0),
            Throw::Blast => {
                let Some((b, bomb)) = blast else { return Vec3::ZERO };
                let v = Vec3::new(at[0] - b[0], at[1] - b[1], at[2] - b[2]);
                let (d, dir) = (v.length(), v.normalize_or_zero());
                if bomb { (dir * (2300.0 - d) * 0.25).with_z((2300.0 - d) / 2.75) } else { (dir * (500.0 - d)).with_z((350.0 - d) * 1.5) }
            }
        }
    }
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
            PlayerAnim::Shot(id, _) | PlayerAnim::Hurt(id, _, _) => {
                if let PlayerAnim::Hurt(_, g, amount) = *m {
                    look.last_hit.insert(id, g);
                    look.last_damage.insert(id, amount);
                    look.blasted.remove(&id);
                }
                let Some((_, mut p, _)) = live.iter_mut().find(|(_, p, _)| p.id == id) else { continue };
                if let PlayerAnim::Shot(_, silenced) = *m {
                    if crate::muzzle::flashes(p.weapon, silenced) {
                        p.flash_until = now + crate::muzzle::FLASH_SECONDS;
                    }
                }
                match *m {
                    PlayerAnim::Hurt(_, g, _) => p.anim.hurt(g, now),
                    _ => p.anim.shot(now),
                }
            }
            PlayerAnim::Blasted(id, at, bomb) => {
                look.blasted.insert(id, (at, bomb));
            }
            PlayerAnim::Died(id, killer) => {
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
                let blast = look.blasted.get(&id).map(|(b, _)| {
                    // the blast behind them throws them forward ("left"), in front of them back
                    let o = tf.translation;
                    let (dx, dy) = (o.x - b[0], -o.z - b[1]);
                    let (s, c) = p.anim.gait_yaw.to_radians().sin_cos();
                    c * dx + s * dy > 0.0
                });
                // (the animation, and how the body is thrown: `Throw`)
                let big = look.last_damage.get(&id).is_some_and(|d| *d > 20.0);
                let (name, throw) = match (p.crouched, blast, look.last_hit.get(&id)) {
                    (true, _, _) => ("crouch_die", Throw::Back),
                    (_, Some(true), _) => ("left", Throw::Blast),
                    (_, Some(false), _) => (if roll % 2 == 0 { DIE_HEAD } else { "crouch_die" }, Throw::Blast),
                    (_, _, Some(HitGroup::Head)) => (DIE_HEAD, match roll % 9 + big as u32 {
                        1 | 2 => Throw::Back,
                        3 | 4 => Throw::Forward,
                        5 | 6 => Throw::Shooter,
                        _ => Throw::None,
                    }),
                    (_, _, Some(HitGroup::Chest)) => (DIE_CHEST, Throw::None),
                    (_, _, Some(HitGroup::Stomach)) => (DIE_GUT, Throw::None),
                    (_, _, Some(HitGroup::LeftArm)) => ("left", Throw::None),
                    (_, _, Some(HitGroup::RightArm)) => ("right", if roll % 2 == 0 { Throw::Shooter } else { Throw::ShooterFlat }),
                    (_, _, Some(HitGroup::Generic) | None) => match roll % 9 {
                        0 => (DIE_HEAD, Throw::Back),
                        8 => (DIE_HEAD, Throw::None),
                        1 | 7 => (DIE_GUT, Throw::None),
                        2 => (DIE_BACK, Throw::Shooter),
                        4 => ("back", Throw::Shooter),
                        5 => ("forward", Throw::Forward),
                        6 => (DIE_CHEST, Throw::None),
                        _ => (die_simple(roll / 9), Throw::None),
                    },
                    _ => (die_simple(roll / 9), Throw::None),
                };
                let at = [tf.translation.x, -tf.translation.z, tf.translation.y];
                let r = |k: u32, lo: f32, hi: f32| lo + (hi - lo) * ((roll.wrapping_mul(k) >> 8) % 1000) as f32 / 1000.0;
                let vel = throw.velocity(at, p.anim.gait_yaw, killer, look.blasted.get(&id).copied(), r);
                let s = seq(model, name).unwrap_or(0);
                commands.entity(e).remove::<OtherPlayer>().insert(Visibility::Inherited).insert(Corpse { character: p.character.clone(), body: p.body.clone(), seq: s, started: now, done: false, vel, hull: if p.crouched { Hull::Crouched } else { Hull::Standing } });
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
    // you too: hidden while you play, your body once you die
    let mut all = w.client.others();
    all.extend(w.client.me_as_remote());
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
        let Some(character) = look.look(*li, &characters, &mut cache, &mut meshes, &mut bindposes, &mut images, &mut materials) else { continue };
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
    mut corpses: Query<(&mut Corpse, &mut Transform), (Without<OtherPlayer>, Without<ChildOf>)>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
    mut vis: Query<&mut Visibility>,
    cam: Single<&GlobalTransform, With<crate::game::FlyCam>>,
    watch: Res<crate::spectate::Watch>,
) {
    // Watching someone through their eyes: their own body isn't drawn.
    let in_eye = crate::spectate::in_eye(&w, &watch).map(|o| o.id);
    let cam_rot = cam.compute_transform().rotation;
    let (now, dt) = (time.elapsed_secs(), time.delta_secs().max(1e-4));
    let mut others = w.client.others_at(fixed.overstep_fraction());
    others.extend(w.client.me_as_remote());
    let me = w.client.id();
    for (e, mut p, mut tf) in &mut live {
        let Some(o) = others.iter().find(|o| o.id == p.id) else { continue };
        // spawn protection (deathmatch): they shimmer until they move or shoot
        let glow = if o.protected { 0.35 + 0.25 * (now * std::f32::consts::TAU * 2.0).sin() } else { 0.0 };
        commands.entity(e).insert(crate::studio::Glow(glow));
        if let Ok(mut v) = vis.get_mut(e) {
            v.set_if_neq(if in_eye == Some(p.id) || me == Some(p.id) { Visibility::Hidden } else { Visibility::Inherited });
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
    for (mut c, mut tf) in &mut corpses {
        // Thrown: flies (gravity 800) and slides to a stop (PlayerDeathThink: 20 off each think).
        if c.vel != Vec3::ZERO {
            let world = &w.client.world;
            let p = tf.translation;
            let at = [p.x, -p.z, p.y];
            c.vel.z -= 800.0 * dt;
            let to = [at[0] + c.vel.x * dt, at[1] + c.vel.y * dt, at[2] + c.vel.z * dt];
            let t = world.trace(c.hull, at, to);
            if t.fraction < 1.0 {
                let n = Vec3::from(t.normal);
                let into = n * c.vel.dot(n);
                c.vel -= into;
            }
            let ground = world.trace(c.hull, t.end, [t.end[0], t.end[1], t.end[2] - 2.0]).fraction < 1.0;
            if ground {
                c.vel.z = c.vel.z.max(0.0);
                // ponytail: CS thinks every server frame (~100 a second), so 2000 a second
                let speed = c.vel.length();
                c.vel = if speed > 2000.0 * dt { c.vel * (1.0 - 2000.0 * dt / speed) } else { Vec3::ZERO };
            }
            tf.translation = Vec3::from(map::to_y_up(t.end));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bodies_are_thrown_the_way_cs_throws_them() {
        let mid = |_: u32, lo: f32, hi: f32| (lo + hi) / 2.0;
        // facing +x: back is -x and a little up
        let v = Throw::Back.velocity([0.0; 3], 0.0, None, None, mid);
        assert!(v.x < -100.0 && v.z > 50.0, "{v}");
        // along the killer's aim, away from them, and up
        let v = Throw::Shooter.velocity([100.0, 0.0, 0.0], 0.0, Some([0.0; 3]), None, mid);
        assert!(v.x > 200.0 && v.z > 200.0, "{v}");
        // a grenade 100 units west throws them east (400) and up (375)
        let v = Throw::Blast.velocity([0.0; 3], 0.0, None, Some(([-100.0, 0.0, 0.0], false)), mid);
        assert!((v.x - 400.0).abs() < 1e-3 && (v.z - 375.0).abs() < 1e-3, "{v}");
    }
}
