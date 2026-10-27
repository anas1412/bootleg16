## A faction base behind the home screen (base_strays.tscn, a street hideout; base_bureau.tscn,
## an agency room; both built by tools/build_bases.gd). Your squad stands in it: you in front,
## up to four more behind on the scene's slots (Slots/0..4, each with its pose), under a fixed
## camera. `framing` 1 moves the camera so the squad stands right of centre, leaving the left
## for a panel. Same calls as the old lobby stage: set_squad(count), set_side(color).
extends Node3D

## The squad's looks until characters come from the Locker: per side, slot 0 is you.
const LOOKS := {
	"strays": [
		{"body": "Female", "skin": "Light", "hair": ["Hair_Buns"], "hair_color": Color(0.32, 0.07, 0.06)},
		{"body": "Male", "skin": "Dark", "hair": ["Hair_Buzzed"], "hair_color": Color(0.05, 0.04, 0.035)},
		{"body": "Female", "skin": "Dark", "hair": ["Hair_BuzzedFemale"], "hair_color": Color(0.85, 0.3, 0.55)},
		{"body": "Male", "skin": "Light", "hair": ["Hair_Long", "Hair_Beard"], "hair_color": Color(0.16, 0.11, 0.07)},
		{"body": "Male", "skin": "Light", "hair": ["Hair_SimpleParted"], "hair_color": Color(0.55, 0.45, 0.3)},
	],
	"bureau": [
		{"body": "Female", "skin": "Dark", "hair": ["Hair_Long"], "hair_color": Color(0.04, 0.035, 0.03)},
		{"body": "Male", "skin": "Light", "hair": ["Hair_SimpleParted", "Hair_Beard"], "hair_color": Color(0.12, 0.08, 0.05)},
		{"body": "Female", "skin": "Light", "hair": ["Hair_Buns"], "hair_color": Color(0.45, 0.33, 0.2)},
		{"body": "Male", "skin": "Dark", "hair": ["Hair_Buzzed"], "hair_color": Color(0.05, 0.04, 0.035)},
		{"body": "Female", "skin": "Light", "hair": ["Hair_SimpleParted"], "hair_color": Color(0.2, 0.2, 0.22)},
	],
}
## Camera: [position, look-at] for each framing.
const FRAMING := [
	[Vector3(0, 1.42, 7.4), Vector3(0, 1.0, -0.7)],
	[Vector3(-1.25, 1.42, 7.4), Vector3(-1.25, 1.0, -0.7)],
]

@export var side := "strays"
@export var members := 1
@export var framing := 0

var _figures: Array[Node3D] = []

func _ready() -> void:
	var looks: Array = LOOKS[side]
	for slot in $Slots.get_children():
		var f := preload("res://scripts/menu/figure.gd").new()
		f.look = looks[slot.get_index()].duplicate()
		f.look.pose = slot.get_meta("pose", "Idle")
		slot.add_child(f)
		_figures.append(f)
	var at: Array = FRAMING[framing]
	$Camera.position = at[0]
	$Camera.look_at(at[1])
	set_squad(members)

func set_squad(count: int) -> void:
	members = clampi(count, 1, _figures.size())
	for i in _figures.size():
		var on := i < members
		_figures[i].visible = on
		# a slot's props (the crate someone sits on) go with them
		for p in get_tree().get_nodes_in_group("slot_%d" % i):
			p.visible = on

## The rim lights take the side's colour (the base is already the side's).
func set_side(color: Color) -> void:
	for l in $Lights.get_children():
		if l.has_meta("rim"):
			l.light_color = color

## Where each member's name goes on screen: just over their head.
func name_points() -> Array:
	var out := []
	for i in members:
		out.append($Camera.unproject_position(_figures[i].head_top()))
	return out
