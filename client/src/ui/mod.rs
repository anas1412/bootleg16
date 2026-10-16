//! The bootleg16 design system, Interference (docs/UI.md; the mockup in design/mockups/ui is
//! its spec). One place for everything visual:
//! - `tokens`: colours, space, type scale, timings;
//! - `kit`: fonts and text helpers (`t` on the type scale);
//! - `panels`: soft surfaces, HUD glass, cards, feed rows and the anchor (mirroring) rule;
//! - `widgets`: lists, chips, steppers, buttons, titles, status labels;
//! - `motion`: tuning in and out, bursts, sliding selections, rolls, the blur behind menus,
//!   and the HUD's live values (edge lights, meters with trails, counting numbers, tints);
//! - `sound`: the UI's own blips (assistant chirps, timer ticks).
//! Screens build from these; nothing defines its own colours or sizes.
mod kit;
mod motion;
mod panels;
mod sound;
mod tokens;
mod widgets;

pub use kit::*;
pub use motion::*;
pub use panels::*;
pub use sound::UiSound;
pub use tokens::*;
pub use widgets::*;

use bevy::prelude::{App, Plugin, Resource};

/// Something a button asks for. Menus read these from `UiActions` each frame.
#[derive(Clone, Debug, PartialEq)]
pub enum UiAction {
    Page(crate::menu::Page),
    PickMap(String),
    Dummies(i32),
    StartPractice,
    /// The Practice page's choice: deathmatch online (true) or solo.
    PracticeMode(bool),
    Connect,
    Sensitivity(f32),
    Volume(f32),
    FpsMax(i32),
    SetTeamIds(u8),
    SetVsync(bool),
    /// Index into `assistant::ASSISTANTS`, or `assistant::OFF`.
    SetAssistant(u8),
    /// 0 quiet, 1 normal, 2 chatty.
    Chattiness(u8),
    SetRelay(bool),
    SetAssistantInRound(bool),
    /// Wait for a new key for this action (Settings · Keys, Voice).
    Rebind(crate::binds::Action),
    ResetBinds,
    /// Settings page section: 0 general, 1 crosshair, 2 AI assistant, 3 keys.
    SettingsTab(u8),
    CrosshairColor(u8),
    CrosshairSize(u8),
    CrosshairDynamic(bool),
    CrosshairDot(bool),
    /// HUD curve up or down a step.
    HudCurve(i8),
    Quit,
    Resume,
    Leave,
    /// Call a surrender vote (a teammate abandoned the match).
    Surrender,
    /// Open (or close) the report menu under a player's scoreboard row.
    ReportMenu(u8),
    Report(u8, net::protocol::ReportReason),
    /// Ask your team to vote this teammate out (reserved matches).
    Kick(u8),
    ChooseTeam(Option<net::protocol::Team>),
    ShowTeams,
    Buy(net::protocol::BuyItem),
    BuyCategory(usize),
    /// Select a card in the open buy category (the details panel shows it).
    SelectBuy(usize),
    CloseBuy,
    Voice(crate::voice::VoiceAction),
    Lobby(crate::play::LobbyAction),
}

#[derive(Resource, Default)]
pub struct UiActions(pub Vec<UiAction>);

/// The shared UI resources and motion systems.
pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiActions>().init_resource::<Kit>();
        motion::plugin(app);
        sound::plugin(app);
    }
}
