//! Surfaces: softened corners, a soft shadow that lifts them. The anchor rule: a part on a screen edge mirrors to it,
//! so its content and its amber bar (you, or the main action) sit on the outer side; each
//! screen applies it with `UiRect::left` / `right(vh(BAR))`.
use super::kit::vh;
use super::motion::Tint;
use super::tokens::{AMBER, BAR, BG1, GAP, HUD_BG, LINE, RADIUS, RADIUS_S};
use bevy::prelude::*;

/// Softened corners on any node.
pub fn soft(node: Node) -> Node {
    Node { border_radius: BorderRadius::all(vh(RADIUS)), ..node }
}

/// The soft shadow that lifts a panel off what's behind it.
pub fn lift() -> BoxShadow {
    BoxShadow(vec![ShadowStyle { color: Color::srgba(0.0, 0.0, 0.0, 0.35), x_offset: px(0), y_offset: vh(4.0), spread_radius: px(0), blur_radius: vh(18.0) }])
}

/// A panel drawn over the game: dark glass, softened, lifted, tinted by your side.
pub fn hud_panel(node: Node) -> impl Bundle {
    (soft(node), BackgroundColor(HUD_BG), lift(), Tint::Team)
}

/// Which side of the screen a part hugs (where its amber bar goes).
#[derive(Clone, Copy, PartialEq)]
pub enum Outer {
    Left,
    Right,
}

/// A feed row (kill feed, voices): HUD glass with the small corners. Yours is tinted amber
/// and wears the amber bar on its outer side, inside the same padding, so its content lines
/// up with the other rows'.
pub fn feed_row(height: f32, you: bool, outer: Outer) -> impl Bundle {
    let bar = if you { vh(BAR) } else { px(0) };
    let pad = |on: bool| vh(if on && you { GAP - BAR } else { GAP });
    let (border, padding) = match outer {
        Outer::Left => (UiRect::left(bar), UiRect { left: pad(true), right: pad(false), ..default() }),
        Outer::Right => (UiRect::right(bar), UiRect { left: pad(false), right: pad(true), ..default() }),
    };
    (
        Node { height: vh(height), align_items: AlignItems::Center, column_gap: vh(12.0), padding, border, border_radius: BorderRadius::all(vh(RADIUS_S)), ..default() },
        BackgroundColor(HUD_BG),
        BorderColor::all(AMBER),
        lift(),
        if you { Tint::You } else { Tint::Team },
    )
}

/// A card: a choice with content (buy items, party slots, sides). Selected cards wear the
/// amber frame on an amber wash; the others a quiet frame.
pub fn card(node: Node, selected: bool) -> impl Bundle {
    let border = UiRect::all(vh(if selected { 2.0 } else { 1.0 }));
    (soft(Node { border, ..node }), BackgroundColor(if selected { BG1.mix(&AMBER, 0.08) } else { BG1 }), BorderColor::all(if selected { AMBER } else { LINE }), lift())
}
