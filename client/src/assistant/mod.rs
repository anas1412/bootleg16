//! The AI assistant: a companion that talks to you during matches (only there), in one of
//! four voices. Its brain (which line, when) is `client_core::assistant`; this draws it.
//! Same information for all four assistants, and only what your team already knows.
mod uplink;
pub mod view;

use crate::effects::GameNews;
use crate::game::{Frame, Walker};
use crate::menu::{AppState, Settings};
use bevy::prelude::*;
use client_core::assistant::{Known, Mood, Prefs, NAMES};

/// One of the four AI assistants.
pub struct Assistant {
    pub name: &'static str,
    /// Ring colour of its face.
    pub color: Color,
}

pub const ASSISTANTS: [Assistant; 4] = [
    Assistant { name: NAMES[0], color: Color::srgb(0.910, 0.529, 0.373) },
    Assistant { name: NAMES[1], color: Color::srgb(0.435, 0.890, 0.757) },
    Assistant { name: NAMES[2], color: Color::srgb(0.663, 0.545, 1.0) },
    Assistant { name: NAMES[3], color: Color::srgb(0.831, 1.0, 0.227) },
];

/// `Settings::assistant` value meaning "no assistant".
pub const OFF: u8 = ASSISTANTS.len() as u8;

/// The last line said. Twins: `text` holds her line and his after a newline; `mood2` is his.
pub struct Said {
    pub who: usize,
    pub mood: Mood,
    pub mood2: Mood,
    pub text: String,
}

#[derive(Resource, Default)]
pub struct Brain {
    core: client_core::assistant::Brain,
    pub said: Option<Said>,
    /// Bumped on every new line (the views redraw on change).
    pub version: u32,
}

pub struct AssistantPlugin;

impl Plugin for AssistantPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Brain>()
        .init_resource::<view::Faces>()
        .add_systems(Startup, |mut s: ResMut<Settings>| {
            // Screenshot runs pick the assistant (0-3) without touching the saved settings.
            if let Some(i) = std::env::var("BOOTLEG16_ASSISTANT").ok().and_then(|v| v.parse::<u8>().ok()) {
                s.assistant = i.min(OFF);
            }
        })
        .add_systems(OnEnter(AppState::InGame), new_match)
        .add_systems(Update, in_game.in_set(Frame::Ui).run_if(resource_exists::<Walker>))
        .add_systems(Update, (uplink::draw, uplink::animate, uplink::light, uplink::reveal).chain().after(in_game).run_if(in_state(AppState::InGame)));
        view::plugin(app);
    }
}

/// A new match starts clean.
fn new_match(mut brain: ResMut<Brain>) {
    brain.core.new_match();
}

/// Match events into lines; the newest one this frame is shown.
fn in_game(mut news: MessageReader<GameNews>, w: Res<Walker>, settings: Res<Settings>, time: Res<Time>, mut brain: ResMut<Brain>) {
    let events: Vec<_> = news.read().map(|GameNews(e)| e.clone()).collect();
    let prefs = Prefs { who: settings.assistant as usize, name: settings.name.clone(), in_round: settings.assistant_in_round, chattiness: settings.chattiness, relay: settings.relay };
    let lines = brain.core.update(&events, &Known::from_client(&w.client), &prefs, time.elapsed_secs());
    // the newest line, and the brother's answer after it (Twins)
    let Some(start) = lines.iter().rposition(|l| !l.second) else { return };
    let (first, rest) = (&lines[start], &lines[start + 1..]);
    let text = std::iter::once(first).chain(rest).map(|l| l.text.as_str()).collect::<Vec<_>>().join("\n");
    let mood2 = rest.first().map_or(first.mood.brother_to(), |l| l.mood);
    brain.said = Some(Said { who: first.who, mood: first.mood, mood2, text });
    brain.version += 1;
}
