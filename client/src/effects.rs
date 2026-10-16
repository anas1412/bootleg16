//! Server events turned into things you see and hear: tracers, the kill feed, grenades in
//! flight, explosions, the flashbang white-out and guns on the ground.
use crate::game::{Frame, Walker};
use crate::level::rgba_image;
use crate::menu::AppState;
use crate::impacts::Hit;
use crate::killfeed::{Kill, KillFeed};
use crate::round_hud::RoundNews;
use crate::models::Parts;
use crate::players::PlayerAnim;
use crate::{game_dir, map, models, sound};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::TextureFormat,
};
use net::protocol::{GameEvent, Team};

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Flash>()
            .add_message::<GameNews>()
            .add_systems(Startup, load)
            .add_systems(OnEnter(AppState::InGame), spawn_overlay)
            .add_systems(Update, events.in_set(Frame::Events))
            .add_systems(Update, (grenade_effects, ground_guns).in_set(Frame::World));
    }
}

#[derive(Resource)]
struct Effects {
    /// HE, flash, smoke world models.
    grenades: Vec<Parts>,
    sphere: Handle<Mesh>,
}

/// Flashbang: seconds of full white left, seconds of fade left (and total), strength.
#[derive(Resource, Default)]
struct Flash {
    hold: f32,
    fade: f32,
    total_fade: f32,
    alpha: f32,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let grenades = ["w_hegrenade", "w_flashbang", "w_smokegrenade"]
        .iter()
        .map(|name| {
            goldsrc::mdl::Model::load(game_dir().join(format!("cstrike/models/{name}.mdl")))
                .map(|m| model_parts(&m, 0, &mut meshes, &mut materials, &mut images))
                .unwrap_or_default()
        })
        .collect();
    commands.insert_resource(Effects {
        grenades,
        sphere: meshes.add(Sphere::new(1.0)),
    });
}

/// Full-screen white for flashbangs (invisible until flashed).
fn spawn_overlay(mut commands: Commands, mut flash: ResMut<Flash>) {
    *flash = Flash::default();
    commands.spawn((
        FlashOverlay,
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.0)),
        GlobalZIndex(10),
        DespawnOnExit(AppState::InGame),
    ));
}

/// Meshes and materials for one posed studio model.
fn model_parts(model: &goldsrc::mdl::Model, seq: usize, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, images: &mut Assets<Image>) -> Parts {
    let textures: Vec<Handle<StandardMaterial>> = model
        .textures()
        .into_iter()
        .map(|t| {
            let image = images.add(rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb));
            materials.add(StandardMaterial { base_color_texture: Some(image), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() })
        })
        .collect();
    model
        .posed_mesh(&model.pose(seq, 0, 0.5))
        .into_iter()
        .filter(|p| !p.indices.is_empty())
        .map(|p| {
            let positions: Vec<[f32; 3]> = p.positions.iter().map(|v| map::to_y_up(*v)).collect();
            let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uvs)
                .with_inserted_indices(Indices::U32(p.indices))
                .with_computed_normals();
            (meshes.add(mesh), textures.get(p.texture).cloned().unwrap_or_default())
        })
        .collect()
}

/// Spawn a model's parts under one parent at `at` (GoldSrc coordinates).
fn spawn_parts(commands: &mut Commands, marker: impl Bundle, at: [f32; 3], parts: &Parts) {
    let parent = commands.spawn((marker, Transform::from_translation(Vec3::from(map::to_y_up(at))), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
    for (mesh, mat) in parts {
        let child = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone()))).id();
        commands.entity(parent).add_child(child);
    }
}

#[derive(Component)]
struct GrenadeView;

#[derive(Component)]
struct GroundGunView;

/// Guns lying on the ground (CS `w_` models), redrawn from the latest snapshot.
#[allow(clippy::too_many_arguments)]
fn ground_guns(
    mut commands: Commands,
    w: Res<Walker>,
    mut cache: ResMut<models::ModelCache>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut parts: Local<std::collections::HashMap<sim::weapons::Id, Parts>>,
    mut shown: Local<Vec<(sim::weapons::Id, [f32; 3])>>,
    views: Query<Entity, With<GroundGunView>>,
) {
    if *shown == w.client.ground {
        return;
    }
    *shown = w.client.ground.clone();
    for e in &views {
        commands.entity(e).despawn();
    }
    for &(id, at) in &w.client.ground {
        let p = parts.entry(id).or_insert_with(|| match cache.get(&models::weapon_model("w", id), &mut images, &mut materials) {
            Some(m) => models::static_parts(&m, &m.model.pose(0, 0, 0.0), &mut meshes),
            None => Vec::new(),
        });
        spawn_parts(&mut commands, GroundGunView, at, p);
    }
}

/// A fireball: age in seconds, size (1 = HE grenade).
#[derive(Component)]
struct Fireball(f32, f32);


#[derive(Component)]
struct FlashOverlay;

/// Grenades in flight, fireballs, and the flashbang white-out (smoke: `smoke.rs`).
#[allow(clippy::too_many_arguments)]
fn grenade_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut w: ResMut<Walker>,
    mut flash: ResMut<Flash>,
    fx: Res<Effects>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    views: Query<Entity, With<GrenadeView>>,
    mut fireballs: Query<(Entity, &mut Fireball, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut overlay: Single<&mut BackgroundColor, With<FlashOverlay>>,
) {
    let dt = time.delta_secs();
    // Grenades in flight: few enough to redraw from scratch every frame they exist.
    for e in &views {
        commands.entity(e).despawn();
    }
    for (kind, at) in w.client.grenades.clone() {
        let i = match kind {
            sim::grenades::Kind::He => 0,
            sim::grenades::Kind::Flash => 1,
            sim::grenades::Kind::Smoke => 2,
        };
        spawn_parts(&mut commands, GrenadeView, at, &fx.grenades[i]);
    }
    // Explosions and flashes from server events (sorted out in `events`).
    let me = w.client.id();
    for e in std::mem::take(&mut w.pending_fx) {
        match e {
            GameEvent::Explosion { at } | GameEvent::BombExploded { at } => {
                let size = if matches!(e, GameEvent::BombExploded { .. }) { 4.0 } else { 1.0 };
                let mat = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 0.6, 0.15, 0.9), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
                commands.spawn((
                    Fireball(0.0, size),
                    Mesh3d(fx.sphere.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(Vec3::from(map::to_y_up(at))).with_scale(Vec3::splat(20.0)),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            GameEvent::Flash { blinded, .. } => {
                if let Some((_, b)) = blinded.iter().find(|(id, _)| Some(*id) == me) {
                    // The server already combined it with any flash still on (`grenades::combine`).
                    flash.hold = b.hold;
                    flash.fade = b.fade;
                    flash.alpha = b.alpha as f32 / 255.0;
                    flash.total_fade = flash.fade;
                }
            }
            _ => {}
        }
    }
    for (e, mut f, mut tf, mat) in &mut fireballs {
        f.0 += dt;
        let t = f.0 / (0.35 * f.1.sqrt());
        tf.scale = Vec3::splat((20.0 + 130.0 * t.min(1.0)) * f.1);
        if let Some(mut m) = materials.get_mut(&mat.0) {
            m.base_color.set_alpha(0.9 * (1.0 - t).max(0.0));
        }
        if t >= 1.0 {
            commands.entity(e).despawn();
        }
    }
    // Flashbang white-out: full for `hold` seconds, then fades out over `fade`.
    let a = if flash.hold > 0.0 {
        flash.hold -= dt;
        flash.alpha
    } else if flash.fade > 0.0 {
        flash.fade -= dt;
        flash.alpha * (flash.fade / flash.total_fade.max(0.01)).max(0.0)
    } else {
        flash.alpha = 0.0;
        0.0
    };
    overlay.set_if_neq(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, a)));
}

/// Server events: tracers, sounds, the kill feed, bullet holes and blood (`impacts`), player
/// animations (`players`); explosions and flashes go on to `grenade_effects`.
/// Every game event, for systems that only need to watch (the AI assistant).
#[derive(Message)]
pub struct GameNews(pub GameEvent);

#[allow(clippy::too_many_arguments)]
fn events(time: Res<Time>, mut w: ResMut<Walker>, mut feed: ResMut<KillFeed>, mut gizmos: Gizmos, mut hits: MessageWriter<Hit>, mut anims: MessageWriter<PlayerAnim>, mut round: MessageWriter<RoundNews>, mut news: MessageWriter<GameNews>) {
    let me = w.client.id();
    // Where a player is now (for sounds and the direction blood flies).
    let eye_of = |w: &Walker, id: u8| -> Option<[f32; 3]> {
        if Some(id) == me {
            return Some(w.client.me().eye());
        }
        w.client.others().iter().find(|o| o.id == id).map(|o| [o.origin[0], o.origin[1], o.origin[2] + 17.0])
    };
    for e in w.client.take_events() {
        news.write(GameNews(e.clone()));
        match e {
            GameEvent::Shot { shooter, weapon, silenced, from, to, holes } => {
                for (at, normal) in holes {
                    hits.write(Hit::Wall { at, normal });
                    w.cues.push(sound::Cue::Impact { at, normal });
                }
                if Some(shooter) != me {
                    w.tracers.push((from, to, 0.08));
                    w.cues.push(sound::Cue::Shot { weapon, silenced, at: Some(from) });
                    anims.write(PlayerAnim::Shot(shooter, silenced));
                }
            }
            GameEvent::Footstep { player, at, step } if Some(player) != me => {
                w.cues.push(sound::Cue::Footstep { feet: [at[0], at[1], at[2] - 36.0], who: Some(player), step });
            }
            GameEvent::Damage { attacker, victim, sound, group, at, .. } => {
                let victim_at = eye_of(&w, victim);
                w.cues.push(sound::Cue::Hit { sound, at: if Some(victim) == me { None } else { victim_at } });
                anims.write(PlayerAnim::Hurt(victim, group));
                // Knife hits have no exact point: their middle.
                let at = at.or_else(|| (sound != net::protocol::HitSound::Explosion).then_some(victim_at).flatten());
                if let (Some(at), Some(from)) = (at, eye_of(&w, attacker)) {
                    let d = [at[0] - from[0], at[1] - from[1], at[2] - from[2]];
                    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-3);
                    hits.write(Hit::Player { at, dir: d.map(|v| v / l), head: group == sim::hitbox::HitGroup::Head });
                }
            }
            GameEvent::GrenadeBounce { kind, at } => w.cues.push(sound::Cue::Bounce { kind, at }),
            GameEvent::Kill { killer, victim, weapon, headshot } => {
                let at = if Some(victim) == me { None } else { w.client.others().iter().find(|o| o.id == victim).map(|o| o.origin) };
                w.cues.push(sound::Cue::Death { at });
                anims.write(PlayerAnim::Died(victim));
                // Name and team as the feed shows them ("the bomb" for the world).
                let who = |id: u8| -> (String, Team) {
                    if id == net::protocol::WORLD {
                        return ("the bomb".to_string(), Team::Spectator);
                    }
                    w.client.scores.iter().find(|s| s.id == id).map(|s| (s.name.clone(), s.team)).unwrap_or_else(|| (format!("player {id}"), Team::Spectator))
                };
                let mine = me.is_some_and(|m| m == killer || m == victim);
                let by = (weapon.is_some() && killer != victim).then(|| who(killer));
                feed.push(Kill { killer: by, victim: who(victim), weapon, headshot, mine });
            }
            GameEvent::Explosion { at } => {
                w.cues.push(sound::Cue::Explosion { at });
                w.pending_fx.push(e);
            }
            GameEvent::Flash { at, .. } => {
                w.cues.push(sound::Cue::Flash { at });
                w.pending_fx.push(e);
            }
            GameEvent::BombPlanted { at, .. } => {
                w.cues.push(sound::Cue::At { file: "weapons/c4_plant.wav", at, far: false });
                round.write(RoundNews(e));
            }
            GameEvent::Defusing { .. } => {
                if let net::protocol::BombNet::Planted { at, .. } = w.client.bomb {
                    w.cues.push(sound::Cue::At { file: "weapons/c4_disarm.wav", at, far: false });
                }
            }
            GameEvent::BombDefused { .. } => {
                if let net::protocol::BombNet::Planted { at, .. } = w.client.bomb {
                    w.cues.push(sound::Cue::At { file: "weapons/c4_disarmed.wav", at, far: false });
                }
                round.write(RoundNews(e));
            }
            GameEvent::BombExploded { at } => {
                w.cues.push(sound::Cue::At { file: "weapons/c4_explode1.wav", at, far: true });
                w.pending_fx.push(e.clone());
                round.write(RoundNews(e));
            }
            GameEvent::Broke { material, at, .. } => {
                use sim::breakables::Material::*;
                let file = match material {
                    Glass | UnbreakableGlass => "debris/bustglass1.wav",
                    Wood => "debris/bustcrate1.wav",
                    Metal | Computer => "debris/bustmetal1.wav",
                    Flesh => "debris/bustflesh1.wav",
                    CeilingTile => "debris/bustceiling.wav",
                    Concrete | Rocks => "debris/bustconcrete1.wav",
                };
                w.cues.push(sound::Cue::At { file, at, far: false });
                hits.write(Hit::Debris { at, material });
            }
            GameEvent::RoundStart { .. } | GameEvent::RoundEnd { .. } | GameEvent::SidesSwapped | GameEvent::MatchOver { .. } | GameEvent::Surrendered { .. } | GameEvent::VotedOut { .. } | GameEvent::KillLimit { .. } => {
                round.write(RoundNews(e));
            }
            _ => {}
        }
    }
    let dt = time.delta_secs();
    for (from, to, left) in &mut w.tracers {
        gizmos.line(Vec3::from(map::to_y_up(*from)), Vec3::from(map::to_y_up(*to)), Color::srgba(1.0, 0.9, 0.5, 0.8));
        *left -= dt;
    }
    w.tracers.retain(|t| t.2 > 0.0);
}
