//! The crosshair, which opens up with the weapon's real spread, and the sniper scope.
use crate::game::Walker;
use crate::level::rgba_image;
use crate::menu::AppState;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use sim::weapons::Id;

const CROSSHAIR: Color = Color::srgb(0.25, 1.0, 0.25);

#[derive(Component)]
pub(super) struct ScopeRoot;
#[derive(Component)]
pub(super) struct CrosshairRoot;
/// One crosshair line: (x, y) direction away from the centre.
#[derive(Component)]
pub(super) struct Line(f32, f32);

fn pos(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

pub(super) fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let game = DespawnOnExit(AppState::InGame);
    // Crosshair: four lines around the centre.
    commands
        .spawn((CrosshairRoot, game.clone(), pos(Node { left: percent(50), top: percent(50), ..default() })))
        .with_children(|c| {
            for (x, y) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                c.spawn((Line(x, y), pos(Node::default()), BackgroundColor(CROSSHAIR)));
            }
        });
    // Sniper scope: a round view with fine cross lines, black everywhere else.
    let scope = images.add(scope_image(512));
    commands
        .spawn((ScopeRoot, game, Visibility::Hidden, pos(Node { width: percent(100), height: percent(100), ..default() }), GlobalZIndex(1)))
        .with_children(|s| {
            s.spawn((Node { flex_grow: 1.0, ..default() }, BackgroundColor(Color::BLACK)));
            s.spawn((ImageNode::new(scope), Node { height: percent(100), aspect_ratio: Some(1.0), ..default() }));
            s.spawn((Node { flex_grow: 1.0, ..default() }, BackgroundColor(Color::BLACK)));
        });
}

/// Black with a clear circle and thin cross lines through the middle.
fn scope_image(n: u32) -> Image {
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let r = (dx * dx + dy * dy).sqrt() / c;
            let line = dx.abs() < 0.8 || dy.abs() < 0.8;
            // Soft edge on the circle so it doesn't look jagged.
            let a = if line { 1.0 } else { ((r - 0.985) / 0.015).clamp(0.0, 1.0) };
            rgba[((y * n + x) * 4 + 3) as usize] = (a * 255.0) as u8;
        }
    }
    rgba_image(n, n, rgba, false, TextureFormat::Rgba8UnormSrgb)
}

/// Snipers only have a crosshair when scoped (in the scope itself).
fn is_sniper(id: Id) -> bool {
    matches!(id, Id::Awp | Id::Scout | Id::G3sg1 | Id::Sg550)
}

pub(super) fn crosshair(
    w: Res<Walker>,
    windows: Query<&Window>,
    covered: Res<crate::ui::Covered>,
    mut root: Single<&mut Visibility, (With<CrosshairRoot>, Without<ScopeRoot>)>,
    mut scope: Single<&mut Visibility, (With<ScopeRoot>, Without<CrosshairRoot>)>,
    mut lines: Query<(&Line, &mut Node)>,
) {
    let me = w.client.me();
    let l = &w.client.state.loadout;
    let alive = w.client.vitals.alive() && w.client.id().is_some();
    let id = l.active_id();
    let scoped = l.active_gun().is_some_and(|g| g.fov() < 90) && is_sniper(id);
    scope.set_if_neq(if alive && scoped { Visibility::Inherited } else { Visibility::Hidden });
    root.set_if_neq(if alive && !is_sniper(id) && !covered.0 { Visibility::Inherited } else { Visibility::Hidden });
    // The gap is the real spread of the next shot, seen at this screen size: spread is
    // the tangent of the cone, and the view is 90° wide at 4:3 (vertical half-angle 36.87°).
    let h = windows.iter().next().map(|w| w.height()).unwrap_or(720.0);
    let speed = (me.velocity[0].powi(2) + me.velocity[1].powi(2)).sqrt();
    let spread = l.spread_now(me.on_ground, me.crouched, speed).unwrap_or(0.0);
    let gap = (spread * (h / 2.0) / 0.75).clamp(3.0, h / 4.0).round();
    let (len, thick) = (10.0, 2.0);
    for (Line(x, y), mut n) in &mut lines {
        let (w, hh) = if *x != 0.0 { (len, thick) } else { (thick, len) };
        // Each line starts `gap` from the centre, pointing away from it.
        let left = px(if *x > 0.0 { gap } else if *x < 0.0 { -gap - len } else { -thick / 2.0 });
        let top = px(if *y > 0.0 { gap } else if *y < 0.0 { -gap - len } else { -thick / 2.0 });
        // Only touch the layout when the gap moved.
        if n.left != left || n.top != top || n.width != px(w) {
            n.width = px(w);
            n.height = px(hh);
            n.left = left;
            n.top = top;
        }
    }
}

