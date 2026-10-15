//! The in-game HUD: crosshair, health / armor / ammo, kill feed, scoreboard (Tab).
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use bevy::prelude::*;
use bevy::text::FontSize;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn).add_systems(Update, (hud, scoreboard).in_set(Frame::Ui));
    }
}

#[derive(Component)]
struct Status;

#[derive(Component)]
struct Feed;

#[derive(Component)]
struct Scores;

fn label(s: &str, size: f32, color: Color, node: Node) -> impl Bundle {
    (Text::new(s), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(color), node, DespawnOnExit(AppState::InGame))
}

fn spawn(mut commands: Commands) {
    commands.spawn(label("+", 22.0, Color::srgb(0.2, 1.0, 0.2), Node { position_type: PositionType::Absolute, left: percent(50), top: percent(50), margin: UiRect { left: px(-6), top: px(-14), ..default() }, ..default() }));
    commands.spawn((Status, label("", 22.0, Color::srgb(1.0, 0.85, 0.3), Node { position_type: PositionType::Absolute, left: px(16), bottom: px(12), ..default() })));
    commands.spawn((Scores, label("", 20.0, Color::WHITE, Node { position_type: PositionType::Absolute, left: percent(30), top: percent(20), ..default() })));
    commands.spawn((Feed, label("", 18.0, Color::WHITE, Node { position_type: PositionType::Absolute, right: px(16), top: px(12), ..default() })));
}

/// Tab: scoreboard while held.
fn scoreboard(keys: Res<ButtonInput<KeyCode>>, w: Res<Walker>, mut text: Single<&mut Text, With<Scores>>) {
    text.0 = if keys.pressed(KeyCode::Tab) {
        let mut t = format!("{:<24} {:>6} {:>6}\n", "Player", "Kills", "Deaths");
        for s in &w.client.scores {
            let me = if Some(s.id) == w.client.id() { " (you)" } else { "" };
            let dead = if s.alive { "" } else { "  dead" };
            t += &format!("\n{:<24} {:>6} {:>6}{dead}", format!("{}{me}", s.name), s.kills, s.deaths);
        }
        t
    } else {
        String::new()
    };
}

fn hud(w: Res<Walker>, mut status: Single<&mut Text, (With<Status>, Without<Feed>)>, mut feed: Single<&mut Text, (With<Feed>, Without<Status>)>) {
    let v = w.client.vitals;
    let l = &w.client.state.loadout;
    let weapon = sim::weapons::def(l.active_id()).name;
    let ammo = l.active_gun().map(|g| format!("{} / {}", g.clip, g.reserve)).unwrap_or_default();
    let [he, fl, sm] = l.grenades;
    let nades = if he + fl + sm > 0 { format!("      HE {he}  flash {fl}  smoke {sm}") } else { String::new() };
    status.0 = if v.alive() {
        format!("+ {:.0}   armor {:.0}{}      {weapon}  {ammo}{nades}", v.health, v.armor, if v.helmet { " (helmet)" } else { "" })
    } else if w.client.team() == net::protocol::Team::Spectator {
        "spectating - M to choose a team".into()
    } else {
        "dead - respawning".into()
    };
    feed.0 = w.feed.iter().map(|(l, _)| l.as_str()).collect::<Vec<_>>().join("\n");
}
