//! Sound from the install's WAV files: GoldSrc-style distance falloff, panned by
//! direction, random pitch like CS's gunshots.
use crate::game::{Frame, Walker};
use crate::map;
use crate::menu::Settings;
use bevy::audio::{PlaybackSettings, Volume};
use bevy::prelude::*;
use goldsrc::materials::Materials;
use sim::grenades::Kind;
use sim::weapons::Id;
use std::collections::HashMap;
use std::path::PathBuf;

/// GoldSrc attenuations: how fast a sound fades with distance (silent at 1000/attn units).
pub const ATTN_NORM: f32 = 0.8;
const ATTN_FAR: f32 = 0.3;

/// Something that should make a sound this frame.
#[derive(Clone, Debug)]
pub enum Cue {
    /// `at` None = our own (heard in the head, not positioned).
    Shot { weapon: Id, silenced: bool, at: Option<[f32; 3]> },
    /// `who` None = our own (heard in the head).
    Footstep { feet: [f32; 3], who: Option<u8>, step: sim::pmove::Step },
    Hit { sound: net::protocol::HitSound, at: Option<[f32; 3]> },
    Death { at: Option<[f32; 3]> },
    Explosion { at: [f32; 3] },
    Flash { at: [f32; 3] },
    Smoke { at: [f32; 3] },
    Bounce { kind: Kind, at: [f32; 3] },
    DryFire { pistol: bool },
    Swing,
    Zoom,
    /// A sound file named by a view model animation event (reloads, draws, pins).
    File(String),
    /// A sound file played somewhere in the world; `far`: carries across the map (the bomb).
    At { file: &'static str, at: [f32; 3], far: bool },
}

#[derive(Resource)]
pub struct Sounds {
    dir: PathBuf,
    cache: HashMap<String, Option<Handle<AudioSource>>>,
    materials: Materials,
    /// Cheap per-sound randomness (pitch, variant picks); doesn't need to be deterministic.
    rng: u64,
    /// Whose next footstep is the left foot (None: ours).
    left_foot: HashMap<Option<u8>, bool>,
}

impl Sounds {
    pub fn new(game_dir: PathBuf) -> Sounds {
        let materials = std::fs::read_to_string(game_dir.join("cstrike/sound/materials.txt")).map(|t| Materials::parse(&t)).unwrap_or_default();
        Sounds { dir: game_dir.join("cstrike/sound"), cache: HashMap::new(), materials, rng: 0x2545_F491_4F6C_DD1D, left_foot: HashMap::new() }
    }

    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 10_000) as f32 / 10_000.0
    }

    fn pick(&mut self, options: &[&str]) -> String {
        options[((self.rand() * options.len() as f32) as usize).min(options.len() - 1)].to_string()
    }

    fn get(&mut self, file: &str, assets: &mut Assets<AudioSource>) -> Option<Handle<AudioSource>> {
        if let Some(h) = self.cache.get(file) {
            return h.clone();
        }
        let h = std::fs::read(self.dir.join(file)).ok().map(|bytes| assets.add(AudioSource { bytes: bytes.into() }));
        if h.is_none() {
            warn!("missing sound {file}");
        }
        self.cache.insert(file.to_string(), h.clone());
        h
    }
}

/// The firing sound for a weapon (CS picks between variants where there are two; volume 1,
/// attenuation `fire_attenuation`, pitch 94..109%: `EV_Fire*` in client.so).
fn fire_sound(id: Id, silenced: bool) -> &'static [&'static str] {
    use Id::*;
    match id {
        Ak47 => &["weapons/ak47-1.wav", "weapons/ak47-2.wav"],
        Aug => &["weapons/aug-1.wav"],
        Awp => &["weapons/awp1.wav"],
        Deagle => &["weapons/deagle-1.wav", "weapons/deagle-2.wav"],
        Elite => &["weapons/elite_fire.wav"],
        FiveSeven => &["weapons/fiveseven-1.wav"],
        Famas => &["weapons/famas-1.wav", "weapons/famas-2.wav"],
        G3sg1 => &["weapons/g3sg1-1.wav"],
        Galil => &["weapons/galil-1.wav", "weapons/galil-2.wav"],
        Glock => &["weapons/glock18-1.wav", "weapons/glock18-2.wav"],
        M249 => &["weapons/m249-1.wav", "weapons/m249-2.wav"],
        M3 => &["weapons/m3-1.wav"],
        M4a1 if silenced => &["weapons/m4a1-1.wav"],
        M4a1 => &["weapons/m4a1_unsil-1.wav", "weapons/m4a1_unsil-2.wav"],
        Mac10 => &["weapons/mac10-1.wav"],
        Mp5 => &["weapons/mp5-1.wav", "weapons/mp5-2.wav"], // EV_FireMP5N
        P228 => &["weapons/p228-1.wav"],
        P90 => &["weapons/p90-1.wav"],
        Scout => &["weapons/scout_fire-1.wav"],
        Sg550 => &["weapons/sg550-1.wav"],
        Sg552 => &["weapons/sg552-1.wav", "weapons/sg552-2.wav"],
        Tmp => &["weapons/tmp-1.wav", "weapons/tmp-2.wav"],
        Ump45 => &["weapons/ump45-1.wav"],
        Usp if silenced => &["weapons/usp1.wav", "weapons/usp2.wav"],
        Usp => &["weapons/usp_unsil-1.wav"],
        Xm1014 => &["weapons/xm1014-1.wav"],
        _ => &[],
    }
}

/// Footstep sound set (`PM_PlayStepSound`): files 1 and 3 are the right foot, 2 and 4 the left.
fn step_set(kind: sim::pmove::StepKind, material: char) -> &'static str {
    use sim::pmove::StepKind::*;
    match (kind, material) {
        (Ladder, _) => "ladder",
        (Wade, _) => "wade",
        (Slosh, _) | (Surface, 'S') => "slosh",
        (Surface, 'M') => "metal",
        (Surface, 'D') => "dirt",
        (Surface, 'V') => "duct",
        (Surface, 'G') => "grate",
        (Surface, 'T') => "tile",
        (Surface, 'N') => "snow",
        _ => "step",
    }
}

struct Play {
    file: String,
    /// None = in your head (own weapon, own footsteps).
    at: Option<[f32; 3]>,
    volume: f32,
    attn: f32,
    pitch: f32,
}

/// Play this frame's cues.
fn play_cues(mut commands: Commands, mut w: ResMut<Walker>, settings: Res<Settings>, mut sounds: ResMut<Sounds>, mut assets: ResMut<Assets<AudioSource>>) {
    let cues = std::mem::take(&mut w.cues);
    if cues.is_empty() {
        return;
    }
    let ear = w.client.me().eye();
    for cue in cues {
        let mut plays: Vec<Play> = Vec::new();
        // Gunshot pitch varies a little, like CS (94 + 0..15 %).
        let gun_pitch = 0.94 + (sounds.rand() * 16.0).floor() / 100.0;
        match cue {
            Cue::Shot { weapon, silenced, at } => {
                let options = fire_sound(weapon, silenced);
                if !options.is_empty() {
                    let file = sounds.pick(options);
                    plays.push(Play { file, at, volume: 1.0, attn: sim::weapons::data::fire_attenuation(weapon, silenced), pitch: gun_pitch });
                }
            }
            Cue::Footstep { feet, who, step } => {
                // The surface under the feet decides the sound and its volume.
                let below = [feet[0], feet[1], feet[2] - 64.0];
                let material = w.client.world.texture_at(feet, below).map(|t| sounds.materials.kind(t)).unwrap_or('C');
                let left = sounds.left_foot.entry(who).or_insert(false);
                *left = !*left;
                let left = *left as u32;
                let n = (sounds.rand() < 0.5) as u32 * 2 + 1 + left;
                let file = format!("player/pl_{}{n}.wav", step_set(step.kind, material));
                plays.push(Play { file, at: if who.is_none() { None } else { Some(feet) }, volume: step.loudness(material), attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Hit { sound, at } => {
                use net::protocol::HitSound::*;
                let options: &[&str] = match sound {
                    Flesh => &["player/bhit_flesh-1.wav", "player/bhit_flesh-2.wav", "player/bhit_flesh-3.wav"],
                    Kevlar => &["player/bhit_kevlar-1.wav"],
                    Helmet => &["player/bhit_helmet-1.wav"],
                    Headshot => &["player/headshot1.wav", "player/headshot2.wav", "player/headshot3.wav"],
                    KnifeSlash => &["weapons/knife_hit1.wav", "weapons/knife_hit2.wav", "weapons/knife_hit3.wav", "weapons/knife_hit4.wav"],
                    KnifeStab => &["weapons/knife_stab.wav"],
                    Explosion => &["player/pl_pain2.wav", "player/pl_pain4.wav", "player/pl_pain5.wav", "player/pl_pain6.wav", "player/pl_pain7.wav"],
                };
                let file = sounds.pick(options);
                plays.push(Play { file, at, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Death { at } => {
                let file = sounds.pick(&["player/die1.wav", "player/die2.wav", "player/die3.wav", "player/death6.wav"]);
                plays.push(Play { file, at, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Explosion { at } => {
                let file = sounds.pick(&["weapons/hegrenade-1.wav", "weapons/hegrenade-2.wav"]);
                plays.push(Play { file, at: Some(at), volume: 1.0, attn: ATTN_FAR, pitch: 1.0 });
            }
            Cue::Flash { at } => {
                let file = sounds.pick(&["weapons/flashbang-1.wav", "weapons/flashbang-2.wav"]);
                plays.push(Play { file, at: Some(at), volume: 0.9, attn: ATTN_FAR, pitch: 1.0 });
            }
            Cue::Smoke { at } => plays.push(Play { file: "weapons/sg_explode.wav".into(), at: Some(at), volume: 1.0, attn: ATTN_NORM, pitch: 1.0 }),
            Cue::Bounce { kind, at } => {
                let file = if kind == Kind::He { "weapons/he_bounce-1.wav".into() } else { sounds.pick(&["weapons/grenade_hit1.wav", "weapons/grenade_hit2.wav", "weapons/grenade_hit3.wav"]) };
                plays.push(Play { file, at: Some(at), volume: 0.5, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::DryFire { pistol } => {
                plays.push(Play { file: if pistol { "weapons/dryfire_pistol.wav" } else { "weapons/dryfire_rifle.wav" }.into(), at: None, volume: 0.8, attn: ATTN_NORM, pitch: 1.0 })
            }
            Cue::Swing => {
                // The swing itself is a whoosh, slash or stab (CS plays the same miss sounds for
                // both); what it hit sounds on its own, from the server (`Cue::Hit`).
                let file = sounds.pick(&["weapons/knife_slash1.wav", "weapons/knife_slash2.wav"]);
                plays.push(Play { file, at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Zoom => plays.push(Play { file: "weapons/zoom.wav".into(), at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 }),
            Cue::File(file) => plays.push(Play { file, at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 }),
            Cue::At { file, at, far } => plays.push(Play { file: file.into(), at: Some(at), volume: 1.0, attn: if far { ATTN_FAR } else { ATTN_NORM }, pitch: 1.0 }),
        }
        for p in plays {
            let Some(handle) = sounds.get(&p.file, &mut assets) else { continue };
            let mut play = PlaybackSettings::DESPAWN.with_speed(p.pitch);
            match p.at {
                Some(at) => {
                    // GoldSrc falloff: full at the source, silent at 1000 / attn units.
                    let d = ((at[0] - ear[0]).powi(2) + (at[1] - ear[1]).powi(2) + (at[2] - ear[2]).powi(2)).sqrt();
                    let gain = settings.volume * p.volume * (1.0 - d * p.attn / 1000.0);
                    if gain <= 0.01 {
                        continue;
                    }
                    play = play.with_volume(Volume::Linear(gain)).with_spatial(true);
                    commands.spawn((AudioPlayer(handle), play, Transform::from_translation(Vec3::from(map::to_y_up(at)))));
                }
                None => {
                    play = play.with_volume(Volume::Linear(settings.volume * p.volume));
                    commands.spawn((AudioPlayer(handle), play));
                }
            }
        }
    }
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, play_cues.in_set(Frame::Sound));
    }
}
