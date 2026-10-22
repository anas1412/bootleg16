//! Other players: their character (`characters`), animated like GoldSrc does it (legs from
//! a gait sequence, upper body aiming up/down with the weapon's pose), shooting and flinching
//! when hit; once dead, `bodies` draws their body. The animation runs on the
//! CS skeleton the server's hitboxes use (arctic.mdl, never drawn) and each character copies
//! it (`characters::rig`), so every body moves exactly like its hitboxes.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::{map, models};
use bevy::prelude::*;
use bevy::mesh::skinning::SkinnedMeshInverseBindposes;
use sim::hitbox::HitGroup;
use sim::weapons::Id;
use std::collections::HashSet;
use std::sync::Arc;

pub struct PlayersPlugin;

impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerAnim>()
            .add_systems(Startup, load_looks)
            .add_systems(OnEnter(AppState::InGame), |mut l: ResMut<OtherPlayers>| l.dying.clear())
            .add_systems(Update, (anim_events, draw_others, animate).chain().in_set(Frame::World).run_if(resource_exists::<OtherPlayers>));
    }
}

/// Something that changes how a player is animated (sent by `effects::events`).
#[derive(Message)]
pub enum PlayerAnim {
    /// They fired (silenced: no muzzle flash).
    Shot(u8, bool),
    /// Hurt: where.
    Hurt(u8, HitGroup),
    /// Died (the server makes their body: `bodies`).
    Died(u8),
}

#[derive(Resource)]
pub(crate) struct OtherPlayers {
    /// The CS skeleton everyone is animated on (the server's hitbox model).
    pub(crate) cs: Arc<models::Loaded>,
    /// CS's C4 on the carrier's back (the player model's "backpack" sub-model) and the bone
    /// it hangs on.
    backpack: models::Parts,
    bomb_bone: Option<usize>,
    /// Characters, loaded the first time someone wears one (by `Characters` index).
    looks: Vec<Option<Arc<crate::characters::look::Look>>>,
    /// Died, but the snapshot we draw from may still show them alive: don't redraw them yet.
    dying: HashSet<u8>,
}

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<crate::studio::StudioMaterial>>, mut meshes: ResMut<Assets<Mesh>>) {
    let Some(cs) = cache.get("player/arctic/arctic.mdl", &mut images, &mut materials) else {
        error!("the game data is missing models/player/arctic/arctic.mdl: players can't be drawn");
        return;
    };
    let backpack = models::sub_model_parts(&cs, "backpack", 1, &mut meshes);
    let bomb_bone = cs.model.bones.iter().position(|b| b.name == "Bomb");
    commands.insert_resource(OtherPlayers { cs, backpack, bomb_bone, looks: Vec::new(), dying: HashSet::new() });
}

impl OtherPlayers {
    /// Character `i`, loaded on first use (None if its files don't load).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn look(&mut self, i: usize, characters: &crate::characters::Characters, cache: &mut models::ModelCache, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, images: &mut Assets<Image>, materials: &mut Assets<crate::studio::StudioMaterial>) -> Option<Arc<crate::characters::look::Look>> {
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
    /// The C4 on their back while they carry it.
    pack: Option<Entity>,
}

fn anim_events(
    mut commands: Commands,
    mut msgs: MessageReader<PlayerAnim>,
    mut look: ResMut<OtherPlayers>,
    time: Res<Time>,
    mut live: Query<(Entity, &mut OtherPlayer)>,
) {
    let now = time.elapsed_secs();
    for m in msgs.read() {
        match *m {
            PlayerAnim::Shot(id, _) | PlayerAnim::Hurt(id, _) => {
                let Some((_, mut p)) = live.iter_mut().find(|(_, p)| p.id == id) else { continue };
                match *m {
                    PlayerAnim::Shot(_, silenced) => {
                        if crate::muzzle::flashes(p.weapon, silenced) {
                            p.flash_until = now + crate::muzzle::FLASH_SECONDS;
                        }
                        p.anim.shot(now);
                    }
                    PlayerAnim::Hurt(_, g) => p.anim.hurt(g, now),
                    _ => {}
                }
            }
            // The live model goes; the server's body takes its place (`bodies`).
            PlayerAnim::Died(id) => {
                look.dying.insert(id);
                if let Some((e, _)) = live.iter().find(|(_, p)| p.id == id) {
                    commands.entity(e).despawn();
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
        commands.entity(parent).insert(OtherPlayer { id: o.id, look: *li, character, weapon: o.weapon, body: joints, gun, anim: sim::anim::Anim::new(o.yaw), crouched: o.crouched, flash, flash_until: 0.0, pack: None });
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
    mut live: Query<(Entity, &mut OtherPlayer, &mut Transform), Without<ChildOf>>,
    mut bones: Query<&mut Transform, With<ChildOf>>,
    mut vis: Query<&mut Visibility>,
    cam: Single<&GlobalTransform, With<crate::game::FlyCam>>,
    watch: Res<crate::spectate::Watch>,
    mut studio: ResMut<Assets<crate::studio::StudioMaterial>>,
) {
    // Watching someone through their eyes: their own body isn't drawn.
    let in_eye = crate::spectate::in_eye(&w, &watch).map(|o| o.id);
    let cam_rot = cam.compute_transform().rotation;
    // A player wholly behind the camera keeps moving but isn't posed (nobody sees the bones;
    // they're culled too, see `models::body_bounds`). Yours always is (the death cam).
    let (cam_at, cam_fwd) = (cam.translation(), cam.forward().as_vec3());
    let behind = |at: Vec3| (at - cam_at).dot(cam_fwd) < -64.0;
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
        let lit = now < p.flash_until;
        // The bomb carrier wears the C4 on their back (as you see them: enemies only learn
        // who carries it from players they're sent).
        let carrying = matches!(w.client.bomb, net::protocol::BombNet::Carried(id) if id == p.id);
        match (carrying, p.pack) {
            (true, None) => {
                let pack = commands.spawn((Transform::default(), Visibility::default(), ChildOf(e))).id();
                models::spawn_lit(&mut commands, pack, &look.backpack, &mut studio);
                p.pack = Some(pack);
            }
            (false, Some(pack)) => {
                commands.entity(pack).try_despawn();
                p.pack = None;
            }
            _ => {}
        }
        if me != Some(p.id) && behind(tf.translation) {
            continue;
        }
        let pose = posed.bones;
        p.character.set_pose(&p.body, &pose, &mut bones);
        if let (Some(pack), Some(b)) = (p.pack, look.bomb_bone) {
            models::set_pose(&[pack], &pose[b..=b], &mut bones);
        }
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
}
