//! Drawing the map: textured, lightmapped world geometry and the doors that move with the sim.
use crate::game::{Frame, Walker};
use crate::menu::{AppState, GameConfig};
use crate::map;
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    math::Rect,
    mesh::{Indices, PrimitiveTopology},
    pbr::Lightmap,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

/// GoldSrc's video defaults: `gamma` 2.5 (also `lightgamma`), `texgamma` 2.0, `brightness` 1.
const GAMMA: f32 = 2.5;
const TEX_GAMMA: f32 = 2.0;
const BRIGHTNESS: f32 = 1.0;

/// GoldSrc's `texgammatable`: every texture brightened a little (x^(texgamma / gamma)).
pub fn tex_gamma(rgba: &mut [u8]) {
    let table: Vec<u8> = (0..256).map(|i| (255.0 * (i as f32 / 255.0).powf(TEX_GAMMA / GAMMA)).round() as u8).collect();
    for px in rgba.chunks_mut(4) {
        for c in &mut px[..3] {
            *c = table[*c as usize];
        }
    }
}

/// GoldSrc's `lightgammatable`: light below `g3` is lifted (more with `brightness`), then
/// everything is raised to 1 / gamma. The result multiplies the texture in display space.
fn light_gamma(rgba: &mut [u8]) {
    let g3 = if BRIGHTNESS <= 0.0 { 0.125 } else if BRIGHTNESS > 1.0 { 0.05 } else { 0.125 - BRIGHTNESS * BRIGHTNESS * 0.075 };
    let table: Vec<u8> = (0..256)
        .map(|i| {
            let mut f = i as f32 / 255.0;
            if BRIGHTNESS > 0.0 {
                f = if f > g3 { 0.125 + (f - g3) / (1.0 - g3) * 0.875 } else { f / g3 * 0.125 };
            }
            (255.0 * f.powf(1.0 / GAMMA)).round() as u8
        })
        .collect();
    for px in rgba.chunks_mut(4) {
        for c in &mut px[..3] {
            *c = table[*c as usize];
        }
    }
}

pub struct LevelPlugin;

impl Plugin for LevelPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_level).add_systems(Update, (move_doors, hide_broken).in_set(Frame::World));
    }
}

pub fn rgba_image(w: u32, h: u32, rgba: Vec<u8>, repeat: bool, format: TextureFormat) -> Image {
    let mut img = Image::new(
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        TextureDimension::D2,
        rgba,
        format,
        RenderAssetUsages::RENDER_WORLD,
    );
    if repeat {
        img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: ImageAddressMode::Repeat,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        });
    } else {
        img.sampler = ImageSampler::linear();
    }
    img
}

fn spawn_level(
    mut commands: Commands,
    config: Res<GameConfig>,
    mut map_data: ResMut<crate::loading::MapData>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(geo) = map_data.geometry.take() else { return };
    info!("{}: {} batches, lightmap {:?}", config.map, geo.batches.len(), geo.lightmap_size);
    commands.insert_resource(Spawns(geo.spawns.clone()));

    let (lw, lh) = geo.lightmap_size;
    // Through GoldSrc's light curve, stored as display values (sRGB): Bevy decodes both it and
    // the texture, multiplies, and encodes again, which is GoldSrc's multiply in display space.
    let mut lm = geo.lightmap_rgba;
    light_gamma(&mut lm);
    let lightmap = images.add(rgba_image(lw as u32, lh as u32, lm, false, TextureFormat::Rgba8UnormSrgb));
    let mut tex_handles = vec![None; geo.textures.len()];

    let mut spawn_batch = |b: map::Batch, render: Option<map::Render>, commands: &mut Commands| -> Entity {
        let t = &geo.textures[b.texture];
        let image = tex_handles[b.texture]
            .get_or_insert_with(|| {
                let mut rgba = t.rgba.clone();
                tex_gamma(&mut rgba);
                images.add(rgba_image(t.width, t.height, rgba, true, TextureFormat::Rgba8UnormSrgb))
            })
            .clone();
        let material = materials.add(StandardMaterial {
            base_color_texture: Some(image),
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            lightmap_exposure: 1.0,
            // See-through brushes: `texture` mode fades by renderamt, `additive` glows (nuke's glass).
            base_color: Color::WHITE.with_alpha(render.filter(|r| matches!(r.mode, 1 | 2 | 3 | 5)).map(|r| r.amt).unwrap_or(1.0)),
            alpha_mode: match render.map(|r| r.mode) {
                Some(5) | Some(3) => AlphaMode::Add,
                Some(1) | Some(2) => AlphaMode::Blend,
                _ if t.name.starts_with('{') => AlphaMode::Mask(0.5),
                _ => AlphaMode::Opaque,
            },
            ..default()
        });
        // Keep the mesh on the CPU side too: Bevy needs it to compute the bounding box for
        // culling (GPU-only meshes get a box at the origin and vanish when it's off-screen).
        let mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, b.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, b.uv)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, b.lightmap_uv)
            .with_inserted_indices(Indices::U32(b.indices))
            .with_computed_normals(); // face vertices are never shared, so these are flat
        commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(material),
                Lightmap { image: lightmap.clone(), uv_rect: Rect::new(0.0, 0.0, 1.0, 1.0), ..default() },
                DespawnOnExit(AppState::InGame),
            ))
            .id()
    };
    for b in geo.batches {
        spawn_batch(b, None, &mut commands);
    }
    // Breakables and see-through brushes: one parent each (breakables vanish when broken).
    for (model, render, batches) in geo.pieces {
        let parent = commands.spawn((BrushPiece(model), Transform::default(), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for b in batches {
            let child = spawn_batch(b, Some(render), &mut commands);
            commands.entity(parent).add_child(child);
        }
    }
    // Doors: one parent per door, moved every frame to where the sim says it is.
    for (model, batches) in geo.movers {
        let parent = commands.spawn((DoorMesh(model), Transform::default(), Visibility::default(), DespawnOnExit(AppState::InGame))).id();
        for b in batches {
            let child = spawn_batch(b, None, &mut commands);
            commands.entity(parent).add_child(child);
        }
    }
}

/// A breakable or see-through brush entity's render parent; the value is its brush model.
#[derive(Component)]
struct BrushPiece(usize);

/// Broken pieces disappear (and come back when the server restores them).
fn hide_broken(w: Res<Walker>, mut pieces: Query<(&BrushPiece, &mut Visibility)>) {
    for (BrushPiece(m), mut v) in &mut pieces {
        v.set_if_neq(if w.client.broken.contains(&(*m as u16)) { Visibility::Hidden } else { Visibility::Inherited });
    }
}

/// Spawn points (Bevy coordinates) and the way they face (GoldSrc yaw, degrees).
#[derive(Resource)]
pub struct Spawns(pub Vec<([f32; 3], f32)>);

/// A door's render entity; the value is its brush model index.
#[derive(Component)]
struct DoorMesh(usize);

/// GoldSrc (Z up) to Bevy (Y up) for direction vectors.
fn y_up(v: [f32; 3]) -> Vec3 {
    Vec3::from(map::to_y_up(v))
}

/// Where to draw a door: the same transform the sim uses for its collision.
fn door_transform(d: &sim::doors::Door) -> Transform {
    let (off, ang) = d.transform();
    let (f, r, u) = sim::doors::angle_vectors(ang);
    // Model space -> world: columns are where the model's x, z, -y axes end up.
    Transform {
        rotation: Quat::from_mat3(&Mat3::from_cols(y_up(f), y_up(u), y_up(r))),
        translation: y_up([d.pivot[0] + off[0], d.pivot[1] + off[1], d.pivot[2] + off[2]]),
        ..default()
    }
}

fn move_doors(w: Res<Walker>, mut doors: Query<(&DoorMesh, &mut Transform)>) {
    for (DoorMesh(model), mut tf) in &mut doors {
        if let Some(d) = w.client.world.doors.doors.iter().find(|d| d.model == *model) {
            *tf = door_transform(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A door drawn with `door_transform` must sit exactly where its collision is.
    #[test]
    fn rendered_door_matches_collision_on_inferno() {
        let path = crate::game_dir().join("cstrike/maps/de_inferno.bsp");
        let Ok(bsp) = goldsrc::bsp::Bsp::load(&path) else { return };
        let mut doors = sim::doors::Doors::from_bsp(&bsp);
        let actor = sim::doors::Actor { mins: [0.0; 3], maxs: [0.0; 3], eye: [0.0; 3], forward: [1.0, 0.0, 0.0], using: false };
        // Fire it open by touching it, then let it swing partway.
        let (lo, hi) = doors.doors[0].bounds();
        let toucher = sim::doors::Actor { mins: lo, maxs: hi, ..actor };
        doors.update(1.0 / 128.0, &[toucher], |_, _| false);
        for _ in 0..40 {
            doors.update(1.0 / 128.0, &[], |_, _| false);
        }
        let d = &doors.doors[0];
        assert!(d.transform().1[1].abs() > 1.0, "door did not move");
        let (off, ang) = d.transform();
        let (f, r, u) = sim::doors::angle_vectors(ang);
        let tf = door_transform(d);
        for corner in [d.mins, d.maxs, [d.mins[0], d.maxs[1], d.mins[2]]] {
            let sim_world = sim::doors::local_to_world(corner, f, r, u);
            let sim_world = [sim_world[0] + d.pivot[0] + off[0], sim_world[1] + d.pivot[1] + off[1], sim_world[2] + d.pivot[2] + off[2]];
            let drawn = tf.transform_point(Vec3::from(map::to_y_up(corner)));
            assert!(drawn.distance(Vec3::from(map::to_y_up(sim_world))) < 0.01, "{drawn} vs {sim_world:?}");
        }
    }
}
