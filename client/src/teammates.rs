//! CS2's "Show Team ID Through Walls": a team-coloured pip, and optionally the name, over
//! each teammate's head, seen through walls, so you know where your team is without a radar.
//! Settings: off / pips / names and health (`Settings::team_ids`). Markers fade with distance.
//! With names on, a mic and a live level sit before the name while that teammate talks
//! (design/mockups/ui `mateTag`).
//!
//! Safe against "make everyone a teammate" memory hacks: the server decides who your team is
//! and never sends enemies you can't see (server/visibility.rs), so there's nothing hidden to mark.
use crate::game::{FlyCam, Frame, Walker};
use crate::map;
use crate::menu::{AppState, Settings};
use crate::ui::{kit_text, BAD, CT_COLOR, T_COLOR};
use crate::voice::{MicIcon, Talking};
use bevy::prelude::*;
use net::protocol::Team;

pub struct TeammatesPlugin;

impl Plugin for TeammatesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, team_ids.in_set(Frame::Ui));
    }
}

/// One teammate's overhead marker, and its parts.
#[derive(Component)]
struct TeamId {
    id: u8,
    talk: Entity,
    mic: Entity,
    bars: [Entity; 4],
    name: Entity,
    track: Entity,
    fill: Entity,
    pip: Entity,
}

/// Distance (game units) where markers start to fade, where they reach their faintest, and how
/// faint that is.
const NEAR: f32 = 600.0;
const FAR: f32 = 2600.0;
const FAINTEST: f32 = 0.4;
/// The marker's width; it's centred over the head.
const WIDTH: f32 = 200.0;

#[allow(clippy::too_many_arguments)]
fn team_ids(
    mut commands: Commands,
    w: Res<Walker>,
    settings: Res<Settings>,
    cam: Single<(&Camera, &GlobalTransform), With<FlyCam>>,
    mut ids: Query<(Entity, &TeamId, &mut Visibility)>,
    mut nodes: Query<&mut Node>,
    mut colors: Query<&mut BackgroundColor>,
    mut texts: Query<&mut TextColor>,
    mut images: Query<&mut ImageNode>,
    (kit, talking, mic, time): (Res<crate::ui::Kit>, Option<Res<Talking>>, Option<Res<MicIcon>>, Res<Time>),
) {
    let now = time.elapsed_secs();
    let mine = w.client.team();
    let color = if mine == Team::Ct { CT_COLOR } else { T_COLOR };
    let on = settings.team_ids > 0 && mine != Team::Spectator;
    let names_on = settings.team_ids >= 2;
    // screenshot runs: BOOTLEG16_VOICE has every teammate talking
    let fake = std::env::var_os("BOOTLEG16_VOICE").is_some();
    let mates: Vec<_> = if on { w.client.others().into_iter().filter(|o| o.alive && o.team == mine).collect() } else { Vec::new() };
    let show = |b: bool| if b { Display::Flex } else { Display::None };
    for (e, t, mut v) in &mut ids {
        let Some(o) = mates.iter().find(|o| o.id == t.id) else {
            commands.entity(e).despawn();
            continue;
        };
        let head = Vec3::from(map::to_y_up([o.origin[0], o.origin[1], o.origin[2] + 44.0]));
        // near teammates read clearly, far ones step back so they don't crowd the screen
        let fade = (1.0 - (cam.1.translation().distance(head) - NEAR) / (FAR - NEAR)).clamp(FAINTEST, 1.0);
        let c = color.with_alpha(fade);
        match cam.0.world_to_viewport(cam.1, head) {
            Ok(p) => {
                if let Ok(mut n) = nodes.get_mut(e) {
                    // the pip's tip sits just over the head
                    let (left, top) = (px(p.x - WIDTH / 2.0), px(p.y - if names_on { 52.0 } else { 18.0 }));
                    if n.left != left || n.top != top {
                        n.left = left;
                        n.top = top;
                    }
                }
                v.set_if_neq(Visibility::Inherited);
            }
            Err(_) => {
                v.set_if_neq(Visibility::Hidden); // behind you
                continue;
            }
        }
        let level = if fake { Some(0.5 + 0.4 * (now * 7.0).sin().abs()) } else { talking.as_ref().and_then(|tk| tk.level(t.id, now)) };
        // health (the server sends it for teammates): red when low
        let hp = o.mate.map(|m| (m.health as f32 / 100.0).clamp(0.0, 1.0));
        for (part, d) in [(t.name, names_on), (t.talk, names_on && level.is_some()), (t.track, names_on && hp.is_some())] {
            if let Ok(mut n) = nodes.get_mut(part) {
                if n.display != show(d) {
                    n.display = show(d);
                }
            }
        }
        if let Ok(mut tc) = texts.get_mut(t.name) {
            tc.set_if_neq(TextColor(c));
        }
        if let Ok(mut img) = images.get_mut(t.mic) {
            if img.color != c {
                img.color = c;
            }
        }
        for (i, b) in t.bars.iter().enumerate() {
            let h = px(2.0 + (level.unwrap_or(0.0).min(1.0) - i as f32 * 0.15).max(0.0) * 12.0);
            if let Ok(mut n) = nodes.get_mut(*b) {
                if n.height != h {
                    n.height = h;
                }
            }
            if let Ok(mut bg) = colors.get_mut(*b) {
                bg.set_if_neq(BackgroundColor(c));
            }
        }
        if let Some(hp) = hp {
            if let Ok(mut n) = nodes.get_mut(t.fill) {
                if n.width != percent(hp * 100.0) {
                    n.width = percent(hp * 100.0);
                }
            }
            if let Ok(mut bg) = colors.get_mut(t.fill) {
                bg.set_if_neq(BackgroundColor(if hp <= 0.25 { BAD.with_alpha(fade) } else { c }));
            }
        }
        if let Ok(mut bg) = colors.get_mut(t.pip) {
            bg.set_if_neq(BackgroundColor(c));
        }
    }
    let Some(mic) = mic else { return };
    for o in &mates {
        if ids.iter().any(|(_, t, _)| t.id == o.id) {
            continue;
        }
        let name = w.client.scores.iter().find(|s| s.id == o.id).map(|s| s.name.clone()).unwrap_or_default();
        // dark shadows so every part reads on any wall
        let shadow = TextShadow { offset: Vec2::splat(1.0), color: Color::srgba(0.0, 0.0, 0.0, 0.9) };
        let dark = Color::srgba(0.0, 0.0, 0.0, 0.6);
        let mic_e = commands.spawn((ImageNode { image: mic.0.clone(), color, ..default() }, Node { width: px(16), height: px(16), ..default() })).id();
        let bars: [Entity; 4] = std::array::from_fn(|_| commands.spawn((Node { width: px(3), height: px(2), border_radius: BorderRadius::all(px(1)), ..default() }, BackgroundColor(color))).id());
        let level = commands.spawn(Node { height: px(14), align_items: AlignItems::Center, column_gap: px(2), ..default() }).add_children(&bars).id();
        let talk = commands.spawn(Node { display: Display::None, align_items: AlignItems::Center, column_gap: px(4), ..default() }).add_children(&[mic_e, level]).id();
        let name_e = commands.spawn((kit_text(name, 18.0, color, &kit.text), shadow)).id();
        let row = commands.spawn(Node { align_items: AlignItems::Center, column_gap: px(6), ..default() }).add_children(&[talk, name_e]).id();
        let fill = commands.spawn((Node { width: percent(100), height: percent(100), ..default() }, BackgroundColor(color))).id();
        let track = commands
            .spawn((Node { display: Display::None, width: px(64), height: px(5), margin: UiRect::top(px(4)), border_radius: BorderRadius::all(px(2)), overflow: Overflow::clip(), ..default() }, BackgroundColor(dark)))
            .add_child(fill)
            .id();
        // the pip: a small diamond pointing down at them
        let pip = commands
            .spawn((
                Node { width: px(10), height: px(10), margin: UiRect::top(px(5)), border: UiRect::all(px(2)), ..default() },
                BackgroundColor(color),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.9)),
                UiTransform::from_rotation(Rot2::degrees(45.0)),
            ))
            .id();
        commands
            .spawn((
                TeamId { id: o.id, talk, mic: mic_e, bars, name: name_e, track, fill, pip },
                Node { position_type: PositionType::Absolute, width: px(WIDTH), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() },
                Visibility::Hidden,
                DespawnOnExit(AppState::InGame),
            ))
            .add_children(&[row, track, pip]);
    }
}
