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
    /// A borderless window the size of the screen (not the compositor's fullscreen mode).
    pub borderless: bool,
    /// The window's size when it is not borderless: an index into `window::RESOLUTIONS`.
    pub resolution: u8,
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
    /// Crosshair: colour (index into `hud::CROSSHAIR_COLORS`), size 0 small .. 2 large, whether
    /// the gap follows the weapon's spread (CS's dynamic crosshair), and a centre dot.
    pub crosshair_color: u8,
    pub crosshair_size: u8,
    pub crosshair_dynamic: bool,
    pub crosshair_dot: bool,
    /// How much the HUD bends like a visor (0 flat .. 100), `hud::visor`.
    pub hud_curve: u8,
    /// Your character as a terrorist and as a counter-terrorist (Characters page); None until
    /// you pick (the server varies them).
    pub looks: Option<[u8; 2]>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { name: "player".into(), sensitivity: 5.0, volume: 0.8, fps_max: 300, vsync: false, borderless: false, resolution: crate::window::DEFAULT_RESOLUTION, team_ids: 2, assistant: 0, chattiness: 1, relay: true, assistant_in_round: true, crosshair_color: 0, crosshair_size: 1, crosshair_dynamic: true, crosshair_dot: false, hud_curve: 60, looks: None }
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
                    Some(("borderless", v)) => s.borderless = v.trim() == "true",
                    Some(("resolution", v)) => s.resolution = v.trim().parse::<u8>().unwrap_or(s.resolution).min(crate::window::RESOLUTIONS.len() as u8 - 1),
                    Some(("team_ids", v)) => s.team_ids = v.trim().parse::<u8>().unwrap_or(s.team_ids).min(2),
                    Some(("assistant", v)) => s.assistant = v.trim().parse::<u8>().unwrap_or(s.assistant).min(crate::assistant::OFF),
                    Some(("chattiness", v)) => s.chattiness = v.trim().parse::<u8>().unwrap_or(s.chattiness).min(2),
                    Some(("relay", v)) => s.relay = v.trim() == "true",
                    Some(("assistant_in_round", v)) => s.assistant_in_round = v.trim() == "true",
                    Some(("crosshair_color", v)) => s.crosshair_color = v.trim().parse::<u8>().unwrap_or(0).min(crate::hud::CROSSHAIR_COLORS.len() as u8 - 1),
                    Some(("crosshair_size", v)) => s.crosshair_size = v.trim().parse::<u8>().unwrap_or(1).min(2),
                    Some(("crosshair_dynamic", v)) => s.crosshair_dynamic = v.trim() == "true",
                    Some(("crosshair_dot", v)) => s.crosshair_dot = v.trim() == "true",
                    Some(("looks", v)) => {
                        let n: Vec<u8> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                        if let [t, ct] = n[..] {
                            s.looks = Some([t.min(net::protocol::CLASSES - 1), ct.min(net::protocol::CLASSES - 1)]);
                        }
                    }
                    Some(("hud_curve", v)) => s.hud_curve = v.trim().parse::<u8>().unwrap_or(60).min(100),
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
                "name={}\nsensitivity={}\nvolume={}\nfps_max={}\nvsync={}\nborderless={}\nresolution={}\nteam_ids={}\nassistant={}\nchattiness={}\nrelay={}\nassistant_in_round={}\ncrosshair_color={}\ncrosshair_size={}\ncrosshair_dynamic={}\ncrosshair_dot={}\nhud_curve={}\n{}",
                self.name, self.sensitivity, self.volume, self.fps_max, self.vsync, self.borderless, self.resolution, self.team_ids, self.assistant, self.chattiness, self.relay, self.assistant_in_round, self.crosshair_color, self.crosshair_size, self.crosshair_dynamic, self.crosshair_dot, self.hud_curve,
                self.looks.map(|[t, ct]| format!("looks={t},{ct}\n")).unwrap_or_default()
            ),
        );
    }
}

