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
const ATTN_QUIET: f32 = 1.6;

/// Something that should make a sound this frame.
#[derive(Clone, Debug)]
pub enum Cue {
    /// `at` None = our own (heard in the head, not positioned).
    Shot { weapon: Id, silenced: bool, at: Option<[f32; 3]> },
    Footstep { feet: [f32; 3], own: bool, ladder: bool, water: bool },
    Hit { sound: net::protocol::HitSound, at: Option<[f32; 3]> },
    Death { at: Option<[f32; 3]> },
    Explosion { at: [f32; 3] },
    Flash { at: [f32; 3] },
    Smoke { at: [f32; 3] },
    Bounce { kind: Kind, at: [f32; 3] },
    DryFire { pistol: bool },
    Swing { stab: bool },
    Zoom,
    /// A sound file named by a view model animation event (reloads, draws, pins).
    File(String),
}

#[derive(Resource)]
pub struct Sounds {
    dir: PathBuf,
    cache: HashMap<String, Option<Handle<AudioSource>>>,
    materials: Materials,
    /// Cheap per-sound randomness (pitch, variant picks); doesn't need to be deterministic.
    rng: u64,
}

impl Sounds {
    pub fn new(game_dir: PathBuf) -> Sounds {
        let materials = std::fs::read_to_string(game_dir.join("cstrike/sound/materials.txt")).map(|t| Materials::parse(&t)).unwrap_or_default();
        Sounds { dir: game_dir.join("cstrike/sound"), cache: HashMap::new(), materials, rng: 0x2545_F491_4F6C_DD1D }
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

/// The firing sound for a weapon (CS picks between variants where there are two).
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
        Mp5 => &["weapons/mp5-1.wav", "weapons/mp5-2.wav"],
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

/// Footstep sounds by surface material (`materials.txt` letters).
fn step_sounds(material: char) -> &'static [&'static str] {
    match material {
        'M' => &["player/pl_metal1.wav", "player/pl_metal2.wav", "player/pl_metal3.wav", "player/pl_metal4.wav"],
        'D' => &["player/pl_dirt1.wav", "player/pl_dirt2.wav", "player/pl_dirt3.wav", "player/pl_dirt4.wav"],
        'V' => &["player/pl_duct1.wav", "player/pl_duct2.wav", "player/pl_duct3.wav", "player/pl_duct4.wav"],
        'G' => &["player/pl_grate1.wav", "player/pl_grate2.wav", "player/pl_grate3.wav", "player/pl_grate4.wav"],
        'T' => &["player/pl_tile1.wav", "player/pl_tile2.wav", "player/pl_tile3.wav", "player/pl_tile4.wav"],
        'S' => &["player/pl_slosh1.wav", "player/pl_slosh2.wav", "player/pl_slosh3.wav", "player/pl_slosh4.wav"],
        'N' => &["player/pl_snow1.wav", "player/pl_snow2.wav", "player/pl_snow3.wav", "player/pl_snow4.wav"],
        _ => &["player/pl_step1.wav", "player/pl_step2.wav", "player/pl_step3.wav", "player/pl_step4.wav"],
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
        // Gunshot pitch varies a little, like CS (94..110%).
        let gun_pitch = 0.94 + sounds.rand() * 0.16;
        match cue {
            Cue::Shot { weapon, silenced, at } => {
                let options = fire_sound(weapon, silenced);
                if !options.is_empty() {
                    let file = sounds.pick(options);
                    plays.push(Play { file, at, volume: if silenced { 0.7 } else { 1.0 }, attn: if silenced { ATTN_QUIET } else { ATTN_NORM }, pitch: gun_pitch });
                }
            }
            Cue::Footstep { feet, own, ladder, water } => {
                let options: &[&str] = if ladder {
                    &["player/pl_ladder1.wav", "player/pl_ladder2.wav", "player/pl_ladder3.wav", "player/pl_ladder4.wav"]
                } else if water {
                    &["player/pl_wade1.wav", "player/pl_wade2.wav", "player/pl_wade3.wav", "player/pl_wade4.wav"]
                } else {
                    // The surface under the feet decides the sound.
                    let below = [feet[0], feet[1], feet[2] - 64.0];
                    let material = w.client.world.texture_at(feet, below).map(|t| sounds.materials.kind(t)).unwrap_or('C');
                    step_sounds(material)
                };
                let file = sounds.pick(options);
                plays.push(Play { file, at: if own { None } else { Some(feet) }, volume: 0.5, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Hit { sound, at } => {
                use net::protocol::HitSound::*;
                let options: &[&str] = match sound {
                    Flesh => &["player/bhit_flesh-1.wav", "player/bhit_flesh-2.wav", "player/bhit_flesh-3.wav"],
                    Kevlar => &["player/bhit_kevlar-1.wav"],
                    Helmet => &["player/bhit_helmet-1.wav"],
                    Headshot => &["player/headshot1.wav", "player/headshot2.wav", "player/headshot3.wav"],
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
            Cue::Swing { stab } => {
                let file = if stab { "weapons/knife_stab.wav".into() } else { sounds.pick(&["weapons/knife_slash1.wav", "weapons/knife_slash2.wav"]) };
                plays.push(Play { file, at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 });
            }
            Cue::Zoom => plays.push(Play { file: "weapons/zoom.wav".into(), at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 }),
            Cue::File(file) => plays.push(Play { file, at: None, volume: 1.0, attn: ATTN_NORM, pitch: 1.0 }),
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
