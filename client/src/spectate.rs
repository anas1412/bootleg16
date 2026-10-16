//! Spectating while dead, as in competitive CS: you watch living teammates through their own
//! eyes (left and right click cycle through them). When your whole team is out, the camera
//! watches where the last of you went down, from behind, until the next round (black if
//! nobody's known). The server sends a dead player only what living teammates can see, so a
//! team that's out sees no enemies at all. Players on the spectator team may watch anyone.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{kit_text, vh, Anim, Kit, BG0, TX2, TX3};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use net::protocol::Team;

pub struct SpectatePlugin;

impl Plugin for SpectatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>().add_systems(Update, (note_downs, cycle, team_out).chain().in_set(Frame::Ui).run_if(resource_exists::<Walker>));
    }
}

/// Which living teammate you watch (an index into them, wrapped), and whether the team is out.
#[derive(Resource, Default)]
pub struct Watch {
    index: usize,
    pub out: bool,
    /// Where the last of your team went down this round, and which way they faced.
    pub last_down: Option<([f32; 3], f32)>,
}

/// Remember where teammates die (the camera goes there once the whole team is out).
fn note_downs(mut news: MessageReader<crate::effects::GameNews>, w: Res<Walker>, mut watch: ResMut<Watch>, cam: Single<&crate::game::FlyCam>) {
    let mine = w.client.team();
    for crate::effects::GameNews(e) in news.read() {
        match e {
            net::protocol::GameEvent::RoundStart { .. } => watch.last_down = None,
            net::protocol::GameEvent::Kill { victim, .. } => {
                let at = if Some(*victim) == w.client.id() {
                    Some((w.client.me().origin, cam.yaw.to_degrees() + 90.0))
                } else {
                    w.client.others().into_iter().find(|o| o.id == *victim && o.team == mine).map(|o| (o.origin, o.yaw))
                };
                if at.is_some() {
                    watch.last_down = at;
                }
            }
            _ => {}
        }
    }
}

/// The teammate you see through the eyes of (None: not spectating, or nobody left).
/// `BOOTLEG16_SPECTATE=front` (model checks) watches from in front instead.
pub fn in_eye(w: &Walker, watch: &Watch) -> Option<net::client::Remote> {
    if !spectating(w) || std::env::var("BOOTLEG16_SPECTATE").is_ok_and(|v| v == "front") {
        return None;
    }
    target(w, watch)
}

/// Whether you are watching someone instead of playing.
pub fn spectating(w: &Walker) -> bool {
    !w.client.vitals.alive() || std::env::var_os("BOOTLEG16_SPECTATE").is_some()
}

/// The player to watch: a living teammate (any living player for the spectator team).
pub fn target(w: &Walker, watch: &Watch) -> Option<net::client::Remote> {
    let mine = w.client.team();
    let mut alive: Vec<_> = w.client.others().into_iter().filter(|o| o.alive && (mine == Team::Spectator || o.team == mine)).collect();
    alive.sort_by_key(|o| o.id);
    let n = alive.len();
    (n > 0).then(|| alive.swap_remove(watch.index % n))
}

/// Left and right click: next and previous teammate (only while the mouse is captured).
fn cycle(mouse: Res<ButtonInput<MouseButton>>, cursor: Single<&CursorOptions>, w: Res<Walker>, mut watch: ResMut<Watch>) {
    if !spectating(&w) || cursor.grab_mode == CursorGrabMode::None {
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        watch.index = watch.index.wrapping_add(1);
    }
    if mouse.just_pressed(MouseButton::Right) {
        watch.index = watch.index.wrapping_add(usize::MAX);
    }
}

#[derive(Component)]
struct OutScreen;

/// No living teammate while you are dead: black over the world, under the HUD.
fn team_out(mut commands: Commands, w: Res<Walker>, kit: Res<Kit>, mut watch: ResMut<Watch>, screens: Query<Entity, With<OutScreen>>) {
    let out = spectating(&w) && w.client.team() != Team::Spectator && w.client.id().is_some() && target(&w, &watch).is_none();
    let black = out && watch.last_down.is_none();
    let was = screens.iter().next().is_some();
    watch.out = out;
    if black == was {
        return;
    }
    for e in &screens {
        commands.entity(e).remove::<OutScreen>().insert(Anim::tune_out());
    }
    if !black {
        return;
    }
    commands
        .spawn((
            OutScreen,
            DespawnOnExit(AppState::InGame),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, justify_content: JustifyContent::Center, row_gap: vh(8.0), ..default() },
            BackgroundColor(BG0),
            GlobalZIndex(-1),
            Anim::tune_in(0.0),
        ))
        .with_children(|c| {
            c.spawn(kit_text("YOUR TEAM IS OUT", 28.0, TX2, &kit.text));
            c.spawn(kit_text("NEXT ROUND SOON", 13.0, TX3, &kit.text));
        });
}
