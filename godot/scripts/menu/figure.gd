## One squad member standing in a faction base: a Quaternius base body (CC0, files untouched)
## dressed in its side's clothes by shaders/outfit.gdshader, a hairstyle on the head bone and
## an idle from the Universal Animation Library on the same rig.
## `look` is {body: "Female" | "Male", skin: "Light" | "Dark", hair: [names], hair_color: Color,
## pose: "Idle"}.
extends Node3D

const DIR := "res://characters/quaternius/"
const SKIN := {
	"Female": {"Light": "T_Superhero_Female_Light_BaseColor.png", "Dark": "T_Superhero_Female_Dark_BaseColor.png"},
	"Male": {"Light": "T_Superhero_Male_Ligh.png", "Dark": "T_Superhero_Male_Dark.png"},
}
## The sides' clothes: the Bureau in dark suits over a white shirt, the Strays in street wear
## with their colour down the sleeves.
const CLOTHES := {
	"bureau": {"top": Color(0.055, 0.06, 0.075), "trousers": Color(0.05, 0.055, 0.068), "shoes": Color(0.02, 0.02, 0.022), "shirt": Color(0.8, 0.82, 0.84), "stripe": Tokens.BUREAU},
	"strays": {"top": Color(0.1, 0.092, 0.1), "trousers": Color(0.12, 0.12, 0.095), "shoes": Color(0.72, 0.71, 0.67), "shirt": Color(0.1, 0.092, 0.1), "stripe": Tokens.STRAYS},
}
## Zones as fractions of the T-pose's height (y) and half span (x); see outfit.gdshader.
const CHIN := 0.86
const NECK := 0.826
const COLLAR := 0.075  # the neckline's half width, metres
const WRIST := 0.765
const WAIST := 0.53
const HEM := 0.47  # the Bureau's jacket hangs below the waist
const ANKLE := 0.075
const SHOULDER := 0.24

static var _anims: AnimationLibrary

var look := {}
var side := "strays"
var skeleton: Skeleton3D

func _ready() -> void:
	var body: Node3D = load(DIR + "Superhero_%s_FullBody.gltf" % look.body).instantiate()
	add_child(body)
	skeleton = body.get_node("Armature/Skeleton3D")
	# the body is the mesh that isn't the eyes or the eyebrows
	var mesh: MeshInstance3D = skeleton.get_children().filter(func(c): return c is MeshInstance3D and not c.name.begins_with("Eye"))[0]
	mesh.mesh = _zoned(mesh.mesh)
	mesh.material_override = _outfit()
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

## The body mesh again with each vertex's zone in its colour.
func _zoned(src: ArrayMesh) -> ArrayMesh:
	var arrays := src.surface_get_arrays(0)
	var verts: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
	var normals: PackedVector3Array = arrays[Mesh.ARRAY_NORMAL]
	var box := src.get_aabb()
	var h := box.size.y
	var w := box.size.x * 0.5
	var bureau := side == "bureau"
	var colors := PackedColorArray()
	colors.resize(verts.size())
	for i in verts.size():
		var v := verts[i]
		var ax := absf(v.x)
		var zone := 1
		if v.y > h * CHIN or (v.y > h * NECK and ax < COLLAR) or ax > w * WRIST:
			zone = 0
		elif v.y < h * ANKLE:
			zone = 3
		elif v.y < h * (HEM if bureau else WAIST):
			zone = 2
		elif bureau and v.z > 0.0 and ax < (v.y - h * 0.74) * 0.3:
			zone = 4  # the shirt's V from the collar to the third button
		elif not bureau and ax > w * SHOULDER and normals[i].y > 0.82:
			zone = 5
		colors[i] = Color(zone / 8.0, 0, 0)
	arrays[Mesh.ARRAY_COLOR] = colors
	var out := ArrayMesh.new()
	for b in src.get_blend_shape_count():
		out.add_blend_shape(src.get_blend_shape_name(b))
	out.blend_shape_mode = src.blend_shape_mode
	# keeps the custom channels' formats and the bone count; drops compression (colours are new)
	var flags := src.surface_get_format(0) & ~Mesh.ARRAY_FLAG_COMPRESS_ATTRIBUTES
	out.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays, src.surface_get_blend_shape_arrays(0), {}, flags)
	return out

func _outfit() -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = preload("res://shaders/outfit.gdshader")
	var g: String = look.body
	m.set_shader_parameter("skin_tex", load(DIR + SKIN[g][look.get("skin", "Light")]))
	m.set_shader_parameter("normal_tex", load(DIR + "T_Superhero_%s_Normal.png" % g))
	m.set_shader_parameter("rough_tex", load(DIR + "T_Superhero_%s_Roughness.png" % g))
	var c: Dictionary = CLOTHES[side]
	for k in c:
		m.set_shader_parameter(k, c[k])
	return m

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
