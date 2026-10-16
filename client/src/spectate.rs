//! Spectating while dead, as in competitive CS: you watch only living teammates (left and
//! right click cycle through them), and when your whole team is out the screen goes black until
//! the next round. The server sends a dead player only what living teammates can see, so there
//! is nothing else to show. Players on the spectator team may watch anyone.
use crate::game::{Frame, Walker};
use crate::menu::AppState;
use crate::ui::{kit_text, vh, Anim, Kit, BG0, TX2, TX3};
use bevy::prelude::*;
use bevy::window::{CursorGrabMode, CursorOptions};
use net::protocol::Team;

pub struct SpectatePlugin;

impl Plugin for SpectatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Watch>().add_systems(Update, (cycle, team_out).chain().in_set(Frame::Ui).run_if(resource_exists::<Walker>));
    }
}

/// Which living teammate you watch (an index into them, wrapped), and whether the team is out.
#[derive(Resource, Default)]
pub struct Watch {
    index: usize,
    pub out: bool,
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
    if out == watch.out {
        return;
    }
    watch.out = out;
    for e in &screens {
        commands.entity(e).remove::<OutScreen>().insert(Anim::tune_out());
    }
    if !out {
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
