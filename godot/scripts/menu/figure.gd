## One squad member standing in a faction base: a Quaternius base body as it comes (CC0, files
## untouched; clothes come later), a hairstyle on the head bone and an idle from the Universal
## Animation Library on the same rig.
## `look` is {body: "Female" | "Male", skin: "Light" | "Dark", hair: [names], hair_color: Color,
## pose: "Idle"}.
extends Node3D

const DIR := "res://characters/quaternius/"
const SKIN := {
	"Female": {"Light": "T_Superhero_Female_Light_BaseColor.png", "Dark": "T_Superhero_Female_Dark_BaseColor.png"},
	"Male": {"Light": "T_Superhero_Male_Ligh.png", "Dark": "T_Superhero_Male_Dark.png"},
}

static var _anims: AnimationLibrary

var look := {}
var skeleton: Skeleton3D

func _ready() -> void:
	var body: Node3D = load(DIR + "Superhero_%s_FullBody.gltf" % look.body).instantiate()
	add_child(body)
	skeleton = body.get_node("Armature/Skeleton3D")
	# the body is the mesh that isn't the eyes or the eyebrows
	var mesh: MeshInstance3D = skeleton.get_children().filter(func(c): return c is MeshInstance3D and not c.name.begins_with("Eye"))[0]
	# the pack's lighter or darker skin texture
	var skin: BaseMaterial3D = mesh.get_active_material(0).duplicate()
	skin.albedo_texture = load(DIR + SKIN[look.body][look.get("skin", "Light")])
	mesh.material_override = skin
	for h in look.get("hair", []):
		var hair: Node3D = load(DIR + "hair/%s.gltf" % h).instantiate()
		for m in hair.find_children("*", "MeshInstance3D", true, false):
			m.get_parent().remove_child(m)
			m.owner = null
			skeleton.add_child(m)
			m.skeleton = NodePath("..")
		hair.free()
	# the hair textures are grey, made to be tinted; the eyebrows come with the body
	for m in skeleton.get_children():
		if m is MeshInstance3D and (m.name.begins_with("Hair") or m.name == "Eyebrows"):
			var mat: BaseMaterial3D = m.get_active_material(0).duplicate()
			mat.albedo_color = look.get("hair_color", Color(0.12, 0.09, 0.07))
			m.material_override = mat
	var player := AnimationPlayer.new()
	body.add_child(player)
	player.add_animation_library("", _library())
	player.play(look.get("pose", "Idle"))
	# members don't breathe in step
	player.seek(randf() * player.current_animation_length, true)

## The head's top in world space, for the name over it.
func head_top() -> Vector3:
	var i := skeleton.find_bone("Head")
	return skeleton.global_transform * skeleton.get_bone_global_pose(i).origin + Vector3.UP * 0.28

## The library's animations without their bone position tracks (except the hips'), so each
## body keeps its own proportions; loaded once.
static func _library() -> AnimationLibrary:
	if _anims:
		return _anims
	var ual: Node = load(DIR + "UAL1_Standard.glb").instantiate()
	var src: AnimationLibrary = ual.get_node("AnimationPlayer").get_animation_library("")
	_anims = AnimationLibrary.new()
	for n in src.get_animation_list():
		var a: Animation = src.get_animation(n).duplicate()
		for t in range(a.get_track_count() - 1, -1, -1):
			if a.track_get_type(t) == Animation.TYPE_POSITION_3D and not String(a.track_get_path(t)).ends_with(":pelvis"):
				a.remove_track(t)
		_anims.add_animation(n, a)
	ual.free()
	return _anims
