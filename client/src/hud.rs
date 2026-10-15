//! The in-game HUD: health / armor, ammo, grenades, kill feed, the scoreboard (Tab),
//! a crosshair that opens up with the weapon's real spread, and the sniper scope.
use crate::game::{Frame, Walker};
use crate::level::rgba_image;
use crate::menu::AppState;
use crate::ui::{panel, text, CT_COLOR, MUTED, TEXT, T_COLOR};
use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use net::protocol::Team;
use sim::weapons::Id;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(FrameTimeDiagnosticsPlugin::default())
            .add_systems(OnEnter(AppState::InGame), spawn)
            .add_systems(Update, (status, crosshair, feed, scoreboard, net_stats).in_set(Frame::Ui));
    }
}

const HUD: Color = Color::srgb(1.0, 0.82, 0.35);
const CROSSHAIR: Color = Color::srgb(0.25, 1.0, 0.25);

#[derive(Component)]
struct Health;
#[derive(Component)]
struct Armor;
#[derive(Component)]
struct Ammo;
#[derive(Component)]
struct WeaponName;
#[derive(Component)]
struct Grenades;
#[derive(Component)]
struct Notice;
#[derive(Component)]
struct NetStats;
#[derive(Component)]
struct FeedRoot;
#[derive(Component)]
struct ScoresRoot;
#[derive(Component)]
struct ScopeRoot;
#[derive(Component)]
struct CrosshairRoot;
/// One crosshair line: (x, y) direction away from the centre.
#[derive(Component)]
struct Line(f32, f32);

fn pos(node: Node) -> Node {
    Node { position_type: PositionType::Absolute, ..node }
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let game = DespawnOnExit(AppState::InGame);
    // Bottom left: health and armor.
    commands.spawn((game.clone(), pos(Node { left: px(24), bottom: px(18), column_gap: px(28), align_items: AlignItems::Baseline, ..default() }))).with_children(|row| {
        row.spawn(text("HP", 16.0, MUTED));
        row.spawn((Health, text("", 40.0, HUD)));
        row.spawn(text("AP", 16.0, MUTED));
        row.spawn((Armor, text("", 40.0, HUD)));
    });
    // Bottom right: weapon, grenades, ammo.
    commands
        .spawn((game.clone(), pos(Node { right: px(24), bottom: px(18), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, ..default() })))
        .with_children(|col| {
            col.spawn((WeaponName, text("", 16.0, MUTED)));
            col.spawn((Grenades, text("", 16.0, MUTED)));
            col.spawn((Ammo, text("", 40.0, HUD)));
        });
    // Bottom centre: what's going on (spectating, dead).
    commands.spawn((game.clone(), pos(Node { width: percent(100), bottom: px(90), justify_content: JustifyContent::Center, ..default() }))).with_children(|row| {
        row.spawn((Notice, text("", 20.0, TEXT)));
    });
    // Top left: frame rate and ping (the radar will sit above it).
    commands.spawn((NetStats, game.clone(), text("", 14.0, MUTED), pos(Node { left: px(12), top: px(10), ..default() })));
    // Top right: kill feed.
    commands.spawn((FeedRoot, game.clone(), pos(Node { right: px(16), top: px(12), flex_direction: FlexDirection::Column, align_items: AlignItems::FlexEnd, row_gap: px(4), ..default() })));
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

fn crosshair(
    w: Res<Walker>,
    windows: Query<&Window>,
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
    root.set_if_neq(if alive && !is_sniper(id) { Visibility::Inherited } else { Visibility::Hidden });
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

#[allow(clippy::type_complexity)]
fn status(
    w: Res<Walker>,
    mut health: Single<&mut Text, (With<Health>, Without<Armor>, Without<Ammo>, Without<WeaponName>, Without<Grenades>, Without<Notice>)>,
    mut armor: Single<&mut Text, (With<Armor>, Without<Ammo>, Without<WeaponName>, Without<Grenades>, Without<Notice>)>,
    mut ammo: Single<&mut Text, (With<Ammo>, Without<WeaponName>, Without<Grenades>, Without<Notice>)>,
    mut weapon: Single<&mut Text, (With<WeaponName>, Without<Grenades>, Without<Notice>)>,
    mut nades: Single<&mut Text, (With<Grenades>, Without<Notice>)>,
    mut notice: Single<&mut Text, With<Notice>>,
) {
    let v = w.client.vitals;
    let l = &w.client.state.loadout;
    let alive = v.alive() && w.client.id().is_some();
    let show = |s: String| if alive { s } else { String::new() };
    set(&mut health, show(format!("{:.0}", v.health)));
    set(&mut armor, show(format!("{:.0}{}", v.armor, if v.helmet { "+H" } else { "" })));
    set(&mut ammo, show(l.active_gun().map(|g| format!("{} | {}", g.clip, g.reserve)).unwrap_or_default()));
    set(&mut weapon, show(sim::weapons::def(l.active_id()).name.to_uppercase()));
    let [he, fl, sm] = l.grenades;
    let names = [("HE", he), ("FLASH", fl), ("SMOKE", sm)];
    set(&mut nades, show(names.iter().filter(|(_, n)| *n > 0).map(|(name, n)| if *n > 1 { format!("{name} x{n}") } else { name.to_string() }).collect::<Vec<_>>().join("  ")));
    let n = match w.client.team() {
        _ if w.client.id().is_none() => "connecting...".into(),
        Team::Spectator => "Spectating  -  press M to choose a team".into(),
        _ if !v.alive() => if w.client.practice { "Respawning..." } else { "You are dead" }.into(),
        _ => String::new(),
    };
    set(&mut notice, n);
}

/// Change a text only when it really changes (a write re-lays out the text every frame).
fn set(t: &mut Mut<Text>, s: String) {
    if t.0 != s {
        t.0 = s;
    }
}

/// Kill feed lines, newest at the bottom.
fn feed(mut commands: Commands, w: Res<Walker>, root: Single<(Entity, Option<&Children>), With<FeedRoot>>, mut shown: Local<Vec<String>>) {
    let lines: Vec<String> = w.feed.iter().map(|(l, _)| l.clone()).collect();
    if *shown == lines {
        return;
    }
    *shown = lines.clone();
    let (root, children) = *root;
    for &c in children.into_iter().flatten() {
        commands.entity(c).despawn();
    }
    commands.entity(root).with_children(|col| {
        for l in lines {
            col.spawn(panel(Node { padding: UiRect::axes(px(10), px(4)), ..default() })).with_children(|p| {
                p.spawn(text(l, 16.0, TEXT));
            });
        }
    });
}

/// Tab: both teams' players with kills and deaths while held.
fn scoreboard(mut commands: Commands, keys: Res<ButtonInput<KeyCode>>, w: Res<Walker>, roots: Query<Entity, With<ScoresRoot>>, mut shown: Local<Option<Vec<net::protocol::Score>>>) {
    let want = keys.pressed(KeyCode::Tab).then(|| w.client.scores.clone());
    if *shown == want {
        return;
    }
    *shown = want.clone();
    for e in &roots {
        commands.entity(e).despawn();
    }
    let Some(scores) = want else { return };
    let me = w.client.id();
    commands
        .spawn((ScoresRoot, DespawnOnExit(AppState::InGame), pos(Node { width: percent(100), top: percent(12), justify_content: JustifyContent::Center, ..default() }), GlobalZIndex(4)))
        .with_children(|root| {
            root.spawn(panel(Node { flex_direction: FlexDirection::Column, padding: UiRect::all(px(20)), min_width: px(640), row_gap: px(4), ..default() })).with_children(|p| {
                for (team, name, color) in [(Team::Ct, "Counter-Terrorists", CT_COLOR), (Team::T, "Terrorists", T_COLOR), (Team::Spectator, "Spectators", MUTED)] {
                    let players: Vec<_> = scores.iter().filter(|s| s.team == team).collect();
                    if players.is_empty() && team == Team::Spectator {
                        continue;
                    }
                    p.spawn((text(format!("{name}  ({})", players.len()), 20.0, color), Node { margin: UiRect::top(px(8)), ..default() }));
                    for s in players {
                        let c = if Some(s.id) == me { TEXT } else if s.alive { color.mix(&TEXT, 0.4) } else { MUTED };
                        p.spawn(Node { flex_direction: FlexDirection::Row, ..default() }).with_children(|r| {
                            let name = format!("{}{}{}", s.name, if s.bot { " (bot)" } else { "" }, if s.alive || team == Team::Spectator { "" } else { "  dead" });
                            r.spawn((text(name, 17.0, c), Node { width: px(420), ..default() }));
                            r.spawn((text(format!("{}", s.kills), 17.0, c), Node { width: px(80), ..default() }));
                            r.spawn((text(format!("{}", s.deaths), 17.0, c), Node { width: px(80), ..default() }));
                        });
                    }
                }
            });
        });
}

/// "120 fps  25 ms", refreshed a few times a second so it's readable.
fn net_stats(w: Res<Walker>, diag: Res<DiagnosticsStore>, time: Res<Time>, mut next: Local<f32>, mut text: Single<&mut Text, With<NetStats>>) {
    if time.elapsed_secs() < *next {
        return;
    }
    *next = time.elapsed_secs() + 0.25;
    let fps = diag.get(&FrameTimeDiagnosticsPlugin::FPS).and_then(|d| d.smoothed()).unwrap_or(0.0);
    set(&mut text, format!("{fps:.0} fps   {:.0} ms", w.client.ping_ms));
}
