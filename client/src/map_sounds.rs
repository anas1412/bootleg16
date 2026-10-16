//! Sounds the map makes, as GoldSrc plays them: `ambient_generic` (looping ambience, and
//! one-shots fired by name: train's jet fly-by, birds, dogs), doors moving and stopping
//! (`movesnd`/`stopsnd`), buttons, and `env_spark` (crackles and sparks every so often).
use crate::effects::GameNews;
use crate::game::{Frame, Walker};
use crate::impacts::Hit;
use crate::map;
use crate::menu::{AppState, GameConfig, Settings};
use crate::sound::{Sounds, ATTN_NORM};
use bevy::audio::{PlaybackSettings, Volume};
use bevy::prelude::*;
use net::protocol::GameEvent;
use sim::doors::DoorState;

pub struct MapSoundsPlugin;

impl Plugin for MapSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), load).add_systems(Update, (ambience, doors, sparks).in_set(Frame::Sound).run_if(resource_exists::<MapSounds>));
    }
}

// GoldSrc attenuations (silent at 1000 / attn units; 0 = heard everywhere at full volume).
const ATTN_NONE: f32 = 0.0;
const ATTN_IDLE: f32 = 2.0;
const ATTN_STATIC: f32 = 1.25;

// `ambient_generic` spawnflags.
const EVERYWHERE: u32 = 1;
const SMALL_RADIUS: u32 = 2;
const MEDIUM_RADIUS: u32 = 4;
const LARGE_RADIUS: u32 = 8;
const START_SILENT: u32 = 16;
const NOT_LOOPING: u32 = 32;

/// One `ambient_generic`.
struct Ambient {
    at: [f32; 3],
    file: String,
    /// `health` / 10.
    volume: f32,
    attn: f32,
    looping: bool,
    name: Option<String>,
    /// A looping sound that's on (its player entity once spawned).
    on: bool,
    playing: Option<Entity>,
}

/// One `env_spark`: when it next sparks (None until the map's first frame).
struct Spark {
    at: [f32; 3],
    max_delay: f32,
    next: Option<f32>,
}

#[derive(Resource)]
struct MapSounds {
    ambients: Vec<Ambient>,
    sparks: Vec<Spark>,
    /// Each door's state last frame, and its move sound while it plays.
    doors: Vec<(DoorState, Option<Entity>)>,
    rng: u64,
}

impl MapSounds {
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10_000) as f32 / 10_000.0
    }
}

/// A looping (or moving) sound in the world: where it is, how loud, how fast it fades.
#[derive(Component)]
struct Placed {
    at: [f32; 3],
    volume: f32,
    attn: f32,
}

fn load(mut commands: Commands, config: Res<GameConfig>) {
    let path = crate::game_dir().join("cstrike/maps").join(format!("{}.bsp", config.map));
    let Ok(bsp) = goldsrc::bsp::Bsp::load(&path) else { return };
    let num = |e: &std::collections::HashMap<String, String>, k: &str| e.get(k).and_then(|v| v.parse::<f32>().ok());
    let origin = |e: &std::collections::HashMap<String, String>| {
        let v: Vec<f32> = e.get("origin").map(|o| o.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)]
    };
    let mut ambients = Vec::new();
    let mut sparks = Vec::new();
    for e in bsp.entity_list() {
        match e.get("classname").map(String::as_str) {
            Some("ambient_generic") => {
                let Some(file) = e.get("message").filter(|m| m.ends_with(".wav")) else { continue };
                let flags = num(&e, "spawnflags").unwrap_or(0.0) as u32;
                let attn = if flags & EVERYWHERE != 0 {
                    ATTN_NONE
                } else if flags & SMALL_RADIUS != 0 {
                    ATTN_IDLE
                } else if flags & MEDIUM_RADIUS != 0 {
                    ATTN_STATIC
                } else if flags & LARGE_RADIUS != 0 {
                    ATTN_NORM
                } else {
                    ATTN_STATIC
                };
                let looping = flags & NOT_LOOPING == 0;
                ambients.push(Ambient {
                    at: origin(&e),
                    file: file.clone(),
                    volume: (num(&e, "health").unwrap_or(10.0) / 10.0).clamp(0.0, 1.0),
                    attn,
                    looping,
                    name: e.get("targetname").cloned(),
                    on: looping && flags & START_SILENT == 0,
                    playing: None,
                });
            }
            // Sparks every 0.1 s plus up to `MaxDelay` (1.5 when unset), from 0.1-1.6 s in.
            Some("env_spark") => sparks.push(Spark { at: origin(&e), max_delay: num(&e, "MaxDelay").filter(|d| *d > 0.0).unwrap_or(1.5), next: None }),
            _ => {}
        }
    }
    commands.insert_resource(MapSounds { ambients, sparks, doors: Vec::new(), rng: 0x9E37_79B9_7F4A_7C15 });
}

/// Distance falloff, GoldSrc's: full at the source, silent at 1000 / attn.
fn gain(at: [f32; 3], ear: [f32; 3], attn: f32) -> f32 {
    let d = ((at[0] - ear[0]).powi(2) + (at[1] - ear[1]).powi(2) + (at[2] - ear[2]).powi(2)).sqrt();
    (1.0 - d * attn / 1000.0).max(0.0)
}

fn spawn_loop(commands: &mut Commands, sounds: &mut Sounds, assets: &mut Assets<AudioSource>, file: &str, at: [f32; 3], volume: f32, attn: f32) -> Option<Entity> {
    let handle = sounds.get(file, assets)?;
    let spatial = attn > 0.0;
    let play = PlaybackSettings::LOOP.with_volume(Volume::Linear(0.0)).with_spatial(spatial);
    Some(commands.spawn((AudioPlayer(handle), play, Placed { at, volume, attn }, Transform::from_translation(Vec3::from(map::to_y_up(at))), DespawnOnExit(AppState::InGame))).id())
}

/// Looping ambience on and off, one-shots when their name fires; every placed sound's volume
/// follows the distance to you.
#[allow(clippy::too_many_arguments)]
fn ambience(
    mut commands: Commands,
    mut ms: ResMut<MapSounds>,
    mut sounds: ResMut<Sounds>,
    mut assets: ResMut<Assets<AudioSource>>,
    mut news: MessageReader<GameNews>,
    w: Res<Walker>,
    settings: Res<Settings>,
    mut placed: Query<(&Placed, &mut Transform, Option<&mut AudioSink>, Option<&mut SpatialAudioSink>)>,
) {
    let ear = w.client.me().eye();
    for GameNews(e) in news.read() {
        let GameEvent::Fired { name } = e else { continue };
        for a in ms.ambients.iter_mut().filter(|a| a.name.as_deref() == Some(name.as_str())) {
            if a.looping {
                a.on = !a.on; // a named looping sound toggles
            } else {
                let g = if a.attn > 0.0 { gain(a.at, ear, a.attn) } else { 1.0 };
                let v = settings.volume * a.volume * g;
                if let (true, Some(h)) = (v > 0.01, sounds.get(&a.file, &mut assets)) {
                    let play = PlaybackSettings::DESPAWN.with_volume(Volume::Linear(v)).with_spatial(a.attn > 0.0);
                    commands.spawn((AudioPlayer(h), play, Transform::from_translation(Vec3::from(map::to_y_up(a.at)))));
                }
            }
        }
    }
    for a in ms.ambients.iter_mut() {
        match (a.on, a.playing) {
            (true, None) => a.playing = spawn_loop(&mut commands, &mut sounds, &mut assets, &a.file, a.at, a.volume, a.attn),
            (false, Some(e)) => {
                commands.entity(e).try_despawn();
                a.playing = None;
            }
            _ => {}
        }
    }
    for (p, mut tf, sink, spatial) in &mut placed {
        tf.translation = Vec3::from(map::to_y_up(p.at));
        let v = Volume::Linear(settings.volume * p.volume * if p.attn > 0.0 { gain(p.at, ear, p.attn) } else { 1.0 });
        if let Some(mut s) = sink {
            s.set_volume(v);
        }
        if let Some(mut s) = spatial {
            s.set_volume(v);
        }
    }
}

/// Doors: the move sound loops while it moves, the stop sound plays when it gets there
/// (`CHAN_STATIC`, volume 1, `ATTN_NORM`). A button clicks when pressed.
#[allow(clippy::too_many_arguments)]
fn doors(mut commands: Commands, mut ms: ResMut<MapSounds>, mut sounds: ResMut<Sounds>, mut assets: ResMut<Assets<AudioSource>>, w: Res<Walker>, settings: Res<Settings>, mut placed: Query<&mut Placed>) {
    let ear = w.client.me().eye();
    let all = &w.client.world.doors.doors;
    if ms.doors.len() != all.len() {
        ms.doors = all.iter().map(|d| (d.state, None)).collect();
    }
    for (d, (last, playing)) in all.iter().zip(ms.doors.iter_mut()) {
        let (lo, hi) = d.bounds();
        let at = [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let moving = |s: DoorState| matches!(s, DoorState::Opening | DoorState::Closing);
        if moving(d.state) && !moving(*last) {
            if d.is_button() {
                if d.state == DoorState::Opening && d.move_sound > 0 {
                    one_shot(&mut commands, &mut sounds, &mut assets, &format!("buttons/button{}.wav", d.move_sound), at, ear, settings.volume);
                }
            } else if d.move_sound > 0 {
                *playing = spawn_loop(&mut commands, &mut sounds, &mut assets, &format!("doors/doormove{}.wav", d.move_sound), at, 1.0, ATTN_NORM);
            }
        }
        if !moving(d.state) && moving(*last) {
            if let Some(e) = playing.take() {
                commands.entity(e).try_despawn();
            }
            if d.stop_sound > 0 {
                one_shot(&mut commands, &mut sounds, &mut assets, &format!("doors/doorstop{}.wav", d.stop_sound), at, ear, settings.volume);
            }
        }
        if let Some(mut p) = playing.and_then(|e| placed.get_mut(e).ok()) {
            p.at = at; // the sound moves with the door
        }
        *last = d.state;
    }
}

fn one_shot(commands: &mut Commands, sounds: &mut Sounds, assets: &mut Assets<AudioSource>, file: &str, at: [f32; 3], ear: [f32; 3], volume: f32) {
    let v = volume * gain(at, ear, ATTN_NORM);
    if let (true, Some(h)) = (v > 0.01, sounds.get(file, assets)) {
        commands.spawn((AudioPlayer(h), PlaybackSettings::DESPAWN.with_volume(Volume::Linear(v)).with_spatial(true), Transform::from_translation(Vec3::from(map::to_y_up(at)))));
    }
}

/// `env_spark` (`DoSpark`): sparks and a crackle at 0.25-0.75 x 0.4 volume, then again in
/// 0.1 s plus up to `MaxDelay`.
#[allow(clippy::too_many_arguments)]
fn sparks(mut commands: Commands, mut ms: ResMut<MapSounds>, mut sounds: ResMut<Sounds>, mut assets: ResMut<Assets<AudioSource>>, w: Res<Walker>, settings: Res<Settings>, time: Res<Time>, mut hits: MessageWriter<Hit>) {
    let now = time.elapsed_secs();
    let ear = w.client.me().eye();
    for i in 0..ms.sparks.len() {
        let r = ms.rand();
        let next = *ms.sparks[i].next.get_or_insert(now + 0.1 + r * 1.5); // first: 0.1-1.6 s in
        if now < next {
            continue;
        }
        let (r1, r2, r3) = (ms.rand(), ms.rand(), ms.rand());
        let s = &mut ms.sparks[i];
        s.next = Some(now + 0.1 + r1 * s.max_delay);
        hits.write(Hit::Sparks { at: s.at });
        let file = format!("buttons/spark{}.wav", 1 + (r2 * 6.0).min(5.0) as u32);
        one_shot(&mut commands, &mut sounds, &mut assets, &file, s.at, ear, settings.volume * (0.25 + r3 * 0.5) * 0.4);
    }
}
