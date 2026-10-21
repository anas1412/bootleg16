//! One character, ready to draw: its Source model turned into GPU-skinned meshes with our
//! lighting material, and the rig that poses it from the CS skeleton each frame.
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

/// A character's meshes, materials and rig.
pub struct Look {
    parts: Vec<(Handle<Mesh>, Handle<StudioMaterial>)>,
    bindposes: Handle<SkinnedMeshInverseBindposes>,
    pub rig: Rig,
}

/// Z-up model space to Bevy's Y-up: (x, y, z) to (x, z, -y), for points and rotations.
fn y_up(q: Quat, p: Vec3) -> (Quat, Vec3) {
    (Quat::from_xyzw(q.x, q.z, -q.y, q.w), Vec3::from(map::to_y_up(p.to_array())))
}

impl Look {
    /// Loads `dir/model.mdl` with its materials (each `<name>.vmt` and the `.vtf` it names,
    /// all in `dir`). `cs` is the CS model whose animations the character plays.
    pub fn load(dir: &Path, cs: &CsModel, meshes: &mut Assets<Mesh>, bindposes: &mut Assets<SkinnedMeshInverseBindposes>, images: &mut Assets<Image>, materials: &mut Assets<StudioMaterial>) -> Result<Look, String> {
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
        Ok(Look { parts, bindposes: bindposes.add(SkinnedMeshInverseBindposes::from(inverse)), rig })
    }

    /// Bones and skinned meshes under `parent` (posed with `set_pose`). Returns the bones.
    pub fn spawn(&self, commands: &mut Commands, parent: Entity, materials: &mut Assets<StudioMaterial>) -> Vec<Entity> {
        let joints: Vec<Entity> = (0..self.rig.bind().len()).map(|_| commands.spawn((Transform::default(), ChildOf(parent))).id()).collect();
        for (mesh, material) in &self.parts {
            // bounds come from the bind pose, nowhere near the posed body: don't cull
            let skin = SkinnedMesh { inverse_bindposes: self.bindposes.clone(), joints: joints.clone() };
            commands.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(crate::models::own(materials, material)), LitPart, skin, NoFrustumCulling, ChildOf(parent)));
        }
        joints
    }

    /// Pose the character from the CS skeleton's pose (GoldSrc model space).
    pub fn set_pose(&self, joints: &[Entity], cs: &[BoneTransform], transforms: &mut Query<&mut Transform, With<ChildOf>>) {
        let scale = Vec3::splat(self.rig.scale);
        for (e, (q, p)) in joints.iter().zip(self.rig.pose(cs)) {
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
