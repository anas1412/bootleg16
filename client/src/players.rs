//! Other players: their CS model posed for the weapon they hold, where the server says they are.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::{map, models};
use bevy::prelude::*;
use net::protocol::Team;
use std::collections::HashMap;
use std::sync::Arc;

pub struct PlayersPlugin;

impl Plugin for PlayersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, load_looks).add_systems(Update, draw_others.in_set(Frame::World));
    }
}

pub type Parts = Vec<(Handle<Mesh>, Handle<StandardMaterial>)>;

/// The player model files for each side (CS 1.6 classes).
const T_MODELS: [&str; 4] = ["terror", "leet", "arctic", "guerilla"];
const CT_MODELS: [&str; 4] = ["urban", "gsg9", "sas", "gign"];

#[derive(Resource)]
struct OtherPlayers {
    /// T looks, then CT looks.
    looks: Vec<Arc<models::Loaded>>,
    t_count: usize,
    /// Body plus weapon-in-hand meshes for (look, crouched, weapon), built on first use.
    posed: HashMap<(usize, bool, sim::weapons::Id), Parts>,
}

fn load_looks(mut commands: Commands, mut cache: ResMut<models::ModelCache>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut load = |names: &[&str]| -> Vec<_> { names.iter().filter_map(|n| cache.get(&format!("player/{n}/{n}.mdl"), &mut images, &mut materials)).collect() };
    let mut looks = load(&T_MODELS);
    let t_count = looks.len();
    looks.extend(load(&CT_MODELS));
    commands.insert_resource(OtherPlayers { looks, t_count, posed: HashMap::new() });
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

/// The player body animation set CS uses for each weapon (`ref_aim_<ext>`, `crouch_aim_<ext>`).
fn anim_ext(id: sim::weapons::Id) -> &'static str {
    use sim::weapons::Id::*;
    match id {
        Knife => "knife",
        Glock | Usp | P228 | Deagle | FiveSeven => "onehanded",
        Elite => "dualpistols",
        M3 | Xm1014 => "shotgun",
        Mac10 | Tmp | Mp5 | Ump45 | P90 => "mp5",
        Galil | Ak47 => "ak47",
        Famas | M4a1 | Sg552 | Aug => "carbine",
        Scout | Awp | G3sg1 | Sg550 => "rifle",
        M249 => "m249",
        HeGrenade | Flashbang | Smoke => "grenade",
        C4 => "c4",
    }
}

/// Another player's model; `look`, `crouched` and `weapon` are what its children were built for.
#[derive(Component)]
struct OtherPlayer {
    id: u8,
    key: (usize, bool, sim::weapons::Id),
}

/// Each other player's model, where the server (interpolated) says they are.
#[allow(clippy::too_many_arguments)]
fn draw_others(
    mut commands: Commands,
    w: Res<Walker>,
    mut look: ResMut<OtherPlayers>,
    mut cache: ResMut<models::ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut drawn: Query<(Entity, &OtherPlayer, &mut Transform)>,
) {
    let others: Vec<_> = w.client.others().into_iter().filter(|o| o.alive).filter_map(|o| Some((look.look_for(o.id, o.team)?, o))).collect();
    let place = |o: &net::client::Remote| {
        Transform::from_translation(Vec3::from(map::to_y_up(o.origin))).with_rotation(Quat::from_rotation_y(o.yaw.to_radians()))
    };
    let key = |li: usize, o: &net::client::Remote| (li, o.crouched, o.weapon);
    for (e, p, mut tf) in &mut drawn {
        match others.iter().find(|(_, o)| o.id == p.id) {
            Some((li, o)) if key(*li, o) == p.key => *tf = place(o),
            // Gone, or stance, team or weapon changed: rebuild.
            _ => commands.entity(e).despawn(),
        }
    }
    for (li, o) in &others {
        let k = key(*li, o);
        if drawn.iter().any(|(_, d, _)| d.id == o.id && d.key == k) {
            continue;
        }
        if !look.posed.contains_key(&k) {
            // Body in the weapon's aim pose; the p_ weapon model borrows that skeleton by bone name.
            let body = look.looks[*li].clone();
            let seq = format!("{}_aim_{}", if o.crouched { "crouch" } else { "ref" }, anim_ext(o.weapon));
            let seq = body.model.sequence(&seq).or_else(|| body.model.sequence(if o.crouched { "crouch_aim_carbine" } else { "ref_aim_carbine" })).unwrap_or(0);
            let pose = body.model.pose(seq, 0, 0.5);
            let mut parts = models::static_parts(&body, &pose, &mut meshes);
            if let Some(wm) = cache.get(&models::weapon_model("p", o.weapon), &mut images, &mut materials) {
                parts.extend(models::static_parts(&wm, &wm.model.borrow_pose(&body.model, &pose), &mut meshes));
            }
            look.posed.insert(k, parts);
        }
        let parent = commands.spawn((OtherPlayer { id: o.id, key: k }, place(o), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for (mesh, material) in &look.posed[&k] {
            let child = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()))).id();
            commands.entity(parent).add_child(child);
        }
    }
}
