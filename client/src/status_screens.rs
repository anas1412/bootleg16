//! Loading, notices and reconnecting (design/mockups/ui: Loading, Notice, Reconnecting).
//! The logic lives in `loading`, `notice` and `game`; these views only read their resources:
//! each view is up while its resource exists and follows it every frame.
//! - Loading (Connecting, and into the game until the first frame is drawn): the map big, the steps in a row (done, now, next), real
//!   progress, Cancel · Esc.
//! - Notice (menu): over everything, blurred behind: what happened, why, Retry and Back.
//! - Reconnecting (in a match): under the score while the server is quiet, draining to the
//!   moment we give up.
use crate::game::Reconnecting;
use crate::loading::{Loading, Step};
use crate::menu::AppState;
use crate::notice::{Notice, NoticeKind};
use crate::ui::{chip, edge, hud_panel, kit_text, lift, meter, primary_button, soft, t, vh, Anim, EdgeColor, HudLayer, Kit, Meter, Type, UiAction, UiSound, AMBER, BACKDROP, BAD, BG0, BG1, LINE_2, TX1, TX2, TX3};
use bevy::prelude::*;

pub struct StatusScreensPlugin;

impl Plugin for StatusScreensPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (loading, notice.run_if(in_state(AppState::Menu)), reconnecting.run_if(in_state(AppState::InGame))));
    }
}

#[derive(Component)]
struct LoadingRoot;
#[derive(Component)]
struct LoadingBar;
#[derive(Component)]
enum LoadingText {
    What,
    Map,
    Percent,
}
/// One step: its index, and the diamond in front of it.
#[derive(Component)]
struct StepRow(usize, Entity);

fn loading(mut commands: Commands, kit: Res<Kit>, state: Option<Res<Loading>>, root: Query<Entity, With<LoadingRoot>>, mut texts: Query<(&LoadingText, &mut Text)>, mut rows: Query<(&StepRow, &mut Node, &Children)>, mut colors: Query<&mut TextColor>, mut fills: Query<&mut BackgroundColor>, mut bar: Query<&mut Meter, With<LoadingBar>>, time: Res<Time>) {
    let Some(l) = state else {
        // the game is drawn under it by now: tune out over it
        for e in &root {
            commands.entity(e).remove::<LoadingRoot>().insert(Anim::tune_out());
        }
        return;
    };
    // practice starts its own server; joining skips that step
    let steps: Vec<Step> = Step::ALL.into_iter().filter(|s| l.practice || *s != Step::StartingServer).collect();
    if root.is_empty() {
        commands
            .spawn((LoadingRoot, Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() }, BackgroundColor(BG0), GlobalZIndex(40)))
            .with_children(|full| {
                full.spawn((Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: vh(380.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, ..default() }, Anim::tune_in(0.0))).with_children(|c| {
                    c.spawn((LoadingText::What, t(&kit, Type::Label, "", TX2)));
                    c.spawn((LoadingText::Map, kit_text("", 120.0, TX1, &kit.display), Node { margin: UiRect::top(vh(12.0)), ..default() }));
                    c.spawn(Node { column_gap: vh(32.0), margin: UiRect::top(vh(40.0)), ..default() }).with_children(|r| {
                        for (i, s) in steps.iter().enumerate() {
                            let dot = r.commands().spawn((Node { width: vh(8.0), height: vh(8.0), border: UiRect::all(px(1)), ..default() }, BorderColor::all(TX3), BackgroundColor(Color::NONE), UiTransform::from_rotation(Rot2::degrees(45.0)))).id();
                            r.spawn(Node { align_items: AlignItems::Center, column_gap: vh(10.0), ..default() }).add_child(dot).with_children(|row| {
                                row.spawn(t(&kit, Type::Label, s.label(), TX3));
                            }).insert(StepRow(i, dot));
                        }
                    });
                    let m = meter(c, 640.0, AMBER, 0.0);
                    c.commands().entity(m).insert(LoadingBar).entry::<Node>().and_modify(|mut n| n.margin = UiRect::top(vh(32.0)));
                    c.spawn((LoadingText::Percent, kit_text("0%", 16.0, TX2, &kit.num), Node { margin: UiRect::top(vh(10.0)), ..default() }));
                });
                full.spawn(Node { position_type: PositionType::Absolute, left: vh(96.0), bottom: vh(48.0), ..default() }).with_children(|b| {
                    chip(b, &kit, "Cancel · Esc", UiAction::CancelLoading, false);
                });
            });
        return;
    }
    let what = if l.practice { "Practice · solo".to_string() } else { format!("Match · {}", l.target) };
    for (which, mut text) in &mut texts {
        let s = match which {
            LoadingText::What => what.to_uppercase(),
            LoadingText::Map => l.map.clone().unwrap_or_else(|| if l.practice { l.target.clone() } else { "...".into() }).to_uppercase(),
            LoadingText::Percent => format!("{:.0}%", l.progress.clamp(0.0, 1.0) * 100.0),
        };
        if text.0 != s {
            text.0 = s;
        }
    }
    for mut m in &mut bar {
        m.value = l.progress.clamp(0.0, 1.0);
    }
    // done steps grey, the current one amber and pulsing, the rest quiet
    let now = steps.iter().position(|s| *s == l.step).unwrap_or(0);
    let pulse = 0.65 + 0.35 * (time.elapsed_secs() * std::f32::consts::TAU / 1.2).cos();
    for (row, _, kids) in &mut rows {
        let c = match row.0.cmp(&now) {
            std::cmp::Ordering::Less => TX2,
            std::cmp::Ordering::Equal => AMBER,
            std::cmp::Ordering::Greater => TX3,
        };
        for k in kids {
            if let Ok(mut tc) = colors.get_mut(*k) {
                tc.set_if_neq(TextColor(c));
            }
        }
        if let Ok(mut bg) = fills.get_mut(row.1) {
            let fill = match row.0.cmp(&now) {
                std::cmp::Ordering::Less => TX2,
                std::cmp::Ordering::Equal => AMBER.with_alpha(pulse),
                std::cmp::Ordering::Greater => Color::NONE,
            };
            bg.set_if_neq(BackgroundColor(fill));
        }
    }
}

#[derive(Component)]
struct NoticeRoot;

/// The notice over the menu: rebuilt when it changes, tuned out when it's closed.
fn notice(mut commands: Commands, kit: Res<Kit>, state: Option<Res<Notice>>, root: Query<Entity, With<NoticeRoot>>, mut sounds: MessageWriter<UiSound>) {
    let changed = state.as_ref().is_some_and(|n| n.is_changed());
    if state.is_none() || changed {
        for e in &root {
            commands.entity(e).despawn();
        }
    }
    let Some(n) = state else { return };
    if !changed && !root.is_empty() {
        return;
    }
    sounds.write(UiSound::On);
    let error = matches!(n.kind, NoticeKind::Error);
    commands
        .spawn((
            NoticeRoot,
            // like match found: flat over the 3D menu, which takes no clicks under it
            crate::found::FoundOverlay,
            DespawnOnExit(AppState::Menu),
            Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
            BackgroundColor(BACKDROP),
            GlobalZIndex(30),
        ))
        .with_children(|full| {
            full.spawn((soft(Node { width: vh(720.0), padding: UiRect::axes(vh(48.0), vh(40.0)), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, border: UiRect::all(px(1)), ..default() }), BackgroundColor(BG1.with_alpha(0.96)), BorderColor::all(LINE_2), lift(), Anim::tune_in(0.0))).with_children(|p| {
                p.spawn(t(&kit, Type::Label, if error { "Error" } else { "Notice" }, if error { BAD } else { TX2 }));
                p.spawn((kit_text(n.title.to_uppercase(), 72.0, TX1, &kit.display), Node { margin: UiRect::top(vh(12.0)), ..default() }));
                p.spawn((kit_text(n.body.clone(), 18.0, TX2, &kit.text), TextLayout::justify(Justify::Center), Node { max_width: vh(560.0), margin: UiRect::top(vh(16.0)), ..default() }));
                p.spawn(Node { flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(12.0), margin: UiRect::top(vh(36.0)), ..default() }).with_children(|b| {
                    if n.retry {
                        let r = primary_button(b, &kit, "Retry", "Enter", UiAction::NoticeRetry);
                        b.commands().entity(r).entry::<Node>().and_modify(|mut n| n.min_width = vh(420.0));
                        chip(b, &kit, "Back · Esc", UiAction::NoticeClose, false);
                    } else {
                        let r = primary_button(b, &kit, "Back", "Enter", UiAction::NoticeClose);
                        b.commands().entity(r).entry::<Node>().and_modify(|mut n| n.min_width = vh(420.0));
                    }
                });
            });
        });
}

#[derive(Component)]
struct ReconnectRoot;
#[derive(Component)]
struct ReconnectSecs;
#[derive(Component)]
struct ReconnectBar;

/// Under the score while the server is quiet: how long, and how long until we give up.
fn reconnecting(mut commands: Commands, kit: Res<Kit>, state: Option<Res<Reconnecting>>, root: Query<Entity, With<ReconnectRoot>>, mut secs: Query<&mut Text, With<ReconnectSecs>>, mut bar: Query<&mut Meter, With<ReconnectBar>>) {
    let Some(r) = state else {
        for e in &root {
            commands.entity(e).remove::<ReconnectRoot>().insert(Anim::tune_out());
        }
        return;
    };
    let left = 1.0 - (r.since / r.give_up.max(1.0)).clamp(0.0, 1.0);
    if root.is_empty() {
        commands
            .spawn((ReconnectRoot, HudLayer, DespawnOnExit(AppState::InGame), Node { position_type: PositionType::Absolute, width: percent(100), top: vh(190.0), justify_content: JustifyContent::Center, ..default() }, Anim::tune_in(0.0)))
            .with_children(|row| {
                row.spawn(hud_panel(Node { width: vh(420.0), flex_direction: FlexDirection::Column, align_items: AlignItems::Center, row_gap: vh(8.0), padding: UiRect { left: vh(24.0), right: vh(24.0), top: vh(16.0), bottom: vh(14.0) }, ..default() })).with_children(|p| {
                    p.spawn(edge(EdgeColor::Fixed(BAD), 1.0));
                    p.spawn(t(&kit, Type::Label, "Connection interrupted", BAD));
                    p.spawn((ReconnectSecs, kit_text("", 20.0, TX1, &kit.text)));
                    let m = meter(p, 372.0, BAD, left);
                    p.commands().entity(m).insert(ReconnectBar).entry::<Node>().and_modify(|mut n| n.margin = UiRect::top(vh(2.0)));
                    p.spawn(t(&kit, Type::Label, "Esc · leave match", TX3));
                });
            });
        return;
    }
    let s = format!("Reconnecting  {:.0} s", r.since.floor());
    for mut t in &mut secs {
        if t.0 != s {
            t.0 = s.clone();
        }
    }
    for mut m in &mut bar {
        m.value = left;
    }
}
