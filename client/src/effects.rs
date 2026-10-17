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
use crate::{map, models, sound};
use bevy::{
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
}

/// Flashbang: seconds of full white left, seconds of fade left (and total), strength.
#[derive(Resource, Default)]
struct Flash {
    hold: f32,
    fade: f32,
    total_fade: f32,
    alpha: f32,
}

fn load(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<crate::studio::StudioMaterial>>, mut images: ResMut<Assets<Image>>) {
    let grenades = ["w_hegrenade", "w_flashbang", "w_smokegrenade"]
        .iter()
        .map(|name| {
            crate::gamefiles::paths(&format!("models/{name}.mdl"))
                .iter()
                .find_map(|p| goldsrc::mdl::Model::load(p).ok())
                .map(|m| model_parts(&m, 0, &mut meshes, &mut materials, &mut images))
                .unwrap_or_default()
        })
        .collect();
    commands.insert_resource(Effects {
        grenades,
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
        // over the menus too: clicks must go through it (it blocked the buy menu)
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
}

/// Meshes and materials for one posed studio model.
fn model_parts(model: &goldsrc::mdl::Model, seq: usize, meshes: &mut Assets<Mesh>, materials: &mut Assets<crate::studio::StudioMaterial>, images: &mut Assets<Image>) -> Parts {
    let textures: Vec<Handle<crate::studio::StudioMaterial>> = model
        .textures()
        .into_iter()
        .map(|t| {
            let mut rgba = t.rgba;
            crate::level::tex_gamma(&mut rgba);
            materials.add(crate::studio::material(images.add(rgba_image(t.width, t.height, rgba, false, TextureFormat::Rgba8UnormSrgb))))
        })
        .collect();
    model
        .posed_mesh(&model.pose(seq, 0, 0.5))
        .into_iter()
        .filter(|p| !p.indices.is_empty())
        .map(|p| (meshes.add(models::part_mesh(&p, map::to_y_up, false)), textures.get(p.texture).cloned().unwrap_or_default()))
        .collect()
}

/// Spawn a model's parts under one parent at `at` (GoldSrc coordinates).
fn spawn_parts(commands: &mut Commands, marker: impl Bundle, at: [f32; 3], parts: &Parts, materials: &mut Assets<crate::studio::StudioMaterial>) {
    let parent = commands.spawn((marker, Transform::from_translation(Vec3::from(map::to_y_up(at))), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
    models::spawn_lit(commands, parent, parts, materials);
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
    mut materials: ResMut<Assets<crate::studio::StudioMaterial>>,
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
        spawn_parts(&mut commands, GroundGunView, at, p, &mut materials);
    }
}



#[derive(Component)]
struct FlashOverlay;

/// Grenades in flight and the flashbang white-out (explosions: `explosions.rs`, smoke: `smoke.rs`).
#[allow(clippy::too_many_arguments)]
fn grenade_effects(
    mut commands: Commands,
    time: Res<Time>,
    mut w: ResMut<Walker>,
    mut flash: ResMut<Flash>,
    fx: Res<Effects>,
    mut studio: ResMut<Assets<crate::studio::StudioMaterial>>,
    views: Query<Entity, With<GrenadeView>>,
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
        spawn_parts(&mut commands, GrenadeView, at, &fx.grenades[i], &mut studio);
    }
    // Explosions and flashes from server events (sorted out in `events`).
    let me = w.client.id();
    for e in std::mem::take(&mut w.pending_fx) {
        match e {
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
fn events(time: Res<Time>, mut w: ResMut<Walker>, mut feed: ResMut<KillFeed>, mut gizmos: Gizmos, mut hits: MessageWriter<Hit>, mut anims: MessageWriter<PlayerAnim>, mut round: MessageWriter<RoundNews>, mut news: MessageWriter<GameNews>, shot_assets: Option<Res<crate::game::ShotAssets>>) {
    let me = w.client.id();
    let predicting = shot_assets.is_some();
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
                // Our own holes were drawn when we fired (game.rs), if we could trace them.
                let predicted = Some(shooter) == me && predicting;
                for (at, normal) in holes.into_iter().filter(|_| !predicted) {
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
                hits.write(Hit::Scorch { at });
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
            // The gibs come from gibs.rs.
            GameEvent::Broke { material, at, .. } => w.cues.push(sound::Cue::Break { material, at }),
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
