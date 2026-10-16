//! Player settings: what they are and their file (~/.config/bootleg16/settings.txt).
use bevy::prelude::*;
use std::path::PathBuf;

/// Player settings, saved in ~/.config/bootleg16/settings.txt.
#[derive(Resource, Clone)]
pub struct Settings {
    pub name: String,
    /// Mouse sensitivity, as in CS (× 0.022° per mouse count).
    pub sensitivity: f32,
    pub volume: f32,
    /// Frame rate cap (0 = none), like CS's fps_max.
    pub fps_max: u32,
    pub vsync: bool,
    /// CS2 "Show Team ID Through Walls": 0 off, 1 pips, 2 pips and names.
    pub team_ids: u8,
    /// AI assistant: index into `assistant::ASSISTANTS`, or `assistant::OFF`.
    pub assistant: u8,
    /// How often it talks: 0 quiet, 1 normal, 2 chatty.
    pub chattiness: u8,
    /// Relay live team info (kills, fights, bomb) like a teammate typing.
    pub relay: bool,
    /// Talk during live rounds at all (competitive servers can force this off).
    pub assistant_in_round: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { name: "player".into(), sensitivity: 5.0, volume: 0.8, fps_max: 300, vsync: false, team_ids: 2, assistant: 0, chattiness: 1, relay: true, assistant_in_round: true }
    }
}

pub(crate) fn settings_path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".config/bootleg16/settings.txt")
}

impl Settings {
    pub fn load() -> Settings {
        let mut s = Settings::default();
        if let Ok(text) = std::fs::read_to_string(settings_path()) {
            for line in text.lines() {
                match line.split_once('=') {
                    Some(("name", v)) => s.name = v.trim().to_string(),
                    Some(("sensitivity", v)) => s.sensitivity = v.trim().parse().unwrap_or(s.sensitivity),
                    Some(("volume", v)) => s.volume = v.trim().parse().unwrap_or(s.volume),
                    Some(("fps_max", v)) => s.fps_max = v.trim().parse().unwrap_or(s.fps_max),
                    Some(("vsync", v)) => s.vsync = v.trim() == "true",
                    Some(("team_ids", v)) => s.team_ids = v.trim().parse::<u8>().unwrap_or(s.team_ids).min(2),
                    Some(("assistant", v)) => s.assistant = v.trim().parse::<u8>().unwrap_or(s.assistant).min(crate::assistant::OFF),
                    Some(("chattiness", v)) => s.chattiness = v.trim().parse::<u8>().unwrap_or(s.chattiness).min(2),
                    Some(("relay", v)) => s.relay = v.trim() == "true",
                    Some(("assistant_in_round", v)) => s.assistant_in_round = v.trim() == "true",
                    _ => {}
                }
            }
        }
        s
    }

    pub(crate) fn save(&self) {
        let p = settings_path();
        let _ = std::fs::create_dir_all(p.parent().unwrap());
        let _ = std::fs::write(
            p,
            format!(
                "name={}\nsensitivity={}\nvolume={}\nfps_max={}\nvsync={}\nteam_ids={}\nassistant={}\nchattiness={}\nrelay={}\nassistant_in_round={}\n",
                self.name, self.sensitivity, self.volume, self.fps_max, self.vsync, self.team_ids, self.assistant, self.chattiness, self.relay, self.assistant_in_round
            ),
        );
    }
}

