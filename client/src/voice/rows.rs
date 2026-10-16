//! Who is talking, top left under the money (design/mockups/ui `voiceRow`): one row each, the
//! shared feed row like the kill feed's. A mic, a live level, the name in its side's colour
//! and the channel (Team or Near). Yours is tinted amber with the amber bar on the outer edge.
//! Rows tune in when someone starts and tune out a moment after they stop.
use super::{MicLevel, VoiceSettings};
use crate::game::Walker;
use crate::menu::{AppState, Overlay};
use crate::ui::{feed_row, kit_text, vh, Anim, HudLayer, Kit, Outer, AMBER, CT_COLOR, TX1, TX2, T_COLOR};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use net::protocol::{Team, VoiceChannel};
use std::collections::HashMap;

/// Who we heard: channel, when, and how loud the last frame was.
#[derive(Resource, Default)]
pub struct Talking(pub(super) HashMap<u8, (VoiceChannel, f32, f32)>);

impl Talking {
    /// How loud a player is talking right now (None: not talking).
    pub fn level(&self, id: u8, now: f32) -> Option<f32> {
        self.0.get(&id).filter(|(_, at, _)| now - at < LINGER).map(|(_, _, l)| *l)
    }
}

/// Someone stays on the list this long after their last frame.
const LINGER: f32 = 0.3;

#[derive(Component)]
pub(super) struct VoiceRoot;
/// A row: whose (None = you) and its smoothed level.
#[derive(Component)]
pub(super) struct Row(Option<u8>, f32);
#[derive(Component)]
pub(super) struct Level(usize);

#[derive(Resource)]
pub struct MicIcon(pub Handle<Image>);

pub(super) fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    commands.insert_resource(Talking::default());
    commands.insert_resource(MicIcon(images.add(mic_image(48))));
    commands.spawn((
        VoiceRoot,
        HudLayer,
        DespawnOnExit(AppState::InGame),
        Node { position_type: PositionType::Absolute, left: vh(crate::hud::SAFE), top: vh(crate::hud::VOICES_TOP), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexStart, row_gap: vh(4.0), ..default() },
    ));
}

/// A microphone: capsule, cradle and stand, white on clear (tinted where it's used).
fn mic_image(n: u32) -> Image {
    let s = n as f32 / 24.0;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (px, py) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
            // capsule: a rounded box 8 wide from y 2 to 15
            let cy = py.clamp(6.0, 11.0);
            let capsule = 4.0 - ((px - 12.0).powi(2) + (py - cy).powi(2)).sqrt();
            // cradle: the lower half of a ring around the capsule
            let ring = 1.0 - ((((px - 12.0).powi(2) + (py - 11.0).powi(2)).sqrt() - 7.0).abs());
            let cradle = if py >= 11.0 { ring } else { -1.0 };
            // stand and foot
            let stem = if (18.0..22.0).contains(&py) { 1.0 - (px - 12.0).abs() } else { -1.0 };
            let foot = if (21.0..23.0).contains(&py) { 4.0 - (px - 12.0).abs() } else { -1.0 };
            let d = capsule.max(cradle).max(stem).max(foot);
            let a = (d * s).clamp(0.0, 1.0);
            let i = ((y * n + x) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[255, 255, 255, (a * 255.0) as u8]);
        }
    }
    crate::level::rgba_image(n, n, rgba, false, TextureFormat::Rgba8UnormSrgb)
}

fn side(t: Team) -> Color {
    match t {
        Team::Ct => CT_COLOR,
        Team::T => T_COLOR,
        Team::Spectator => TX1,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn rows(
    mut commands: Commands,
    w: Res<Walker>,
    settings: Res<VoiceSettings>,
    binds: Res<crate::binds::Binds>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Option<Res<Overlay>>,
    talking: Res<Talking>,
    mic_level: Res<MicLevel>,
    time: Res<Time>,
    kit: Res<Kit>,
    icon: Res<MicIcon>,
    root: Single<Entity, With<VoiceRoot>>,
    mut shown: Query<(Entity, &mut Row, &Children)>,
    groups: Query<&Children, Without<Row>>,
    mut bars: Query<(&Level, &mut Node)>,
) {
    let now = time.elapsed_secs();
    let info = |id: u8| w.client.scores.iter().find(|s| s.id == id).map(|s| (s.name.clone(), s.team));
    // (who, name, colour, channel, level)
    let mut want: Vec<(Option<u8>, String, Color, VoiceChannel, f32)> = Vec::new();
    let mine = super::pushed(&settings, &binds, &keys, &mouse, overlay);
    // screenshot runs: BOOTLEG16_VOICE shows you and a teammate talking
    let fake = std::env::var_os("BOOTLEG16_VOICE").is_some();
    let mine = if fake { Some(VoiceChannel::Team) } else { mine };
    if let Some(ch) = mine {
        let name = w.client.id().and_then(|id| info(id)).map(|i| i.0).unwrap_or_else(|| "You".into());
        want.push((None, name, TX1, ch, if fake { 0.7 } else { mic_level.0 }));
    }
    if fake {
        want.push((Some(u8::MAX), "Ghost".into(), CT_COLOR, VoiceChannel::Team, 0.5 + 0.4 * (now * 7.0).sin().abs()));
    }
    let mut others: Vec<_> = talking.0.iter().filter(|(_, (_, at, _))| now - at < LINGER).filter_map(|(id, (ch, _, lvl))| info(*id).map(|(n, t)| (Some(*id), n, side(t), *ch, *lvl))).collect();
    others.sort_by(|a, b| a.1.cmp(&b.1));
    want.extend(others);

    let dt = time.delta_secs();
    let mut have = Vec::new();
    for (e, mut row, kids) in &mut shown {
        let Some(&(_, _, _, _, lvl)) = want.iter().find(|w| w.0 == row.0) else {
            commands.entity(e).remove::<Row>().insert(Anim::tune_out());
            continue;
        };
        have.push(row.0);
        // the level: quick up, slower down, a little uneven bar to bar
        let k = if lvl > row.1 { 0.6 } else { (dt / 0.12).min(1.0) };
        row.1 += (lvl.min(1.0) * 1.6 - row.1) * k;
        let l = row.1.min(1.0);
        for bar in kids.iter().filter_map(|k| groups.get(k).ok()).flatten() {
            if let Ok((Level(i), mut n)) = bars.get_mut(*bar) {
                let h = 3.0 + (l - *i as f32 * 0.12).max(0.0) * 11.0 * (0.7 + 0.3 * fastrand::f32());
                n.height = vh(h);
            }
        }
    }
    commands.entity(*root).with_children(|col| {
        for (who, name, color, ch, _) in want.into_iter().filter(|w| !have.contains(&w.0)) {
            let you = who.is_none();
            let tint = if you { AMBER } else { color };
            col.spawn((Row(who, 0.0), Anim::tune_in(0.0), feed_row(36.0, you, Outer::Left))).with_children(|r| {
                r.spawn((ImageNode { image: icon.0.clone(), color: tint, ..default() }, Node { width: vh(18.0), height: vh(18.0), ..default() }));
                r.spawn(Node { height: vh(14.0), align_items: AlignItems::Center, column_gap: vh(2.0), ..default() }).with_children(|lv| {
                    for i in 0..4 {
                        lv.spawn((Level(i), Node { width: vh(3.0), height: vh(3.0), border_radius: BorderRadius::all(px(1)), ..default() }, BackgroundColor(if you { AMBER } else { TX2 })));
                    }
                });
                r.spawn(kit_text(name, 16.0, if you { TX1 } else { color }, &kit.num));
                r.spawn(kit_text(if ch == VoiceChannel::Team { "TEAM" } else { "NEAR" }, 13.0, TX2, &kit.text));
            });
        }
    });
}
