//! Menus over the game: the game blurs (the 3D camera's depth of field), the shared scrim
//! dims it, a scanline sweeps once, and the HUD hides; closing reverses it.
use super::ease;
use crate::ui::tokens::*;
use bevy::post_process::dof::{DepthOfField, DepthOfFieldMode};
use bevy::prelude::*;

/// Put on the root of a menu or popup drawn over the game, with how dark the scrim gets.
#[derive(Component)]
pub struct Blurs(pub f32);

impl Default for Blurs {
    fn default() -> Self {
        Blurs(BACKDROP.alpha())
    }
}

/// True while a menu covers the game (the HUD hides under it).
#[derive(Resource, Default)]
pub struct Covered(pub bool);

/// HUD roots hidden while a menu covers the game (`Covered`). Parts that manage their own
/// visibility read `Covered` instead.
#[derive(Component)]
pub struct HudLayer;

// ---- menus over the game ------------------------------------------------------------------
/// The one full-screen dim behind every menu over the game (under them, over the HUD).
#[derive(Component)]
struct Scrim;
/// The scanline that sweeps down once when a menu opens.
#[derive(Component)]
struct Sweep;

fn spawn_scrim(mut commands: Commands) {
    let full = Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() };
    commands.spawn((Scrim, full, BackgroundColor(Color::NONE), GlobalZIndex(Z_SCRIM), Pickable::IGNORE));
    commands.spawn((
        Sweep,
        Node { position_type: PositionType::Absolute, width: percent(100), height: px(2), top: percent(0), ..default() },
        BackgroundColor(Color::NONE),
        GlobalZIndex(Z_SCRIM),
        Pickable::IGNORE,
    ));
}

/// How covered the game is (0..1), the scrim's darkness, and when the last sweep started.
#[derive(Resource, Default)]
struct BlurLevel(f32, f32, Option<f32>, bool);

/// The 3D camera's depth of field doubles as a full-screen blur: everything is out of focus
/// and the strength is the largest blur size, eased with the level.
#[allow(clippy::type_complexity)]
fn blur(
    time: Res<Time>,
    mut level: ResMut<BlurLevel>,
    wanted: Query<&Blurs>,
    mut scrim: Query<&mut BackgroundColor, (With<Scrim>, Without<Sweep>)>,
    mut sweep: Query<(&mut Node, &mut BackgroundColor), With<Sweep>>,
    mut hud: Query<&mut Visibility, With<HudLayer>>,
    mut covered_flag: ResMut<Covered>,
    mut commands: Commands,
    // the world's camera only: the weapon's camera draws into the same picture, and depth of
    // field on it breaks frames on resize (the weapon stays sharp under menus)
    mut cams: Query<(Entity, Option<&mut DepthOfField>), (With<Camera3d>, Without<crate::viewmodel::ViewModelCamera>)>,
    window: Single<&Window, With<bevy::window::PrimaryWindow>>,
    (mut size, mut settle): (Local<(u32, u32)>, Local<(f32, u32)>),
) {
    let now = time.elapsed_secs();
    // Depth of field breaks the frame while the window changes size (a render error that
    // quits the game), so it steps aside until the size has settled.
    let now_size = (window.physical_width(), window.physical_height());
    if now_size != *size {
        *size = now_size;
        *settle = (now + SETTLE, SETTLE_FRAMES);
    }
    // time and frames both: one slow frame (loading) can eat the whole wait
    settle.1 = settle.1.saturating_sub(1);
    let steady = now >= settle.0 && settle.1 == 0;
    let covered = !wanted.is_empty();
    if covered && !level.3 {
        level.2 = Some(now);
    }
    level.3 = covered;
    if let Some(dim) = wanted.iter().map(|b| b.0).reduce(f32::max) {
        level.1 = dim;
    }
    let target = if covered { 1.0 } else { 0.0 };
    let k = ease(time.delta_secs(), T_BLUR / 3.0);
    level.0 += (target - level.0) * k;
    if (level.0 - target).abs() < 0.01 {
        level.0 = target;
    }
    for mut bg in &mut scrim {
        bg.0 = BACKDROP.with_alpha(level.1 * level.0);
    }
    // the sweep: 420 ms down the screen, fading
    for (mut node, mut bg) in &mut sweep {
        let t = level.2.map(|s| ((now - s) / 0.42).clamp(0.0, 1.0)).unwrap_or(1.0);
        let e = 1.0 - (1.0 - t).powi(3);
        node.top = percent(100.0 * e);
        bg.0 = TX1.with_alpha(0.35 * (1.0 - t));
    }
    let hide = level.0 > 0.5;
    covered_flag.0 = hide;
    for mut v in &mut hud {
        let want = if hide { Visibility::Hidden } else { Visibility::Inherited };
        if *v != want {
            *v = want;
        }
    }
    for (cam, dof) in &mut cams {
        match (dof, level.0 > 0.0 && steady) {
            (Some(mut d), true) => d.max_circle_of_confusion_diameter = 24.0 * level.0,
            (None, true) => {
                commands.entity(cam).insert(DepthOfField { mode: DepthOfFieldMode::Gaussian, focal_distance: 0.5, aperture_f_stops: 0.02, max_circle_of_confusion_diameter: 24.0 * level.0, ..default() });
            }
            (Some(_), false) => {
                commands.entity(cam).remove::<DepthOfField>();
            }
            (None, false) => {}
        }
    }
}

/// Seconds without blur after the window changes size.
const SETTLE: f32 = 0.5;
/// And at least this many frames.
const SETTLE_FRAMES: u32 = 10;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<BlurLevel>().init_resource::<Covered>().add_systems(Startup, spawn_scrim).add_systems(Update, blur);
}
