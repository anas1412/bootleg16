//! The menu in 3D, Persona-style (design/mockups/ui: interact.js `placeAll`). The main menu sits
//! straight; opening a page swings it into 3D: the list turns in from the left, the page from
//! the right, each landing with a small overshoot. Esc (back to the main menu) swings it flat.
//! A page swap straightens the page while it swaps, then swings it back.
//!
//! How: the menu's two sides are drawn into their own images by two UI cameras (`Layer`), and
//! each image is shown on a flat surface in front of a perspective camera, sized to cover
//! exactly its part of the screen, so at rest it looks like the flat menu. Clicks and hovers
//! are carried through: the mouse ray is met with the surface it points at and becomes a
//! pointer on that side's image. The Play screen and a match found stay flat (normal camera).
use super::{AppState, MenuState, Page};
use bevy::asset::uuid::Uuid;
use bevy::camera::{ImageRenderTarget, RenderTarget};
use bevy::core_pipeline::tonemapping::{DebandDither, Tonemapping};
use bevy::input::ButtonState;
use bevy::mesh::VertexAttributeValues;
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::picking::PickingSystems;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureFormat};
use bevy::window::{PrimaryWindow, WindowEvent};

/// Which side of the menu a root belongs to.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// The list, the wordmark.
    Left,
    /// The page, your name, invites.
    Right,
}

/// Where the screen splits between the sides, in 1080p units from the left edge (the list and
/// wordmark end before it, the page starts after it).
const SPLIT: f32 = 700.0;
/// Distance from the eye to the screen in 1080p units: the mockup's CSS perspective, so the
/// turn looks the same.
const EYE: f32 = 1300.0;
/// Turn (degrees) and how far toward you (1080p units) each side sits when open.
const LEFT_TURN: f32 = 16.0;
const RIGHT_TURN: f32 = -10.0;
const LEFT_NEAR: f32 = 48.0;
const RIGHT_NEAR: f32 = 32.0;
/// Swinging into place, the stagger between the sides, and flattening.
const SETTLE: f32 = 0.82;
const STAGGER: f32 = 0.07;
const FLATTEN: f32 = 0.38;

const POINTERS: [PointerId; 2] = [PointerId::Custom(Uuid::from_u128(0xb16_1e57)), PointerId::Custom(Uuid::from_u128(0xb16_819e7))];

/// One side: its camera, image and surface, and how far it is turned (0 flat, 1 placed).
struct Side {
    layer: Layer,
    camera: Entity,
    image: Handle<Image>,
    hinge: Entity,
    surface: Entity,
    tween: Tween,
}

/// A value moving from `from` to `to` over `dur`, starting at `start`; `back` swings with an
/// overshoot, else it eases in and out. `then` queues the next move.
#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    start: f32,
    dur: f32,
    back: bool,
    then: Option<(f32, f32)>,
}

impl Tween {
    fn at(&self, now: f32) -> f32 {
        let t = ((now - self.start) / self.dur.max(1e-4)).clamp(0.0, 1.0);
        let e = if self.back {
            // easeOutBack: lands a touch past the target, then settles
            let (c1, c3) = (1.2, 2.2);
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        } else {
            t * t * (3.0 - 2.0 * t)
        };
        self.from + (self.to - self.from) * e
    }
    fn go(&mut self, now: f32, to: f32, dur: f32, back: bool, delay: f32) {
        *self = Tween { from: self.at(now), to, start: now + delay, dur, back, then: None };
    }
}

#[derive(Resource)]
struct Stage {
    sides: Vec<Side>,
    /// window physical size the images and surfaces were made for
    size: UVec2,
    open: bool,
    page: Page,
}

fn open_page(p: Page) -> bool {
    !matches!(p, Page::Main | Page::Join)
}

/// World size of the screen at the surfaces' distance, in 1080p units.
fn screen(size: UVec2) -> Vec2 {
    Vec2::new(1080.0 * size.x as f32 / size.y.max(1) as f32, 1080.0)
}

/// A rectangle covering screen x from `x0` to `x1` (1080p units from the left), showing that
/// part of a full-screen image.
fn surface_mesh(x0: f32, x1: f32, scr: Vec2) -> Mesh {
    let (w, h) = (x1 - x0, scr.y);
    let mut m = Mesh::from(Rectangle::new(w, h));
    let positions: Vec<[f32; 3]> = match m.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.clone(),
        _ => Vec::new(),
    };
    let uvs: Vec<[f32; 2]> = positions.iter().map(|p| [(x0 + p[0] + w / 2.0) / scr.x, (h / 2.0 - p[1]) / h]).collect();
    m.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    m
}

fn build(commands: &mut Commands, images: &mut Assets<Image>, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, window: &Window) -> Stage {
    let size = UVec2::new(window.physical_width().max(1), window.physical_height().max(1));
    let scr = screen(size);
    let mut sides = Vec::new();
    for (i, layer) in [Layer::Left, Layer::Right].into_iter().enumerate() {
        let image = images.add(Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None));
        let camera = commands
            .spawn((
                Camera2d,
                Camera { order: -20 + i as isize, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
                RenderTarget::Image(ImageRenderTarget { handle: image.clone(), scale_factor: window.scale_factor() }),
                DespawnOnExit(AppState::Menu),
            ))
            .id();
        let (x0, x1) = if layer == Layer::Left { (0.0, SPLIT) } else { (SPLIT, scr.x) };
        // the hinge on the side's outer edge; the surface hangs from it
        let hinge_x = if layer == Layer::Left { -scr.x / 2.0 } else { scr.x / 2.0 };
        let half = (x1 - x0) / 2.0;
        let material = materials.add(StandardMaterial { base_color_texture: Some(image.clone()), unlit: true, alpha_mode: AlphaMode::Premultiplied, ..default() });
        let mut surface = Entity::PLACEHOLDER;
        let hinge = commands
            .spawn((Transform::from_xyz(hinge_x, 0.0, -EYE), Visibility::default(), DespawnOnExit(AppState::Menu)))
            .with_children(|h| {
                surface = h.spawn((Mesh3d(meshes.add(surface_mesh(x0, x1, scr))), MeshMaterial3d(material), Transform::from_xyz(if layer == Layer::Left { half } else { -half }, 0.0, 0.0))).id();
            })
            .id();
        commands.spawn((POINTERS[i], DespawnOnExit(AppState::Menu)));
        sides.push(Side { layer, camera, image, hinge, surface, tween: Tween { from: 0.0, to: 0.0, start: 0.0, dur: 1.0, back: false, then: None } });
    }
    Stage { sides, size, open: false, page: Page::Main }
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, window: Single<&Window, With<PrimaryWindow>>) {
    // the camera that sees the surfaces: a vertical field of view matching EYE
    commands.spawn((
        Camera3d::default(),
        Camera { order: 0, clear_color: ClearColorConfig::Custom(crate::ui::BG0), ..default() },
        Projection::Perspective(PerspectiveProjection { fov: 2.0 * (540.0 / EYE).atan(), near: 10.0, far: 10_000.0, ..default() }),
        Msaa::Off,
        Tonemapping::None,
        DebandDither::Disabled,
        DespawnOnExit(AppState::Menu),
    ));
    let stage = build(&mut commands, &mut images, &mut meshes, &mut materials, &window);
    commands.insert_resource(stage);
}

/// Menu roots go to their side's camera (the Play screen and a match found stay flat).
fn route(mut commands: Commands, stage: Option<Res<Stage>>, roots: Query<(Entity, &Layer), Without<UiTargetCamera>>) {
    let Some(stage) = stage else { return };
    for (e, layer) in &roots {
        if let Some(s) = stage.sides.iter().find(|s| s.layer == *layer) {
            commands.entity(e).insert(UiTargetCamera(s.camera));
        }
    }
}

/// Follow the page: open swings both sides in (left first), back to the main menu swings them
/// flat, a page swap dips the right side flat and back.
fn animate(time: Res<Time>, st: Option<Res<MenuState>>, stage: Option<ResMut<Stage>>, mut hinges: Query<&mut Transform>) {
    let (Some(st), Some(mut stage)) = (st, stage) else { return };
    let now = time.elapsed_secs();
    let open = open_page(st.page);
    let swapped = open && stage.open && st.page != stage.page;
    if open != stage.open || swapped {
        for (i, s) in stage.sides.iter_mut().enumerate() {
            if swapped {
                if s.layer == Layer::Right {
                    s.tween.go(now, 0.0, crate::ui::T_OUT, false, 0.0);
                    s.tween.then = Some((1.0, SETTLE));
                }
            } else if open {
                s.tween.go(now, 1.0, SETTLE, true, i as f32 * STAGGER);
            } else {
                s.tween.go(now, 0.0, FLATTEN, false, 0.0);
            }
        }
        stage.open = open;
    }
    stage.page = st.page;
    for s in &mut stage.sides {
        if let Some((to, dur)) = s.tween.then {
            if now >= s.tween.start + s.tween.dur {
                s.tween.go(now, to, dur, true, 0.0);
            }
        }
        let k = s.tween.at(now);
        let (turn, near) = if s.layer == Layer::Left { (LEFT_TURN, LEFT_NEAR) } else { (RIGHT_TURN, RIGHT_NEAR) };
        if let Ok(mut tr) = hinges.get_mut(s.hinge) {
            tr.rotation = Quat::from_rotation_y((turn * k).to_radians());
            tr.translation.z = -EYE + near * k;
        }
    }
}

/// The images and surfaces follow the window's size and scale factor.
#[allow(clippy::too_many_arguments)]
fn resize(mut commands: Commands, stage: Option<ResMut<Stage>>, window: Single<&Window, With<PrimaryWindow>>, mut images: ResMut<Assets<Image>>, mut meshes: ResMut<Assets<Mesh>>, mut targets: Query<&mut RenderTarget>, surfaces: Query<&Mesh3d>) {
    let Some(mut stage) = stage else { return };
    let size = UVec2::new(window.physical_width().max(1), window.physical_height().max(1));
    for s in &stage.sides {
        if let Ok(mut t) = targets.get_mut(s.camera) {
            if let RenderTarget::Image(img) = &mut *t {
                if img.scale_factor != window.scale_factor() {
                    img.scale_factor = window.scale_factor();
                }
            }
        }
    }
    if size == stage.size {
        return;
    }
    stage.size = size;
    let scr = screen(size);
    for s in &stage.sides {
        if let Some(mut img) = images.get_mut(&s.image) {
            img.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
        }
        let (x0, x1) = if s.layer == Layer::Left { (0.0, SPLIT) } else { (SPLIT, scr.x) };
        if let Ok(m) = surfaces.get(s.surface) {
            if let Some(mut mesh) = meshes.get_mut(&m.0) {
                *mesh = surface_mesh(x0, x1, scr);
            }
        }
        let half = (x1 - x0) / 2.0;
        let hinge_x = if s.layer == Layer::Left { -scr.x / 2.0 } else { scr.x / 2.0 };
        commands.entity(s.hinge).entry::<Transform>().and_modify(move |mut t| t.translation.x = hinge_x);
        commands.entity(s.surface).insert(Transform::from_xyz(if s.layer == Layer::Left { half } else { -half }, 0.0, 0.0));
    }
}

/// The mouse, carried onto the side it points at: a pointer on that side's image, moved to
/// where the ray meets the surface; buttons go to that side. The other side's pointer leaves.
/// Nothing is carried while a flat screen is up (a match found, the Play screen).
#[allow(clippy::too_many_arguments)]
fn carry(
    stage: Option<Res<Stage>>,
    st: Option<Res<MenuState>>,
    window: Single<(Entity, &Window), With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform), With<Camera3d>>,
    surfaces: Query<&GlobalTransform, With<Mesh3d>>,
    targets: Query<&RenderTarget>,
    flat: Query<(), With<crate::found::FoundOverlay>>,
    mut events: MessageReader<WindowEvent>,
    mut pointers: MessageWriter<PointerInput>,
    mut last: Local<[Option<Vec2>; 2]>,
    mut over: Local<Option<usize>>,
) {
    let (Some(stage), Some(st)) = (stage, st) else { return };
    let (win_e, win) = *window;
    let blocked = st.page == Page::Join || !flat.is_empty();
    let scr = screen(stage.size);
    let logical = Vec2::new(win.width(), win.height());
    // where the cursor meets each side, as a position on that side's image
    let mut hit: Option<(usize, Vec2)> = None;
    if let (Some(cursor), Ok((cam, cam_tr)), false) = (win.cursor_position(), cams.single(), blocked) {
        if let Ok(ray) = cam.viewport_to_world(cam_tr, cursor) {
            for (i, s) in stage.sides.iter().enumerate() {
                let Ok(tr) = surfaces.get(s.surface) else { continue };
                let normal = tr.forward().as_vec3();
                let denom = ray.direction.dot(normal);
                if denom.abs() < 1e-5 {
                    continue;
                }
                let d = (tr.translation() - ray.origin).dot(normal) / denom;
                if d <= 0.0 {
                    continue;
                }
                let local = tr.affine().inverse().transform_point3(ray.origin + ray.direction * d);
                let (x0, x1) = if s.layer == Layer::Left { (0.0, SPLIT) } else { (SPLIT, scr.x) };
                let (w, h) = (x1 - x0, scr.y);
                if local.x.abs() <= w / 2.0 && local.y.abs() <= h / 2.0 {
                    let uv = Vec2::new((x0 + local.x + w / 2.0) / scr.x, (h / 2.0 - local.y) / h);
                    hit = Some((i, uv * logical));
                    break;
                }
            }
        }
    }
    let location = |i: usize, position: Vec2| -> Option<Location> {
        let target = targets.get(stage.sides[i].camera).ok()?.normalize(Some(win_e))?;
        Some(Location { target, position })
    };
    for i in 0..2 {
        let pos = match hit {
            Some((j, p)) if j == i => p,
            // off the image: hovers end
            _ => Vec2::splat(-10_000.0),
        };
        if last[i] != Some(pos) {
            if let Some(loc) = location(i, pos) {
                pointers.write(PointerInput::new(POINTERS[i], loc, PointerAction::Move { delta: pos - last[i].unwrap_or(pos) }));
            }
            last[i] = Some(pos);
        }
    }
    *over = hit.map(|h| h.0);
    for ev in events.read() {
        let WindowEvent::MouseButtonInput(input) = ev else { continue };
        let button = match input.button {
            MouseButton::Left => PointerButton::Primary,
            MouseButton::Right => PointerButton::Secondary,
            MouseButton::Middle => PointerButton::Middle,
            _ => continue,
        };
        match input.state {
            ButtonState::Pressed => {
                if let Some(i) = *over {
                    if let Some(loc) = location(i, last[i].unwrap_or_default()) {
                        pointers.write(PointerInput::new(POINTERS[i], loc, PointerAction::Press(button)));
                    }
                }
            }
            // releases go to both, so nothing stays held
            ButtonState::Released => {
                for i in 0..2 {
                    if let Some(loc) = location(i, last[i].unwrap_or_default()) {
                        pointers.write(PointerInput::new(POINTERS[i], loc, PointerAction::Release(button)));
                    }
                }
            }
        }
    }
}

/// Esc closes a page back to the straight main menu (not while a key is being bound; the Play
/// screen has its own, `play::back`).
fn escape(keys: Res<ButtonInput<KeyCode>>, binds: Res<crate::binds::Binds>, st: Option<ResMut<MenuState>>) {
    let Some(mut st) = st else { return };
    if keys.just_pressed(KeyCode::Escape) && binds.capture.is_none() && open_page(st.page) {
        st.page = Page::Main;
        st.rebuild = true;
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(AppState::Menu), spawn)
        .add_systems(OnExit(AppState::Menu), |mut commands: Commands| commands.remove_resource::<Stage>())
        .add_systems(Update, (escape, animate, resize).run_if(in_state(AppState::Menu)))
        // A rebuilt page is routed to its side's camera in the same frame it appears: routed a
        // frame later it would show flat for one frame (a snap to straight, then back into 3D).
        .add_systems(Update, route.after(crate::menu_pages::rebuild_menu).run_if(in_state(AppState::Menu)))
        .add_systems(First, carry.in_set(PickingSystems::Input).run_if(in_state(AppState::Menu)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swing_lands_with_an_overshoot_and_settles() {
        let t = Tween { from: 0.0, to: 1.0, start: 0.0, dur: 1.0, back: true, then: None };
        assert!(t.at(0.0).abs() < 1e-5);
        assert!((30..95).any(|i| t.at(i as f32 / 100.0) > 1.0));
        assert!((t.at(1.0) - 1.0).abs() < 1e-5);
        let f = Tween { from: 1.0, to: 0.0, start: 0.0, dur: 0.5, back: false, then: None };
        assert!((0..=50).map(|i| f.at(i as f32 / 100.0)).all(|v| (0.0..=1.0).contains(&v)));
    }

    #[test]
    fn surfaces_show_their_part_of_the_image() {
        let scr = Vec2::new(1920.0, 1080.0);
        let m = surface_mesh(SPLIT, 1920.0, scr);
        let Some(VertexAttributeValues::Float32x2(uv)) = m.attribute(Mesh::ATTRIBUTE_UV_0) else { panic!() };
        let us: Vec<f32> = uv.iter().map(|u| u[0]).collect();
        assert!(us.iter().any(|u| (u - SPLIT / 1920.0).abs() < 1e-4));
        assert!(us.iter().any(|u| (u - 1.0).abs() < 1e-4));
    }
}
