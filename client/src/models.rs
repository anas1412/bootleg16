//! Loading CS studio models (weapons, grenades) from the install and turning
//! posed models into Bevy meshes.
use crate::level::rgba_image;
use crate::map;
use crate::studio::{LitPart, StudioMaterial};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{skinning::{SkinnedMesh, SkinnedMeshInverseBindposes}, Indices, PrimitiveTopology, VertexAttributeValues},
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

/// Meshes with their materials, ready to spawn (`spawn_lit` gives each instance its own).
pub type Parts = Vec<(Handle<Mesh>, Handle<StudioMaterial>)>;

/// A copy of a model material for one instance (its light changes as it moves).
pub fn own(materials: &mut Assets<StudioMaterial>, shared: &Handle<StudioMaterial>) -> Handle<StudioMaterial> {
    let m = materials.get(shared).cloned().unwrap_or_else(|| crate::studio::material(Handle::default()));
    materials.add(m)
}

/// Spawn lit model parts under `parent`.
pub fn spawn_lit(commands: &mut Commands, parent: Entity, parts: &Parts, materials: &mut Assets<StudioMaterial>) {
    for (mesh, mat) in parts {
        let child = commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(own(materials, mat)), LitPart)).id();
        commands.entity(parent).add_child(child);
    }
}

/// A model and the materials for its textures.
pub struct Loaded {
    pub model: Model,
    pub materials: Vec<Handle<StudioMaterial>>,
    /// GPU-skinned meshes, built on first use (see `Loaded::skin`); [normal, mirrored].
    skin: [std::sync::OnceLock<Skin>; 2],
}

/// A model's meshes in bind pose (each vertex in its bone's space) for GPU skinning:
/// animating it only moves bone entities, the meshes are never re-uploaded.
pub struct Skin {
    pub parts: Parts,
    pub bindposes: Handle<SkinnedMeshInverseBindposes>,
}

/// A skinned model spawned under a parent: its bone entities and its mesh entities
/// (plus the mirroring entity the bones hang off, if mirrored).
pub struct SpawnedSkin {
    pub joints: Vec<Entity>,
    pub parts: Vec<Entity>,
    pub mirror: Option<Entity>,
}

impl SpawnedSkin {
    pub fn despawn(&self, commands: &mut Commands) {
        for e in self.joints.iter().chain(&self.parts).chain(&self.mirror) {
            commands.entity(*e).try_despawn();
        }
    }
}

impl Loaded {
    /// `mirrored`: triangles wound the other way, for bones placed through a reflection.
    pub fn skin(&self, mirrored: bool, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>) -> &Skin {
        self.skin[mirrored as usize].get_or_init(|| {
            let parts = self
                .model
                .bind_mesh()
                .iter()
                .filter(|p| !p.indices.is_empty())
                .map(|p| {
                    let joints: Vec<[u16; 4]> = p.bones.iter().map(|b| [*b, 0, 0, 0]).collect();
                    let weights = vec![[1.0f32, 0.0, 0.0, 0.0]; p.bones.len()];
                    let mesh = part_mesh(p, map::to_y_up, mirrored)
                        .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(joints))
                        .with_inserted_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, weights);
                    (meshes.add(mesh), self.materials.get(p.texture).cloned().unwrap_or_default())
                })
                .collect();
            // Vertices are already in bone space, so binding is the identity.
            let bindposes = bindposes.add(SkinnedMeshInverseBindposes::from(vec![Mat4::IDENTITY; self.model.bones.len().max(1)]));
            Skin { parts, bindposes }
        })
    }

    /// Spawn bones and skinned meshes as children of `parent` (posed with `set_pose`).
    /// `mirror`: the bones go through this reflection first (the first-person view model).
    pub fn spawn_skinned(&self, commands: &mut Commands, parent: Entity, mirror: Option<Transform>, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, materials: &mut Assets<StudioMaterial>) -> SpawnedSkin {
        let skin = self.skin(mirror.is_some(), meshes, bindposes);
        let mirror = mirror.map(|t| commands.spawn((t, ChildOf(parent))).id());
        let joints: Vec<Entity> = (0..self.model.bones.len().max(1)).map(|_| commands.spawn((Transform::default(), ChildOf(mirror.unwrap_or(parent)))).id()).collect();
        let mut parts = Vec::new();
        for (mesh, material) in &skin.parts {
            // Bounds come from the bind pose, which is nowhere near the posed body: don't cull.
            parts.push(commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(own(materials, material)), LitPart, SkinnedMesh { inverse_bindposes: skin.bindposes.clone(), joints: joints.clone() }, NoFrustumCulling)).id());
        }
        commands.entity(parent).add_children(&parts);
        SpawnedSkin { joints, parts, mirror }
    }
}

/// Move a skinned model's bones to `pose` (GoldSrc model space).
pub fn set_pose(joints: &[Entity], pose: &[BoneTransform], transforms: &mut Query<&mut Transform, With<ChildOf>>) {
    for (e, b) in joints.iter().zip(pose) {
        if let Ok(mut t) = transforms.get_mut(*e) {
            *t = bone_transform(b);
        }
    }
}

/// A GoldSrc bone transform in Bevy's Y-up space: C·M·C⁻¹ with C = (x, y, z) → (x, z, −y).
fn bone_transform(b: &BoneTransform) -> Transform {
    let r = &b.rot;
    // Rows of C·R·Cᵀ, written out.
    let m = Mat3::from_cols(
        Vec3::new(r[0][0], r[2][0], -r[1][0]),
        Vec3::new(r[0][2], r[2][2], -r[1][2]),
        Vec3::new(-r[0][1], -r[2][1], r[1][1]),
    );
    Transform { translation: Vec3::from(map::to_y_up(b.pos)), rotation: Quat::from_mat3(&m), scale: Vec3::ONE }
}

/// Models loaded on first use and kept (missing files are remembered as missing).
#[derive(Resource)]
pub struct ModelCache {
    /// The game data folder's models.
    dirs: Vec<PathBuf>,
    models: HashMap<String, Option<Arc<Loaded>>>,
}

impl ModelCache {
    pub fn new() -> ModelCache {
        ModelCache { dirs: crate::gamefiles::paths("models"), models: HashMap::new() }
    }

    pub fn get(&mut self, file: &str, images: &mut Assets<Image>, materials: &mut Assets<StudioMaterial>) -> Option<Arc<Loaded>> {
        if let Some(m) = self.models.get(file) {
            return m.clone();
        }
        let loaded = self.dirs.iter().find_map(|d| Model::load(d.join(net::gamefiles::clean(file))).ok()).map(|model| {
            let mats = model
                .textures()
                .into_iter()
                .map(|t| {
                    let mut rgba = t.rgba;
                    crate::level::tex_gamma(&mut rgba);
                    let image = images.add(rgba_image(t.width, t.height, rgba, true, TextureFormat::Rgba8UnormSrgb));
                    materials.add(crate::studio::material(image))
                })
                .collect();
            Arc::new(Loaded { model, materials: mats, skin: Default::default() })
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
/// Normals go through `place` too (as directions), for `studio` lighting.
pub fn part_mesh(p: &MeshPart, place: impl Fn([f32; 3]) -> [f32; 3], mirrored: bool) -> Mesh {
    let positions: Vec<[f32; 3]> = p.positions.iter().map(|v| place(*v)).collect();
    let o = place([0.0; 3]);
    let normals: Vec<[f32; 3]> = p.normals.iter().map(|n| { let d = place(*n); [d[0] - o[0], d[1] - o[1], d[2] - o[2]] }).collect();
    let mut indices = p.indices.clone();
    if mirrored {
        for t in indices.chunks_exact_mut(3) {
            t.swap(1, 2);
        }
    }
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uvs.clone())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_indices(Indices::U32(indices))
}

/// A model posed once, as world-ready (Y-up) meshes with their materials.
pub fn static_parts(l: &Loaded, pose: &[BoneTransform], meshes: &mut Assets<Mesh>) -> Parts {
    l.model
        .posed_mesh(pose)
        .iter()
        .filter(|p| !p.indices.is_empty())
        .map(|p| (meshes.add(part_mesh(p, map::to_y_up, false)), l.materials.get(p.texture).cloned().unwrap_or_default()))
        .collect()
}
