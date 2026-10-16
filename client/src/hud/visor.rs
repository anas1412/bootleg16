//! The HUD's visor (design/mockups/ui, "HUD curve"): the HUD is drawn into its own image by a
//! second UI camera, then shown full-screen through a small bend: the middle swells toward you
//! and the edges lean back, as if the HUD sat on curved glass in front of your eyes. Strength is
//! the "HUD curve" setting (0 flat to 100; 60 by default). One extra pass over a screen-sized
//! image, a fraction of a millisecond.
//! Which parts: every UI root with `HudLayer` or `Visor`. Menus, the scoreboard, the buy menu
//! and the assistant's uplink stay on the normal camera (clicks and their own stacking).
use crate::menu::{AppState, Settings};
use crate::ui::HudLayer;
use bevy::asset::embedded_asset;
use bevy::prelude::*;
use bevy::render::render_resource::{AsBindGroup, Extent3d, TextureFormat};
use bevy::shader::ShaderRef;
use bevy::window::PrimaryWindow;

/// A HUD root drawn on the visor (roots with `HudLayer` already are).
#[derive(Component)]
pub struct Visor;

/// How much the middle swells at 100 (a share of the half-screen at the edge's middle).
const BEND_AT_FULL: f32 = 0.04;

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
pub(super) struct VisorMaterial {
    /// x: bend
    #[uniform(0)]
    bend: Vec4,
    #[texture(1)]
    #[sampler(2)]
    hud: Handle<Image>,
}

impl UiMaterial for VisorMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://client/hud/visor.wesl".into()
    }
}

/// The camera that draws the HUD, its image, and the material that shows it.
#[derive(Resource)]
struct VisorView {
    camera: Entity,
    image: Handle<Image>,
    material: Handle<VisorMaterial>,
}

fn spawn(mut commands: Commands, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<VisorMaterial>>, window: Single<&Window, With<PrimaryWindow>>, settings: Res<Settings>) {
    let (w, h) = (window.physical_width().max(1), window.physical_height().max(1));
    let image = images.add(Image::new_target_texture(w, h, TextureFormat::Rgba8UnormSrgb, None));
    let camera = commands
        .spawn((
            Camera2d,
            // before the world camera, into its own image, cleared to clear
            Camera { order: -10, clear_color: ClearColorConfig::Custom(Color::NONE), ..default() },
            // the window's scale factor, so it lays out exactly like the screen (high-DPI too)
            bevy::camera::RenderTarget::Image(bevy::camera::ImageRenderTarget { handle: image.clone(), scale_factor: window.scale_factor() }),
            DespawnOnExit(AppState::InGame),
        ))
        .id();
    let material = materials.add(VisorMaterial { bend: Vec4::new(bend(&settings), 0.0, 0.0, 0.0), hud: image.clone() });
    // over the world and the scope, under menus (the scrim is 5)
    commands.spawn((
        Node { position_type: PositionType::Absolute, width: percent(100), height: percent(100), ..default() },
        MaterialNode(material.clone()),
        GlobalZIndex(2),
        Pickable::IGNORE,
        DespawnOnExit(AppState::InGame),
    ));
    commands.insert_resource(VisorView { camera, image, material });
}

fn bend(s: &Settings) -> f32 {
    BEND_AT_FULL * s.hud_curve.min(100) as f32 / 100.0
}

/// HUD roots go to the visor's camera.
#[allow(clippy::type_complexity)]
fn route(mut commands: Commands, view: Option<Res<VisorView>>, roots: Query<Entity, (With<Node>, Without<ChildOf>, Without<UiTargetCamera>, Or<(With<HudLayer>, With<Visor>)>)>) {
    let Some(view) = view else { return };
    for e in &roots {
        commands.entity(e).insert(UiTargetCamera(view.camera));
    }
}

/// The image follows the window's size and scale factor; the bend follows the setting.
fn follow(view: Option<Res<VisorView>>, window: Single<&Window, With<PrimaryWindow>>, mut images: ResMut<Assets<Image>>, mut materials: ResMut<Assets<VisorMaterial>>, settings: Res<Settings>, mut targets: Query<&mut bevy::camera::RenderTarget>) {
    let Some(view) = view else { return };
    if let Ok(mut t) = targets.get_mut(view.camera) {
        if let bevy::camera::RenderTarget::Image(img) = &mut *t {
            if img.scale_factor != window.scale_factor() {
                img.scale_factor = window.scale_factor();
            }
        }
    }
    let size = Extent3d { width: window.physical_width().max(1), height: window.physical_height().max(1), depth_or_array_layers: 1 };
    if images.get(&view.image).is_some_and(|i| i.texture_descriptor.size != size) {
        if let Some(mut img) = images.get_mut(&view.image) {
            img.resize(size);
        }
        // the material holds the image: touch it so the new one is bound
        let _ = materials.get_mut(&view.material);
    }
    let b = bend(&settings);
    if materials.get(&view.material).is_some_and(|m| m.bend.x != b) {
        if let Some(mut m) = materials.get_mut(&view.material) {
            m.bend.x = b;
        }
    }
}

pub(super) fn plugin(app: &mut App) {
    embedded_asset!(app, "visor.wesl");
    app.add_plugins(UiMaterialPlugin::<VisorMaterial>::default())
        .add_systems(OnEnter(AppState::InGame), spawn)
        .add_systems(OnExit(AppState::InGame), |mut commands: Commands| commands.remove_resource::<VisorView>())
        .add_systems(Update, (route, follow).run_if(in_state(AppState::InGame)));
}
