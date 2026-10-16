//! Surfaces: softened corners, lit from above, sitting at a height (`elevation`). The anchor rule: a part on a screen edge mirrors to it,
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

/// How high a surface sits (light from above): 1 panels and rows, 2 raised (toasts, chosen
/// cards), 3 overlays. Each height is the slab's own edge (solid, a few px under it, so it reads
/// on any background) and its soft shadow, farther the higher it sits (tokens.css --e1..3).
pub fn elevation(h: u8) -> BoxShadow {
    let (edge, near, far, far_a) = match h {
        0 | 1 => (3.0, 10.0, 30.0, 0.30),
        2 => (5.0, 16.0, 52.0, 0.42),
        _ => (7.0, 24.0, 90.0, 0.58),
    };
    let s = |y: f32, blur: f32, a: f32| ShadowStyle { color: Color::srgba(0.0, 0.0, 0.0, a), x_offset: px(0), y_offset: vh(y), spread_radius: px(0), blur_radius: vh(blur) };
    BoxShadow(vec![
        ShadowStyle { color: Color::srgba(0.012, 0.016, 0.024, 0.55), x_offset: px(0), y_offset: vh(edge), spread_radius: px(0), blur_radius: px(0) },
        s(edge * 2.0, near, 0.25),
        s(edge * 4.0, far, far_a),
    ])
}

/// A panel's height off what's behind it (`elevation(1)`).
pub fn lift() -> BoxShadow {
    elevation(1)
}

/// The light from above on a surface: a touch lighter at the top, a touch darker at the
/// bottom, drawn over its fill (tokens.css --lit).
pub fn lit() -> BackgroundGradient {
    BackgroundGradient::from(LinearGradient::to_bottom(vec![
        ColorStop::percent(Color::srgba(0.918, 0.949, 0.973, 0.05), 0.0),
        ColorStop::percent(Color::srgba(0.918, 0.949, 0.973, 0.0), 45.0),
        ColorStop::percent(Color::srgba(0.0, 0.0, 0.0, 0.14), 100.0),
    ]))
}

/// A panel drawn over the game: dark glass, softened, lit, lifted, tinted by your side.
pub fn hud_panel(node: Node) -> impl Bundle {
    (soft(node), BackgroundColor(HUD_BG), lit(), lift(), Tint::Team)
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
        lit(),
        lift(),
        if you { Tint::You } else { Tint::Team },
    )
}

/// A card: a choice with content (buy items, party slots, sides). Selected cards wear the
/// amber frame on an amber wash; the others a quiet frame.
pub fn card(node: Node, selected: bool) -> impl Bundle {
    let border = UiRect::all(vh(if selected { 2.0 } else { 1.0 }));
    (soft(Node { border, ..node }), BackgroundColor(if selected { BG1.mix(&AMBER, 0.08) } else { BG1 }), BorderColor::all(if selected { AMBER } else { LINE }), lit(), elevation(if selected { 2 } else { 1 }))
}
