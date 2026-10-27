# Builds the placeholder character: a clay mannequin made with MPFB (MakeHuman for Blender), on the
# game_engine skeleton the real characters will share, so animations and hitbox work carry over.
# Run through tools/make_character.sh, which installs MPFB into a profile inside the project
# (design/assets/blender/profile) so the user's own Blender is never touched.
#
# Args after "--": <asset pack zips dir> <output .glb>
import math
import os
import sys

import bpy
from mathutils import Matrix

from bl_ext.user_default.mpfb.services import AssetService, ExportService, HumanService, LocationService

packs_dir, out_path = sys.argv[sys.argv.index("--") + 1:]

# MakeHuman system assets (CC0), unpacked once into MPFB's data dir; only the eyes are used.
data = LocationService.get_user_data()
if not os.path.exists(os.path.join(data, "eyes")):
    AssetService.fix_and_extract_asset_pack_zip(os.path.join(packs_dir, "mh_system.zip"), data)
AssetService.update_all_asset_lists()

# Neutral adult body at the default height: one hitbox for everyone.
human = HumanService._create_default_human_info_dict()
human["name"] = "mannequin"
human["rig"] = "game_engine"
human["eyes"] = "high-poly/high-poly.mhclo"  # without them the sockets are holes

settings = HumanService.get_default_deserialization_settings()
settings["subdiv_levels"] = 0
basemesh = HumanService.deserialize_from_dict(human, settings)

# Bake targets into the mesh and strip the helper geometry, on a copy.
copy = ExportService.create_character_copy(basemesh)
body = copy if copy.type == "MESH" else next(c for c in copy.children if c.type == "MESH" and "body" in c.name)
ExportService.bake_modifiers_remove_helpers(body, bake_masks=True, remove_helpers=True)
rig = body.parent

# One dark graphite clay material on everything: the rim light carves the silhouette.
clay = bpy.data.materials.new("clay")
bsdf = clay.node_tree.nodes["Principled BSDF"]
bsdf.inputs["Base Color"].default_value = (0.09, 0.095, 0.105, 1.0)
bsdf.inputs["Roughness"].default_value = 0.45
for child in rig.children:
    if child.type == "MESH":
        child.data.materials.clear()
        child.data.materials.append(clay)

# MakeHuman stands in an A pose. Key a one-frame "Stand" action with the upper arms lowered
# to hang at the sides (85 degrees below horizontal), rotated about the world Y axis.
bpy.context.view_layer.objects.active = rig
bpy.ops.object.mode_set(mode="POSE")
for name in ("upperarm_l", "upperarm_r"):
    pb = rig.pose.bones[name]
    v = (pb.tail - pb.head).normalized()
    side = 1.0 if v.x > 0 else -1.0
    turn = math.radians(85) - math.asin(max(-1.0, min(1.0, -v.z)))
    about = Matrix.Translation(pb.head) @ Matrix.Rotation(side * turn, 4, "Y") @ Matrix.Translation(-pb.head)
    pb.matrix = about @ pb.matrix
    bpy.context.view_layer.update()
rig.animation_data_create()
rig.animation_data.action = bpy.data.actions.new("Stand")
for pb in rig.pose.bones:
    pb.keyframe_insert("rotation_quaternion" if pb.rotation_mode == "QUATERNION" else "rotation_euler", frame=1)
bpy.ops.object.mode_set(mode="OBJECT")

bpy.ops.object.select_all(action="DESELECT")
rig.select_set(True)
for child in rig.children:
    child.select_set(True)
os.makedirs(os.path.dirname(out_path), exist_ok=True)
bpy.ops.export_scene.gltf(filepath=out_path, export_format="GLB", use_selection=True, export_apply=True)
print("exported", out_path)
