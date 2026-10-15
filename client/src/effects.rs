//! Server events turned into things you see and hear: tracers, the kill feed, grenades in
//! flight, explosions, smoke clouds, the flashbang white-out and guns on the ground.
use crate::game::{Frame, Walker};
use crate::level::rgba_image;
use crate::menu::AppState;
use crate::impacts::Hit;
use crate::models::Parts;
use crate::players::PlayerAnim;
use crate::{game_dir, map, models, sound};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::TextureFormat,
};
use net::protocol::GameEvent;

pub struct EffectsPlugin;

impl Plugin for EffectsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Flash>()
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
    smoke_puff: Handle<StandardMaterial>,
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
        // Double-sided: you can stand inside a smoke cloud.
        smoke_puff: materials.add(StandardMaterial { base_color: Color::srgba(0.55, 0.55, 0.55, 0.85), unlit: true, alpha_mode: AlphaMode::Blend, cull_mode: None, double_sided: true, ..default() }),
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

/// An HE fireball: age in seconds.
#[derive(Component)]
struct Fireball(f32);

/// A smoke cloud's puffs, keyed by where the cloud is.
#[derive(Component)]
struct SmokeCloud([i32; 3]);

#[derive(Component)]
struct FlashOverlay;

/// Grenades in flight, fireballs, smoke clouds, and the flashbang white-out.
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
    clouds: Query<(Entity, &SmokeCloud)>,
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
            GameEvent::Explosion { at } => {
                let mat = materials.add(StandardMaterial { base_color: Color::srgba(1.0, 0.6, 0.15, 0.9), unlit: true, alpha_mode: AlphaMode::Blend, ..default() });
                commands.spawn((
                    Fireball(0.0),
                    Mesh3d(fx.sphere.clone()),
                    MeshMaterial3d(mat),
                    Transform::from_translation(Vec3::from(map::to_y_up(at))).with_scale(Vec3::splat(20.0)),
                    DespawnOnExit(AppState::InGame),
                ));
            }
            GameEvent::Flash { blinded, .. } => {
                if let Some((_, b)) = blinded.iter().find(|(id, _)| Some(*id) == me) {
                    // A new flash never shortens one that's still on (simplified "max" rule).
                    flash.hold = flash.hold.max(b.hold);
                    flash.fade = flash.fade.max(b.fade);
                    flash.alpha = flash.alpha.max(b.alpha as f32 / 255.0);
                    flash.total_fade = flash.fade;
                }
            }
            _ => {}
        }
    }
    for (e, mut f, mut tf, mat) in &mut fireballs {
        f.0 += dt;
        let t = f.0 / 0.35;
        tf.scale = Vec3::splat(20.0 + 130.0 * t.min(1.0));
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
    // Smoke clouds: a cluster of grey puffs.
    let key = |at: [f32; 3]| at.map(|v| v.round() as i32);
    let live: Vec<_> = w.client.smokes.iter().map(|s| (key(s.at), *s)).collect();
    for (e, c) in &clouds {
        if !live.iter().any(|(k, _)| *k == c.0) {
            commands.entity(e).despawn();
        }
    }
    for (k, smoke) in &live {
        if clouds.iter().any(|(_, c)| c.0 == *k) {
            continue;
        }
        if smoke.left > sim::grenades::SMOKE_SECONDS as f32 - 1.0 {
            w.cues.push(sound::Cue::Smoke { at: smoke.at }); // just popped (not one we joined late)
        }
        let centre = Vec3::from(map::to_y_up(smoke.at));
        let parent = commands.spawn((SmokeCloud(*k), Transform::from_translation(centre), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for i in 0..14 {
            let a = i as f32 * 2.4;
            let r = 30.0 + (i % 4) as f32 * 22.0;
            let offset = Vec3::new(a.cos() * r, 20.0 + (i % 3) as f32 * 35.0, a.sin() * r);
            let child = commands.spawn((Mesh3d(fx.sphere.clone()), MeshMaterial3d(fx.smoke_puff.clone()), Transform::from_translation(offset).with_scale(Vec3::splat(70.0)))).id();
            commands.entity(parent).add_child(child);
        }
    }
}

/// Server events: tracers, sounds, the kill feed, bullet holes and blood (`impacts`), player
/// animations (`players`); explosions and flashes go on to `grenade_effects`.
fn events(time: Res<Time>, mut w: ResMut<Walker>, mut gizmos: Gizmos, mut hits: MessageWriter<Hit>, mut anims: MessageWriter<PlayerAnim>) {
    let me = w.client.id();
    // Where a player is now (for sounds and the direction blood flies).
    let eye_of = |w: &Walker, id: u8| -> Option<[f32; 3]> {
        if Some(id) == me {
            return Some(w.client.me().eye());
        }
        w.client.others().iter().find(|o| o.id == id).map(|o| [o.origin[0], o.origin[1], o.origin[2] + 17.0])
    };
    for e in w.client.take_events() {
        match e {
            GameEvent::Shot { shooter, weapon, silenced, from, to, holes } => {
                for (at, normal) in holes {
                    hits.write(Hit::Wall { at, normal });
                }
                if Some(shooter) != me {
                    w.tracers.push((from, to, 0.08));
                    w.cues.push(sound::Cue::Shot { weapon, silenced, at: Some(from) });
                    anims.write(PlayerAnim::Shot(shooter));
                }
            }
            GameEvent::Footstep { player, at } if Some(player) != me => {
                w.cues.push(sound::Cue::Footstep { feet: [at[0], at[1], at[2] - 36.0], own: false, ladder: false, water: false });
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
                let name = |id: u8| {
                    if Some(id) == me {
                        return "you".to_string();
                    }
                    w.client.scores.iter().find(|s| s.id == id).map(|s| s.name.clone()).unwrap_or_else(|| format!("player {id}"))
                };
                let line = match weapon {
                    None => format!("{} fell", name(victim)),
                    Some(wp) => format!("{} [{}{}] {}", name(killer), sim::weapons::def(wp).name, if headshot { " HS" } else { "" }, name(victim)),
                };
                w.feed.push((line, 6.0));
            }
            GameEvent::Explosion { at } => {
                w.cues.push(sound::Cue::Explosion { at });
                w.pending_fx.push(e);
            }
            GameEvent::Flash { at, .. } => {
                w.cues.push(sound::Cue::Flash { at });
                w.pending_fx.push(e);
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
    for (_, left) in &mut w.feed {
        *left -= dt;
    }
    w.feed.retain(|f| f.1 > 0.0);
    let n = w.feed.len();
    if n > 5 {
        w.feed.drain(..n - 5);
    }
}
