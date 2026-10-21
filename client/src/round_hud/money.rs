//! Money, top left under the radar: it counts to its new value, the change floats up beside
//! it and fades; the status icons on the right (defuse kit, buy zone, bomb) open and close in
//! place with a soft blip. The bomb icon turns red in a bomb site.
use super::{abs, set};
use crate::game::Walker;
use crate::hud::{Icons, SAFE};
use crate::menu::AppState;
use crate::ui::{edge, hud_panel, kit_text, vh, Anim, Count, Edge, EdgeColor, HudLayer, Kit, Opens, UiSound, EDGE_REST, OK, TX1, TX2};
use bevy::prelude::*;

#[derive(Component)]
pub(super) struct MoneyPanel;
#[derive(Component)]
pub(super) struct MoneyEdge;
#[derive(Component)]
pub(super) struct Money;
#[derive(Component)]
pub(super) struct Delta;
/// A status icon: which one.
#[derive(Component, Clone, Copy, PartialEq)]
pub(super) enum Status {
    Buy,
}

/// An icon's slot: 24 wide plus the 8 gap before it.
const SLOT: f32 = 32.0;
/// The change floats for this long.
const T_DELTA: f32 = 1.3;

pub(super) fn spawn(mut commands: Commands, kit: Res<Kit>, icons: Res<Icons>) {
    commands
        .spawn((MoneyPanel, HudLayer, DespawnOnExit(AppState::InGame), hud_panel(abs(Node { left: vh(SAFE), top: vh(SAFE + crate::radar::SIZE + 8.0), width: vh(256.0), height: vh(48.0), align_items: AlignItems::Center, padding: UiRect::horizontal(vh(16.0)), column_gap: vh(8.0), ..default() }))))
        .with_children(|p| {
            p.spawn((MoneyEdge, edge(EdgeColor::Team, EDGE_REST)));
            p.spawn(kit_text("$", 26.0, OK, &kit.num));
            p.spawn((Money, Count::new(0.0, 0.14), kit_text("0", 26.0, TX1, &kit.num)));
            p.spawn((Delta, kit_text("", 16.0, OK, &kit.num), UiTransform::default()));
            p.spawn(Node { margin: UiRect::left(Val::Auto), align_items: AlignItems::Center, ..default() }).with_children(|row| {
                // the kit and the C4 are in the carried strip over the weapon panel (hud/owned.rs)
                for (s, icon, color) in [(Status::Buy, "buyzone", OK)] {
                    row.spawn((s, Opens::new(false, SLOT), Node { width: px(0), height: vh(24.0), justify_content: JustifyContent::FlexEnd, overflow: Overflow::clip(), ..default() })).with_children(|slot| {
                        slot.spawn(icons.node(icon, 24.0, color)).entry::<Node>().and_modify(|mut n| n.flex_shrink = 0.0);
                    });
                }
            });
        });
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn money(
    w: Res<Walker>,
    time: Res<Time>,
    mut last: Local<Option<(i32, f32, i32)>>,
    mut panel: Single<&mut Visibility, With<MoneyPanel>>,
    mut count: Single<&mut Count, With<Money>>,
    delta: Single<(&mut Text, &mut TextColor, &mut UiTransform), With<Delta>>,
    mut edge: Single<&mut Edge, With<MoneyEdge>>,
    mut slots: Query<(Entity, &Status, &mut Opens)>,
    mut sounds: MessageWriter<UiSound>,
    mut commands: Commands,
    covered: Res<crate::ui::Covered>,
) {
    let alive = w.client.vitals.alive() && w.client.id().is_some();
    let now = time.elapsed_secs();
    let money = w.client.me_info.money;
    // hidden under a menu: the buy menu shows your funds itself
    panel.set_if_neq(if w.client.practice || covered.0 { Visibility::Hidden } else { Visibility::Inherited });
    count.target = money as f32;
    match *last {
        Some((before, _, _)) if before != money => {
            *last = Some((money, now, money - before));
            edge.flare();
        }
        None => *last = Some((money, -10.0, 0)),
        _ => {}
    }
    // the change: in from below, held, then up and out
    let (_, at, d) = last.unwrap_or((0, -10.0, 0));
    let (text, color, tr) = delta.into_inner();
    let t = (now - at) / T_DELTA;
    if (0.0..1.0).contains(&t) && d != 0 {
        let mut text = text;
        set(&mut text, if d > 0 { format!("+{d}") } else { format!("−{}", -d) });
        let (y, a) = if t < 0.15 { (8.0 * (1.0 - t / 0.15), t / 0.15) } else if t < 0.7 { (0.0, 1.0) } else { (-10.0 * (t - 0.7) / 0.3, 1.0 - (t - 0.7) / 0.3) };
        color.into_inner().0 = if d > 0 { OK } else { TX2 }.with_alpha(a);
        tr.into_inner().translation = Val2::new(px(0), vh(y));
    } else {
        let mut text = text;
        set(&mut text, String::new());
    }
    // where you can buy
    for (e, s, mut o) in &mut slots {
        let want = alive
            && match s {
                Status::Buy => w.client.me_info.can_buy,
            };
        if o.open != want {
            o.open = want;
            if want {
                commands.entity(e).insert(Anim::burst());
            }
            sounds.write(if want { UiSound::On } else { UiSound::Off });
        }
    }
}
