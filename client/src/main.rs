//! The game client. Opens on the main menu; practice starts a server inside the game,
//! "Join server" connects to one. Each part of the game is its own plugin module.
//!
//! Usage: `client [map] [--connect host:port] [--name name] [--give ak47,awp] [--practice N]`
//! (any argument skips the menu). The game directory comes from `BOOTLEG16_DIR`
//! (default: the Steam Half-Life install).
//! `BOOTLEG16_FAKE_PING=ms` and `BOOTLEG16_FAKE_LOSS=0.05` simulate a bad connection.
//! `BOOTLEG16_SCREENSHOT=path.png` saves one frame (after `BOOTLEG16_SCREENSHOT_AT` seconds,
//! default 2) and exits; such runs ignore mouse and keyboard (used for automated checks).
//! `BOOTLEG16_TEAM=t|ct|spec` picks the team in such runs (default auto).
//! `BOOTLEG16_MENU_PAGE=practice|join|settings|crosshair|assistant|voice` opens that menu page
//! (crosshair and assistant: Settings sections) in such runs.
//! `BOOTLEG16_OVERLAY=pause|teams` shows that in-game overlay; `BOOTLEG16_MENU=1` the buy menu.
//! `BOOTLEG16_SCORES=1` holds the scoreboard open.
//! `BOOTLEG16_ASSISTANT=0..3` picks the AI assistant for the run (not saved).
//! `BOOTLEG16_VOICE=1` shows you and a teammate on the voice list.
//! `BOOTLEG16_FOUND=1` shows a match found and the Reconnect card on the menu.
//! `BOOTLEG16_SPECTATE=1` watches another player instead of playing (as when dead);
//! `BOOTLEG16_SPECTATE=front` watches them from the front.
mod binds;
mod bomb;
mod assistant;
mod buy;
mod effects;
mod framerate;
mod game;
mod found;
mod hud;
mod impacts;
mod killfeed;
mod level;
mod map;
mod map_pick;
mod map_sounds;
mod match_flow;
mod menu;
mod menu_pages;
mod models;
mod muzzle;
mod overlay;
mod play;
mod play_page;
mod players;
mod radar;
mod round_hud;
mod scoreboard;
mod spectate;
mod sky;
mod smoke;
mod sound;
mod steam;
mod teammates;
mod ui;
mod viewmodel;
mod vote;
mod voice;

use bevy::{
    light::GlobalAmbientLight,
    prelude::*,
    render::view::window::screenshot::{save_to_disk, Screenshot},
};
use std::path::PathBuf;

fn game_dir() -> PathBuf {
    std::env::var_os("BOOTLEG16_DIR").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".steam/steam/steamapps/common/Half-Life")
    })
}

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(WindowPlugin { primary_window: Some(Window { title: "bootleg16".into(), ..default() }), ..default() })
                // Shrink audio space so rodio's 1/d^2 loudness never kicks in: it only pans,
                // and `sound` applies GoldSrc's own distance falloff.
                .set(bevy::audio::AudioPlugin { default_spatial_scale: bevy::audio::SpatialScale::new(1.0 / 4096.0), ..default() }),
        )
        // GoldSrc lighting is fully baked into lightmaps; no extra ambient.
        .insert_resource(GlobalAmbientLight { brightness: 0.0, ..default() })
        .insert_resource(ClearColor(Color::srgb(0.45, 0.6, 0.8)))
        // The movement simulation runs at a fixed 128 ticks per second, like the server.
        .insert_resource(Time::<Fixed>::from_hz(128.0))
        .insert_resource(models::ModelCache::new(game_dir()))
        .insert_resource(sound::Sounds::new(game_dir()))
        .init_state::<menu::AppState>()
        .add_plugins((
            menu::MenuPlugin,
            framerate::FrameratePlugin,
            game::GamePlugin,
            level::LevelPlugin,
            players::PlayersPlugin,
            effects::EffectsPlugin,
            impacts::ImpactsPlugin,
            viewmodel::ViewModelPlugin,
            sound::SoundPlugin,
            buy::BuyPlugin,
            hud::HudPlugin,
            killfeed::KillFeedPlugin,
            round_hud::RoundHudPlugin,
            // (Bevy takes at most 15 plugins per tuple.)
            (bomb::BombPlugin, map_sounds::MapSoundsPlugin, teammates::TeammatesPlugin, voice::VoicePlugin, smoke::SmokePlugin, sky::SkyPlugin, muzzle::MuzzlePlugin, assistant::AssistantPlugin, scoreboard::ScoreboardPlugin, radar::RadarPlugin, ui::UiPlugin, spectate::SpectatePlugin, vote::VotePlugin, binds::BindsPlugin),
        ))
        .add_systems(Update, screenshot)
        .run();
}

/// With BOOTLEG16_SCREENSHOT set: save a frame to that path after ~2 s (or
/// BOOTLEG16_SCREENSHOT_AT seconds), then quit.
fn screenshot(mut commands: Commands, time: Res<Time>, mut taken: Local<Option<f32>>, mut exit: MessageWriter<AppExit>) {
    let Some(path) = std::env::var_os("BOOTLEG16_SCREENSHOT") else { return };
    let at: f32 = std::env::var("BOOTLEG16_SCREENSHOT_AT").ok().and_then(|f| f.parse().ok()).unwrap_or(2.0);
    let now = time.elapsed_secs();
    match *taken {
        None if now >= at => {
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(PathBuf::from(path)));
            *taken = Some(now);
        }
        Some(t) if now >= t + 1.0 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}
