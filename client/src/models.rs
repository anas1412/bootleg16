//! Loading CS studio models (weapons, grenades) from the install and turning
//! posed models into Bevy meshes.
use crate::{map, rgba_image};
use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::TextureFormat,
};
use goldsrc::mdl::{BoneTransform, MeshPart, Model};
use sim::weapons::Id;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Model file for a weapon: `v_` (first person), `p_` (in a player's hand) or `w_` (on the ground).
pub fn weapon_model(prefix: &str, id: Id) -> String {
    let name = match sim::weapons::def(id).name {
        "mp5navy" => "mp5",
        n => n,
    };
    format!("{prefix}_{name}.mdl")
}

/// A model and the materials for its textures.
pub struct Loaded {
    pub model: Model,
    pub materials: Vec<Handle<StandardMaterial>>,
}

/// Models loaded on first use and kept (missing files are remembered as missing).
#[derive(Resource)]
pub struct ModelCache {
    dir: PathBuf,
    models: HashMap<String, Option<Arc<Loaded>>>,
}

impl ModelCache {
    pub fn new(game_dir: PathBuf) -> ModelCache {
        ModelCache { dir: game_dir.join("cstrike/models"), models: HashMap::new() }
    }

    pub fn get(&mut self, file: &str, images: &mut Assets<Image>, materials: &mut Assets<StandardMaterial>) -> Option<Arc<Loaded>> {
        if let Some(m) = self.models.get(file) {
            return m.clone();
        }
        let loaded = Model::load(self.dir.join(file)).ok().map(|model| {
            let mats = model
                .textures()
                .into_iter()
                .map(|t| {
                    let image = images.add(rgba_image(t.width, t.height, t.rgba, false, TextureFormat::Rgba8UnormSrgb));
                    // ⚠ Unlit until models are lit from the map's lightmaps like GoldSrc does.
                    materials.add(StandardMaterial { base_color_texture: Some(image), unlit: true, alpha_mode: AlphaMode::Mask(0.5), ..default() })
                })
                .collect();
            Arc::new(Loaded { model, materials: mats })
        });
        if loaded.is_none() {
            warn!("missing model {file}");
        }
        self.models.insert(file.to_string(), loaded.clone());
        loaded
    }
}

/// Bevy mesh for one posed part, with `place` mapping GoldSrc model space to the target
/// space. `mirrored`: `place` flips handedness, so triangles are wound the other way.
pub fn part_mesh(p: &MeshPart, place: impl Fn([f32; 3]) -> [f32; 3], mirrored: bool) -> Mesh {
    let positions: Vec<[f32; 3]> = p.positions.iter().map(|v| place(*v)).collect();
    let mut indices = p.indices.clone();
    if mirrored {
        for t in indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uvs.clone())
        .with_inserted_indices(Indices::U32(indices))
        .with_computed_normals()
}

/// A model posed once, as world-ready (Y-up) meshes with their materials.
pub fn static_parts(l: &Loaded, pose: &[BoneTransform], meshes: &mut Assets<Mesh>) -> Vec<(Handle<Mesh>, Handle<StandardMaterial>)> {
    l.model
        .posed_mesh(pose)
        .iter()
        .filter(|p| !p.indices.is_empty())
        .map(|p| (meshes.add(part_mesh(p, map::to_y_up, false)), l.materials.get(p.texture).cloned().unwrap_or_default()))
        .collect()
}
