//! One character, ready to draw: GPU-skinned meshes with our lighting material, posed from
//! the CS skeleton each frame. Either a Source model (`model.mdl` in its folder, posed through
//! the `rig`) or one of CS 1.6's own player models (`cs = "urban"` in its character.cfg,
//! the same skeleton as the animations, so it takes the pose bone for bone).
use super::rig::Rig;
use crate::map;
use crate::studio::{LitPart, StudioMaterial};
use bevy::{
    asset::RenderAssetUsages,
    camera::visibility::NoFrustumCulling,
    mesh::{skinning::{SkinnedMesh, SkinnedMeshInverseBindposes}, Indices, PrimitiveTopology, VertexAttributeValues},
    prelude::*,
    render::render_resource::TextureFormat,
};
use goldsrc::mdl::{BoneTransform, Model as CsModel};
use goldsrc::source::{Material, Model as Character, Texture};
use std::path::Path;
use std::sync::Arc;

/// A character's meshes, materials and how it's posed.
pub struct Look {
    parts: Vec<(Handle<Mesh>, Handle<StudioMaterial>)>,
    bindposes: Handle<SkinnedMeshInverseBindposes>,
    body: Body,
}

enum Body {
    /// A Source model, retargeted.
    Source(Rig),
    /// A CS 1.6 model: its own bones take the skeleton's pose by name.
    Cs { model: Arc<crate::models::Loaded>, skeleton: Arc<crate::models::Loaded> },
}

/// Z-up model space to Bevy's Y-up: (x, y, z) to (x, z, -y), for points and rotations.
fn y_up(q: Quat, p: Vec3) -> (Quat, Vec3) {
    (Quat::from_xyzw(q.x, q.z, -q.y, q.w), Vec3::from(map::to_y_up(p.to_array())))
}

impl Look {
    /// Loads `dir/model.mdl` with its materials (each `<name>.vmt` and the `.vtf` it names,
    /// all in `dir`). `cs` is the CS model whose animations the character plays.
    #[allow(clippy::too_many_arguments)]
    pub fn load(dir: &Path, skeleton: &Arc<crate::models::Loaded>, cache: &mut crate::models::ModelCache, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, images: &mut Assets<Image>, materials: &mut Assets<StudioMaterial>) -> Result<Look, String> {
        // a CS 1.6 class: its model from the game data, skinned as the CS code does it
        if let Some(class) = super::cs_class(dir) {
            let model = cache.get(&format!("player/{class}/{class}.mdl"), images, materials).ok_or_else(|| format!("{}: no models/player/{class}/{class}.mdl", dir.display()))?;
            let skin = model.skin(false, meshes, bindposes);
            return Ok(Look { parts: skin.parts.clone(), bindposes: skin.bindposes.clone(), body: Body::Cs { model: model.clone(), skeleton: skeleton.clone() } });
        }
        let cs: &CsModel = &skeleton.model;
        let model = Character::load(dir.join("model.mdl")).map_err(|e| format!("{}: {e}", dir.display()))?;
        let rig = Rig::new(cs, &model);
        let mats: Vec<Handle<StudioMaterial>> = model.materials.iter().map(|name| material(dir, name, images, materials)).collect();
        let parts = model
            .meshes
            .iter()
            .map(|m| {
                let v = &m.vertices;
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD);
                mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, v.iter().map(|x| map::to_y_up(x.pos)).collect::<Vec<_>>());
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, v.iter().map(|x| map::to_y_up(x.normal)).collect::<Vec<_>>());
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, v.iter().map(|x| x.uv).collect::<Vec<_>>());
                mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_INDEX, VertexAttributeValues::Uint16x4(v.iter().map(|x| [x.bones[0] as u16, x.bones[1] as u16, x.bones[2] as u16, 0]).collect()));
                mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, v.iter().map(|x| [x.weights[0], x.weights[1], x.weights[2], 0.0]).collect::<Vec<_>>());
                // Source winds its triangles the other way round from Bevy
                mesh.insert_indices(Indices::U32(m.indices.chunks_exact(3).flat_map(|t| [t[0], t[2], t[1]]).collect()));
                (meshes.add(mesh), mats.get(m.material).cloned().unwrap_or_default())
            })
            .collect();
        // bind matrices: the mesh is modelled in the bind pose, in the character's own units
        let inverse: Vec<Mat4> = rig.bind().iter().map(|&(q, p)| {
            let (q, p) = y_up(q, p);
            Mat4::from_rotation_translation(q, p).inverse()
        }).collect();
        Ok(Look { parts, bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(inverse)), body: Body::Source(rig) })
    }

    /// Bones and skinned meshes under `parent` (posed with `set_pose`). Returns the bones.
    pub fn spawn(&self, commands: &mut Commands, parent: Entity, materials: &mut Assets<StudioMaterial>) -> Vec<Entity> {
        let bones = match &self.body {
            Body::Source(rig) => rig.bind().len(),
            Body::Cs { model, .. } => model.model.bones.len().max(1),
        };
        let joints: Vec<Entity> = (0..bones).map(|_| commands.spawn((Transform::default(), ChildOf(parent))).id()).collect();
        for (mesh, material) in &self.parts {
            // bounds come from the bind pose, nowhere near the posed body: don't cull
            let skin = SkinnedMesh { inverse_bindposes: self.bindposes.clone(), joints: joints.clone() };
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(crate::models::own(materials, material)), LitPart, skin, NoFrustumCulling, ChildOf(parent)));
        }
        joints
    }

    /// Pose the character from the CS skeleton's pose (GoldSrc model space).
    pub fn set_pose(&self, joints: &[Entity], cs: &[BoneTransform], transforms: &mut Query<&mut Transform, With<ChildOf>>) {
        let rig = match &self.body {
            Body::Source(rig) => rig,
            Body::Cs { model, skeleton } => return crate::models::set_pose(joints, &model.model.borrow_pose(&skeleton.model, cs), transforms),
        };
        let scale = Vec3::splat(rig.scale);
        for (e, (q, p)) in joints.iter().zip(rig.pose(cs)) {
            if let Ok(mut t) = transforms.get_mut(*e) {
                let (rotation, translation) = y_up(q, p);
                *t = Transform { translation, rotation, scale };
            }
        }
    }
}

/// A material from `<name>.vmt` in `dir`: its base texture (a `.vtf` in `dir`) with cut-out,
/// blended and two-sided drawing as the material says.
fn material(dir: &Path, name: &str, images: &mut Assets<Image>, materials: &mut Assets<StudioMaterial>) -> Handle<StudioMaterial> {
    let vmt = Material::load(dir.join(format!("{name}.vmt"))).unwrap_or_default();
    let file = vmt.base_texture.rsplit('/').next().filter(|f| !f.is_empty()).unwrap_or(name).to_string();
    let image = match Texture::load(dir.join(format!("{file}.vtf"))) {
        Ok(t) => images.add(crate::level::rgba_image(t.width, t.height, t.rgba, true, TextureFormat::Rgba8UnormSrgb)),
        Err(e) => {
            warn!("character texture {}/{file}.vtf: {e}", dir.display());
            Handle::default()
        }
    };
    let mut m = crate::studio::material(image);
    m.base.alpha_mode = if vmt.translucent { AlphaMode::Blend } else if vmt.alpha_test { AlphaMode::Mask(0.5) } else { AlphaMode::Opaque };
    if vmt.double_sided {
        m.base.double_sided = true;
        m.base.cull_mode = None;
    }
    materials.add(m)
}
