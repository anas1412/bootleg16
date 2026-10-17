//! The map's glows (`env_glow`: nuke's and train's lamps), as GoldSrc draws them
//! (`R_GlowBlend`): an additive sprite in the entity's colour, the same size on screen at any
//! distance (scale = distance x 0.005), fading with distance (19000 / distance², 0.05..1), and
//! gone when the line from your eye to it is blocked.
use crate::game::{Frame, Walker};
use crate::level::rgba_image;
use crate::map;
use crate::menu::AppState;
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;

pub struct GlowsPlugin;

impl Plugin for GlowsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn).add_systems(Update, update.in_set(Frame::World));
    }
}

#[derive(Component)]
struct Glow {
    at: [f32; 3],
    /// Sprite width in texels (its quad is that many units at scale 1).
    width: f32,
    /// rendercolor x renderamt, linear.
    color: LinearRgba,
    material: Handle<StandardMaterial>,
}

fn spawn(mut commands: Commands, map: Res<crate::loading::MapData>, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, mut images: ResMut<Assets<Image>>) {
    let bsp = &map.bsp;
    let quad = meshes.add(Rectangle::new(1.0, 1.0));
    let mut sprites = std::collections::HashMap::new();
    let vec3 = |s: Option<&String>| -> [f32; 3] {
        let v: Vec<f32> = s.map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect()).unwrap_or_default();
        std::array::from_fn(|i| v.get(i).copied().unwrap_or(0.0))
    };
    for e in bsp.entity_list().iter().filter(|e| e.get("classname").map(String::as_str) == Some("env_glow")) {
        let Some(file) = e.get("model") else { continue };
        let sprite = sprites.entry(file.clone()).or_insert_with(|| {
            crate::gamefiles::paths(file).iter().find_map(|p| goldsrc::spr::Sprite::load(p).ok()).map(|s| {
                // Additive: the colours themselves are the glow.
                let rgba = s.colors.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect();
                (s.width as f32, images.add(rgba_image(s.width, s.height, rgba, false, TextureFormat::Rgba8UnormSrgb)))
            })
        });
        let Some((width, image)) = sprite.clone() else { continue };
        let c = vec3(e.get("rendercolor"));
        let amt = e.get("renderamt").and_then(|v| v.parse::<f32>().ok()).unwrap_or(255.0) / 255.0;
        let color = Color::srgb(c[0] / 255.0, c[1] / 255.0, c[2] / 255.0).to_linear() * amt;
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            base_color: Color::BLACK,
            unlit: true,
            alpha_mode: AlphaMode::Add,
            ..default()
        });
        let at = vec3(e.get("origin"));
        commands.spawn((
            Glow { at, width, color, material: material.clone() },
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(Vec3::from(map::to_y_up(at))),
            Visibility::Hidden,
            DespawnOnExit(AppState::InGame),
        ));
    }
}

fn update(w: Res<Walker>, cam: Single<&GlobalTransform, With<crate::game::FlyCam>>, mut glows: Query<(&Glow, &mut Transform, &mut Visibility)>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let eye = cam.translation();
    let eye_gs = [eye.x, -eye.z, eye.y];
    for (g, mut tf, mut vis) in &mut glows {
        let d = [g.at[0] - eye_gs[0], g.at[1] - eye_gs[1], g.at[2] - eye_gs[2]];
        let dist = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1.0);
        let t = w.client.world.trace_line(eye_gs, g.at);
        if (1.0 - t.fraction) * dist > 8.0 {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        }
        vis.set_if_neq(Visibility::Inherited);
        let brightness = (19000.0 / (dist * dist)).clamp(0.05, 1.0);
        // GoldSrc draws glows over everything: pulled towards the eye (scaled to look the same)
        // so the lamp they sit in can't cut them; `trace_line` above hides them behind walls.
        let pull = (g.width * 0.5).min(dist * 0.5);
        let at = Vec3::from(map::to_y_up(g.at)).lerp(eye, pull / dist);
        let size = g.width * dist * 0.005 * (dist - pull) / dist;
        *tf = Transform::from_translation(at).looking_at(eye, Vec3::Y).with_scale(Vec3::splat(size));
        // looking_at points -Z at the eye; the quad faces +Z: turn it round.
        tf.rotate_local_y(std::f32::consts::PI);
        if let Some(mut m) = materials.get_mut(&g.material) {
            // GoldSrc adds the glow on screen (display space); scaling the linear colour by
            // the same factor lands close to it over the dark surroundings glows sit in.
            m.base_color = LinearRgba::new(g.color.red * brightness, g.color.green * brightness, g.color.blue * brightness, 1.0).into();
        }
    }
}
