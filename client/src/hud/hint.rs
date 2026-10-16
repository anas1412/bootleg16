//! Short hints in the low middle ("Not in a buy zone"): the line tunes in with a soft blip,
//! holds, and tunes out; a new one replaces it. Send a `Hint`.
use crate::menu::AppState;
use crate::ui::{kit_text, vh, Anim, Kit, UiSound, TX1};
use bevy::prelude::*;

/// A hint to show.
#[derive(Message)]
pub struct Hint(pub String);

/// How long a hint stays.
const HOLD: f32 = 2.0;
/// Above the plant bar and the dead notice.
const BOTTOM: f32 = 320.0;

#[derive(Component)]
pub(super) struct Shown(f32);

pub(super) fn plugin(app: &mut App) {
    app.add_message::<Hint>().add_systems(Update, show);
}

fn show(mut commands: Commands, mut hints: MessageReader<Hint>, time: Res<Time>, kit: Res<Kit>, shown: Query<(Entity, &Shown)>, mut sounds: MessageWriter<UiSound>) {
    let now = time.elapsed_secs();
    let leave = |commands: &mut Commands, e: Entity| {
        commands.entity(e).remove::<Shown>().insert(Anim::tune_out());
    };
    if let Some(Hint(text)) = hints.read().last() {
        for (e, _) in &shown {
            leave(&mut commands, e);
        }
        sounds.write(UiSound::Off);
        commands.spawn((Shown(now), Anim::tune_in(0.0), super::Visor, DespawnOnExit(AppState::InGame), Node { position_type: PositionType::Absolute, left: px(0), right: px(0), bottom: vh(BOTTOM), justify_content: JustifyContent::Center, ..default() })).with_children(|row| {
            row.spawn((kit_text(text.clone(), 18.0, TX1, &kit.text), TextShadow { offset: Vec2::splat(1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) }));
        });
        return;
    }
    for (e, s) in &shown {
        if now - s.0 > HOLD {
            leave(&mut commands, e);
        }
    }
}
