## Builds the data_center grey box (godot/maps/data_center.tscn) from the layout below:
##   godot --headless --path godot --script res://tools/build_data_center.gd
## then export its .bmap with tools/export_map.gd. The layout is the structure of the classic
## two-site map (docs/MAPS.md): its lanes, sites, chokes and distances, measured as numbers
## with `net/examples/mapstats.rs`, laid out again from our own blocks.
##
## Everything here is in GoldSrc units (x east, y north, z up, 1 unit = 1 inch), as the
## measurements are. Areas are where players walk: a floor (solid down to the base), a ramp
## (its `z` is [height at the low corner, height at the high corner] along `axis`) or a bridge
## (a thin slab over another area). Every 16-unit cell no area covers becomes wall (every area edge is on that grid). Over each
## area its ceiling (CEILINGS): a roof indoors, the sky outdoors. Blocks are cover and door frames.
extends SceneTree

const METRES := 0.0254
const CELL := 16
const BASE := -320
const TOP := 512
const OUT := "res://maps/data_center.tscn"
## Ceiling lamps: their colour, how far apart (units).
const LAMP := Color(0.85, 0.93, 1.0)
const LAMP_EVERY := 384.0

## [name, [x0, y0, x1, y1], z or [z0, z1], extra]
## extra: {"axis": "x" | "y"} for ramps, {"bridge": true}.
const AREAS := [
	# Strays side: the loading dock (spawn) and its ramps
	["dock", [-2304, -1024, -1024, -640], 128, {}],
	["dock_north", [-1792, -640, -1024, -256], 128, {}],
	["dock_east", [-1024, -1024, -512, -640], 128, {}],
	["dock_gap", [-1792, -256, -1632, -128], 128, {}],
	["dock_ramp", [-512, -1024, 0, -576], [128, 0], {"axis": "x"}],
	["dock_lane", [0, -768, 256, -576], 0, {}],
	# to B: the intake yard, then the cable crawlspace under the floor
	["intake_ramp", [-2048, -640, -1792, -128], [128, 0], {"axis": "y"}],
	["intake_steps", [-1632, -256, -1536, 32], [128, 0], {"axis": "y"}],
	["intake", [-2048, -128, -1536, 512], 0, {}],
	["crawl_mouth", [-2048, 512, -1536, 704], 0, {}],
	["crawl_neck", [-1792, 704, -1664, 1024], 0, {}],
	["crawl_cross", [-2048, 1024, -1408, 1280], 0, {}],
	["crawl_east", [-1408, 1088, -1184, 1248], 0, {}],
	["crawl_drop", [-1184, 1088, -1056, 1280], [0, -112], {"axis": "y"}],
	["crawl_low", [-1184, 1280, -512, 1536], -112, {}],
	["crawl_up", [-2048, 1280, -1920, 1792], 0, {}],
	# B: the archive hall
	["hall_b", [-2112, 1792, -1376, 2944], 0, {}],
	["hall_b_back", [-2048, 2944, -1792, 3072], 0, {}],
	["b_doors", [-1376, 2080, -1248, 2272], 0, {}],
	# Bureau side of mid: the lobby down to B's doors, the control room (spawn)
	["lobby_west", [-1248, 2048, -768, 2688], [0, -128], {"axis": "x"}],
	["lobby", [-768, 2048, -288, 2560], -128, {}],
	["lobby_link", [-288, 2048, 64, 2304], -128, {}],
	["control", [64, 2048, 576, 2496], -128, {}],
	["control_ramp", [576, 2176, 1024, 2304], [-128, 0], {"axis": "x"}],
	# mid: the spine corridor, its security doors and the raised-floor walkway
	["spine_stem", [-512, -576, -384, -384], [112, 0], {"axis": "y"}],
	["spine_south", [-512, -384, -384, 224], 0, {}],
	["spine_top", [-672, 224, 256, 576], 0, {}],
	["spine", [-512, 576, -96, 768], 0, {}],
	["spine_ramp", [-512, 768, -272, 1280], [0, -128], {"axis": "y"}],
	["spine_low", [-512, 1280, -272, 1696], -128, {}],
	["spine_north", [-512, 1696, -256, 2048], -128, {}],
	["spine_side", [-272, 768, -96, 1568], 0, {}],
	["walkway", [-96, 1408, 384, 1568], 16, {}],
	["walkway_turn", [384, 1408, 512, 1632], 16, {}],
	["walkway_stairs", [384, 1632, 512, 1984], [16, 96], {"axis": "y"}],
	["walkway_top", [384, 1984, 512, 2048], 96, {}],
	["walkway_bridge", [288, 2048, 576, 2496], 96, {"bridge": true}],
	# to A: the yard, the aisle doors, the main server aisle and its sump
	["yard_south", [128, -576, 256, -352], 0, {}],
	["yard_lot", [128, -352, 704, 256], 0, {}],
	["yard_link", [512, 256, 704, 768], 0, {}],
	["yard", [512, 768, 736, 1184], 0, {}],
	["aisle_doors", [736, 768, 1152, 1088], 0, {}],
	["aisle_mouth", [960, 288, 1280, 768], 0, {}],
	["aisle_entry", [1152, 768, 1280, 1184], 0, {}],
	["sump", [1280, 0, 1536, 256], -192, {}],
	["sump_ramp", [1280, 256, 1536, 768], [-192, 0], {"axis": "y"}],
	["aisle_ledge", [1600, 384, 1696, 1088], 16, {}],
	["aisle", [1280, 768, 1600, 2176], 0, {}],
	["aisle_end", [1600, 1792, 1664, 2304], 0, {}],
	["a_floor", [1024, 2176, 1536, 2304], 0, {}],
	# A: the core hall
	["a_ramp", [1344, 2304, 1600, 2752], [0, 112], {"axis": "y"}],
	["a_deck_west", [288, 2496, 576, 2752], 96, {}],
	["a_deck", [576, 2304, 1344, 2752], 96, {}],
	["a_back", [1088, 2752, 1600, 3072], 112, {}],
]

## Blocks: [name, [x0, y0, x1, y1], [z0, z1], what it is]. Each is something a data center
## has, and looks it (PROPS); "" is part of the building (frames, headers, a column).
const BLOCKS := [
	# the spine's security doors: a frame 192 wide (its two panels are LEAVES), a header
	["spine_door_w", [-512, 1592, -480, 1672], [-128, TOP], ""],
	["spine_door_e", [-288, 1592, -256, 1672], [-128, TOP], ""],
	["spine_door_head", [-480, 1592, -288, 1672], [0, TOP], ""],
	# the archive doors (B): a frame 176 wide (its panels are LEAVES), a header
	["b_door_s", [-1344, 2080, -1312, 2096], [0, TOP], ""],
	["b_door_head", [-1344, 2096, -1312, 2272], [144, TOP], ""],
	# a low header where the control room opens onto the ramp to A (under the walkway deck)
	["control_head", [528, 2176, 560, 2304], [0, 96], ""],
	# the aisle doors: a header over the 320-unit passage
	["aisle_door_head", [912, 768, 944, 1088], [144, TOP], ""],
	# a column holding up the floor over the low crawl
	["crawl_pillar", [-896, 1280, -800, 1376], [-112, 32], ""],
	# cover (the reference's), each a thing with a reason to be there
	["dock_crates", [-1152, -640, -1024, -448], [128, 256], "delivery"],
	["spine_kerb", [-272, 768, -240, 1344], [-128, 32], "kerb"],
	["spine_kerb_north", [-272, 1488, -240, 1568], [-128, 32], "kerb"],
	# the xbox: crated servers on their way in, against the kerb (the reference's box, 96 up
	# from the lane); the kerb is open beside it and a smaller crate stands in front, so you
	# climb from the lane to the walkway side in three jumps (48, 48, 32), as in CS2
	["spine_crates", [-384, 1344, -240, 1488], [-128, -32], "delivery"],
	["spine_crate_step", [-384, 1296, -320, 1344], [-128, -80], "delivery"],
	["walkway_ups", [400, 1408, 496, 1520], [16, 96], "ups"],
	["aisle_boxes", [1536, 1984, 1600, 2112], [0, 64], "boxes"],
	["sump_pump", [960, 288, 1056, 384], [0, 64], "pump"],
	["core_servers", [1024, 2432, 1152, 2560], [96, 160], "core"],
	["a_kerb", [1280, 2304, 1344, 2752], [96, 128], "kerb"],
	["tape_library", [-1920, 2304, -1792, 2432], [0, 96], "cabinet"],
	["archive_boxes", [-1664, 2560, -1536, 2688], [0, 64], "archive"],
	["archive_box", [-1536, 2560, -1472, 2624], [0, 32], "archive"],
	["bureau_barrier", [-160, 2208, -32, 2304], [-128, -64], "barrier"],
]

## How each kind of block looks: [prop.gdshader kind, body colour, light colour].
const PROPS := {
	"cabinet": [0, Color(0.36, 0.38, 0.41), Color(0.25, 1.0, 0.55)],
	"core": [0, Color(0.30, 0.31, 0.34), Color(1.0, 0.76, 0.24)],
	"delivery": [1, Color(0.55, 0.43, 0.30), Color()],
	"boxes": [1, Color(0.58, 0.48, 0.34), Color()],
	"archive": [1, Color(0.46, 0.49, 0.52), Color()],
	"ups": [2, Color(0.30, 0.32, 0.35), Color()],
	"pump": [2, Color(0.34, 0.40, 0.48), Color()],
	"barrier": [3, Color(0.55, 0.56, 0.56), Color()],
	"kerb": [4, Color(0.40, 0.42, 0.45), Color()],
	"door": [5, Color(0.40, 0.46, 0.54), Color(0.29, 0.78, 1.0)],
}

## The height of each area's ceiling, measured on the reference (the median of 3 x 3 points
## over the area, mapstats sightlines straight up): ["roof", z] indoors, ["sky", z] outdoors
## (the dock and the yards; not drawn, it stops grenades as the reference's sky does); a third
## item: over that part of the area only (the rest is under the walkway bridge). Areas not
## named here are open to the sky over everything (TOP).
const CEILINGS := {
	"dock_east": ["sky", 384], "dock_ramp": ["sky", 384], "dock_lane": ["sky", 384],
	"spine_stem": ["sky", 384], "spine_south": ["sky", 384], "spine_top": ["sky", 384],
	"yard_south": ["sky", 384], "yard_lot": ["sky", 384], "yard": ["sky", 352],
	"yard_link": ["roof", 192],
	"crawl_neck": ["roof", 192], "crawl_cross": ["roof", 192], "crawl_east": ["roof", 192],
	"crawl_drop": ["roof", 192], "crawl_up": ["roof", 192], "crawl_low": ["roof", 32],
	"hall_b": ["roof", 384], "hall_b_back": ["roof", 384], "b_doors": ["roof", 146],
	"lobby_west": ["roof", 384], "lobby": ["roof", 384], "lobby_link": ["roof", 64],
	"control": ["roof", 64, [64, 2048, 288, 2496]], "walkway_bridge": ["roof", 352],
	"control_ramp": ["roof", 352],
	"spine": ["roof", 384], "spine_ramp": ["roof", 384], "spine_low": ["roof", 384],
	"spine_north": ["roof", 384], "spine_side": ["roof", 384],
	"walkway": ["roof", 384], "walkway_turn": ["roof", 384], "walkway_stairs": ["roof", 384],
	"walkway_top": ["roof", 352], "a_deck_west": ["roof", 352],
	"aisle_doors": ["roof", 352], "aisle_mouth": ["roof", 352], "aisle_entry": ["roof", 352],
	"sump": ["roof", 352], "sump_ramp": ["roof", 352], "aisle_ledge": ["roof", 352],
	"aisle": ["roof", 352], "aisle_end": ["roof", 352], "a_floor": ["roof", 352],
	"a_ramp": ["roof", 352], "a_deck": ["roof", 352], "a_back": ["roof", 352],
}

## Dressing (looks only: meshes the exporter skips, nothing players or bullets touch).
## What lines the walls of each area, and how tall (units): server racks in the halls and the
## aisle, cable bundles low along the crawlspace, condensers along the outdoor walls.
const DRESS := {
	"racks": ["aisle", "aisle_end", "aisle_entry", "aisle_mouth", "aisle_doors", "sump_ramp", "a_floor", "a_ramp", "a_deck", "a_deck_west", "a_back", "hall_b", "hall_b_back", "lobby", "spine_north", "walkway_top"],
	"cables": ["crawl_neck", "crawl_cross", "crawl_east", "crawl_up", "crawl_low"],
	"condensers": ["dock", "dock_north", "dock_east", "dock_gap", "dock_ramp", "dock_lane", "intake_ramp", "intake_steps", "intake", "crawl_mouth", "yard_south", "yard_lot", "yard_link", "yard", "spine_top"],
}
const DRESS_HEIGHT := {"racks": 84.0, "cables": 56.0, "condensers": 56.0}
## The landmark: cooling towers west of the dock, steaming (GoldSrc x, y, and their height).
const TOWERS := [[-3000, -900, 1300], [-3050, -100, 1400], [-2950, 700, 1250]]

## Door panels standing at an angle (security doors, PROPS "door"): [name, [x, y] one end, [x, y] the other, thickness, [z0, z1]].
## Doors standing half open, parallel, leave a slanted gap between their panels.
const LEAVES := [
	["spine_leaf_w", [-480, 1632], [-392, 1591], 8, [-128, 0]],
	["spine_leaf_e", [-376, 1674], [-288, 1633], 8, [-128, 0]],
	# the archive doors stand the same way
	["b_leaf_s", [-1304, 2096], [-1272, 2166], 8, [0, 128]],
	["b_leaf_n", [-1347, 2196], [-1316, 2268], 8, [0, 128]],
]

## Spawns: [side, x, y, floor z, yaw (GoldSrc degrees)]
const SPAWNS := [
	["t", -976, -896, 128, 90], ["t", -848, -896, 128, 90], ["t", -720, -896, 128, 90], ["t", -592, -896, 128, 90], ["t", -464, -896, 128, 90],
	["t", -976, -704, 128, 90], ["t", -848, -704, 128, 90], ["t", -720, -704, 128, 90], ["t", -592, -704, 128, 90], ["t", -464, -704, 128, 90],
	["ct", 96, 2176, -128, 270], ["ct", 192, 2176, -128, 270], ["ct", 288, 2176, -128, 270], ["ct", 384, 2176, -128, 270], ["ct", 96, 2304, -128, 270],
	["ct", 192, 2304, -128, 270], ["ct", 288, 2304, -128, 270], ["ct", 384, 2304, -128, 270], ["ct", 192, 2432, -128, 270], ["ct", 288, 2432, -128, 270],
]

## Zones: [group, [x0, y0, z0, x1, y1, z1]]; a site's box starts at its floor (bots take the
## box's bottom for the site's level).
const ZONES := [
	["buy_t", [-2304, -1024, 96, -416, -256, 256]],
	["buy_ct", [-288, 2048, -160, 576, 2560, 160]],
	["site_a", [1024, 2304, 96, 1280, 2624, 256]],
	["site_b", [-1728, 2496, 0, -1344, 2880, 160]],
]

## Site marks: [letter, colour, light (x, y, z), painted letter (x, y), wall letter (x, y, z),
## the way the wall letter faces (GoldSrc yaw)]
const SITES := [
	["A", Color(1.0, 0.76, 0.24), [1152, 2464, 300], [1216, 2368], [1344, 3070, 250], 270],
	["B", Color(0.24, 1.0, 0.63), [-1536, 2688, 224], [-1504, 2784], [-2110, 2688, 192], 0],
]

var _mats := {}
var _root: Node3D
var _lit: Node3D
var _skies: Node3D
var _lamp: StandardMaterial3D
var _dress_mats := {}
var _prop_mats := {}

func _initialize() -> void:
	_root = Node3D.new()
	_root.name = "DataCenter"
	_root.set_meta("bomb_radius", 500.0)
	_mats = {
		"floor": _grid(Color(0.40, 0.42, 0.45)),
		"wall": _grid(Color(0.47, 0.48, 0.50)),
		"cover": _grid(Color(0.40, 0.43, 0.48)),
		"roof": _grid(Color(0.26, 0.27, 0.29)),
		"raised": _shader("raised_floor"),
	}
	_dress_mats = {"racks": _shader("rack"), "cables": _shader("cables"), "condensers": _shader("condenser")}
	# one solid, drawn as the union of its boxes: no two faces in the same place flicker
	# (z-fighting) and nothing hidden inside is drawn; the exporter still reads every box
	var solid := CSGCombiner3D.new()
	solid.name = "Solid"
	_add(_root, solid)
	var floors := _combiner(solid, "Floors")
	var walls := _combiner(solid, "Walls")
	var blocks := _combiner(solid, "Blocks")
	_lit = _group("Lights")
	_skies = _group("Skies")
	_lamp = StandardMaterial3D.new()
	_lamp.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	_lamp.albedo_color = LAMP
	for a in AREAS:
		_area(floors, a)
	_walls(walls)
	var props := _group("Props")
	for b in BLOCKS:
		var box := _box(blocks, b[0], b[1][0], b[1][1], b[2][0], b[1][2], b[1][3], b[2][1], "wall" if b[3] == "" else "cover", {"material": "C" if b[3] == "" else "M"})
		if b[3] != "":
			_prop(props, box, b[3])
	for l in LEAVES:
		_prop(props, _leaf(blocks, l), "door")
	_sky()
	_dress(_group("Dressing"))
	_landmark(_group("Landmark"))
	_spawns()
	_zones()
	_marks()
	var packed := PackedScene.new()
	packed.pack(_root)
	var err := ResourceSaver.save(packed, OUT)
	print("%s: %d areas, %d wall boxes, %d blocks (%s)" % [OUT, AREAS.size(), walls.get_child_count(), BLOCKS.size(), error_string(err)])
	_root.free()
	quit(0 if err == OK else 1)

func _shader(name: String) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = load("res://shaders/%s.gdshader" % name)
	return m

## Indoors (under a roof) the floor is raised tiles; outdoors and in the crawlspace, concrete.
func _floor_mat(name: String) -> String:
	var c: Array = CEILINGS.get(name, [])
	return "raised" if not c.is_empty() and c[0] == "roof" and not name.begins_with("crawl") else "floor"

func _grid(base: Color) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = load("res://shaders/grid.gdshader")
	m.set_shader_parameter("base", base)
	m.set_shader_parameter("line", base.darkened(0.3))
	return m

func _combiner(parent: Node, name: String) -> CSGCombiner3D:
	var n := CSGCombiner3D.new()
	n.name = name
	_add(parent, n)
	return n

func _group(name: String) -> Node3D:
	var n := Node3D.new()
	n.name = name
	_add(_root, n)
	return n

func _add(parent: Node, child: Node) -> void:
	parent.add_child(child)
	child.owner = _root

## A Godot point (metres, y up) from GoldSrc units.
func _p(x: float, y: float, z: float) -> Vector3:
	return Vector3(x, z, -y) * METRES

## An axis-aligned box between two GoldSrc corners.
func _box(parent: Node, name: String, x0: float, y0: float, z0: float, x1: float, y1: float, z1: float, mat: String, meta := {}) -> CSGBox3D:
	var b := CSGBox3D.new()
	b.name = name
	b.size = Vector3(x1 - x0, z1 - z0, y1 - y0) * METRES
	b.position = _p((x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2)
	b.material = _mats[mat]
	for k in meta:
		b.set_meta(k, meta[k])
	_add(parent, b)
	return b

## A box along the line between two points (GoldSrc), turned to follow it.
func _leaf(parent: Node, l: Array) -> CSGBox3D:
	var a := Vector2(l[1][0], l[1][1])
	var b := Vector2(l[2][0], l[2][1])
	var z: Array = l[4]
	var box := _box(parent, l[0], 0, 0, z[0], a.distance_to(b), l[3], z[1], "cover", {"material": "M"})
	var mid := (a + b) / 2.0
	box.position = _p(mid.x, mid.y, (z[0] + z[1]) / 2.0)
	# GoldSrc's angle from x towards y is a turn around Godot's up the same way
	box.rotation.y = (b - a).angle()
	return box

## A block's look: a box just bigger than it (half a unit all round), with its kind's pattern.
func _prop(parent: Node3D, box: CSGBox3D, kind: String) -> void:
	var look: Array = PROPS[kind]
	if not _prop_mats.has(kind):
		var m := _shader("prop")
		m.set_shader_parameter("kind", look[0])
		m.set_shader_parameter("body", look[1])
		m.set_shader_parameter("accent", look[2])
		_prop_mats[kind] = m
	var mesh := BoxMesh.new()
	mesh.size = box.size + Vector3.ONE * METRES
	mesh.material = _prop_mats[kind]
	var skin := MeshInstance3D.new()
	skin.name = box.name
	skin.mesh = mesh
	skin.transform = box.transform
	skin.set_instance_shader_parameter("size", mesh.size)
	_add(parent, skin)

func _area(parent: Node, a: Array) -> void:
	var name: String = a[0]
	var r: Array = a[1]
	var extra: Dictionary = a[3]
	if a[2] is Array:
		_ramp(parent, name, r, a[2], extra)
	elif extra.get("bridge", false):
		_box(parent, name, r[0], r[1], a[2] - 32, r[2], r[3], a[2], _floor_mat(name))
	else:
		_box(parent, name, r[0], r[1], BASE, r[2], r[3], a[2], _floor_mat(name))
	if CEILINGS.has(name):
		var c: Array = CEILINGS[name]
		if c.size() > 2:
			r = c[2]
		if c[0] == "roof":
			_box(parent, name + "_roof", r[0], r[1], c[1], r[2], r[3], TOP, "roof")
			_lights(name, r, c[1], a[2] if not (a[2] is Array) else min(a[2][0], a[2][1]))
		else:
			_sky_over(name, r, c[1])

## A ramp: solid up to its low end, and a slab whose top runs from z0 at the low-coordinate
## edge to z1 at the other.
func _ramp(parent: Node, name: String, r: Array, z: Array, extra: Dictionary) -> void:
	var lo_z: float = min(z[0], z[1])
	_box(parent, name, r[0], r[1], BASE, r[2], r[3], lo_z, _floor_mat(name))
	var along_x: bool = extra.axis == "x"
	var z0: float = z[0]
	var z1: float = z[1]
	# the top edge's ends (GoldSrc), across the middle of the ramp
	var mid_x: float = (r[0] + r[2]) / 2.0
	var mid_y: float = (r[1] + r[3]) / 2.0
	var start := _p(r[0], mid_y, z0) if along_x else _p(mid_x, r[1], z0)
	var end := _p(r[2], mid_y, z1) if along_x else _p(mid_x, r[3], z1)
	var width: float = (r[3] - r[1] if along_x else r[2] - r[0]) * METRES
	var along := (end - start).normalized()
	var x_axis := Vector3.UP.cross(along).normalized()
	var up := along.cross(x_axis)
	var thick: float = (absf(z1 - z0) + 96.0) * METRES
	var b := CSGBox3D.new()
	b.name = name + "_slope"
	b.size = Vector3(width, thick, (end - start).length())
	b.transform = Transform3D(Basis(x_axis, up, along), (start + end) / 2.0 - up * thick / 2.0)
	b.material = _mats[_floor_mat(name)]
	_add(parent, b)

func _covered(x: float, y: float) -> bool:
	for a in AREAS:
		var r: Array = a[1]
		if x > r[0] and x < r[2] and y > r[1] and y < r[3]:
			return true
	return false

## Walls: every cell no area covers, merged into as few boxes as rows allow.
func _walls(parent: Node) -> void:
	var lo := Vector2(INF, INF)
	var hi := -lo
	for a in AREAS:
		lo = lo.min(Vector2(a[1][0], a[1][1]))
		hi = hi.max(Vector2(a[1][2], a[1][3]))
	lo -= Vector2(CELL, CELL)
	hi += Vector2(CELL, CELL)
	var cols := int((hi.x - lo.x) / CELL)
	var rows := int((hi.y - lo.y) / CELL)
	var wall := []
	for r in rows:
		for c in cols:
			wall.append(not _covered(lo.x + (c + 0.5) * CELL, lo.y + (r + 0.5) * CELL))
	var used := []
	used.resize(wall.size())
	used.fill(false)
	var n := 0
	for r in rows:
		for c in cols:
			if not wall[r * cols + c] or used[r * cols + c]:
				continue
			var w := 1
			while c + w < cols and wall[r * cols + c + w] and not used[r * cols + c + w]:
				w += 1
			var h := 1
			while r + h < rows:
				var full := true
				for k in w:
					if not wall[(r + h) * cols + c + k] or used[(r + h) * cols + c + k]:
						full = false
						break
				if not full:
					break
				h += 1
			for dr in h:
				for k in w:
					used[(r + dr) * cols + c + k] = true
			n += 1
			_box(parent, "Wall%d" % n, lo.x + c * CELL, lo.y + r * CELL, BASE, lo.x + (c + w) * CELL, lo.y + (r + h) * CELL, TOP, "wall")

## Lamps under a roof, about LAMP_EVERY apart: a light and a bar to show where it comes from.
func _lights(name: String, r: Array, ceiling: float, floor_z: float) -> void:
	var nx := maxi(1, ceili((r[2] - r[0]) / LAMP_EVERY))
	var ny := maxi(1, ceili((r[3] - r[1]) / LAMP_EVERY))
	var reach := clampf((ceiling - floor_z) * 1.8, 160.0, 420.0)
	for i in nx:
		for j in ny:
			var x: float = r[0] + (r[2] - r[0]) * (i + 0.5) / nx
			var y: float = r[1] + (r[3] - r[1]) * (j + 0.5) / ny
			var light := OmniLight3D.new()
			light.name = "%s_lamp%d" % [name, i * ny + j]
			light.light_color = LAMP
			light.light_energy = 1.2
			light.omni_range = reach * METRES
			light.position = _p(x, y, ceiling - 24)
			_add(_lit, light)
			var bar := MeshInstance3D.new()
			bar.name = "Bar"
			var mesh := BoxMesh.new()
			mesh.size = Vector3(64, 4, 12) * METRES
			mesh.material = _lamp
			bar.mesh = mesh
			bar.position = Vector3(0, 22, 0) * METRES
			_add(light, bar)

## The sky over an outdoor area, as low as the reference's (not drawn).
func _sky_over(name: String, r: Array, z: float) -> void:
	var body := StaticBody3D.new()
	body.name = name + "_sky"
	body.set_meta("contents", "sky")
	_add(_skies, body)
	var shape := CollisionShape3D.new()
	shape.name = "Box"
	var box := BoxShape3D.new()
	box.size = Vector3(r[2] - r[0], TOP - z, r[3] - r[1]) * METRES
	shape.shape = box
	shape.position = _p((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0, (z + TOP) / 2.0)
	_add(body, shape)

## Panels on every stretch of wall around the dressed areas, facing in, a unit off the wall,
## from the floor up; and a cable tray under each high roof.
func _dress(parent: Node3D) -> void:
	for kind in DRESS:
		for name in DRESS[kind]:
			var a: Array = _area_named(name)
			var r: Array = a[1]
			var h: float = DRESS_HEIGHT[kind]
			# [along x?, the fixed coordinate, the cell centre just outside, which way is in]
			for edge in [[true, r[1], r[1] - CELL / 2.0, 1.0], [true, r[3], r[3] + CELL / 2.0, -1.0], [false, r[0], r[0] - CELL / 2.0, 1.0], [false, r[2], r[2] + CELL / 2.0, -1.0]]:
				var from: float = r[0] if edge[0] else r[1]
				var to: float = r[2] if edge[0] else r[3]
				var start := NAN
				var t := from
				while t <= to:
					var wall := t < to and not (_covered(t + CELL / 2.0, edge[2]) if edge[0] else _covered(edge[2], t + CELL / 2.0))
					if wall and is_nan(start):
						start = t
					elif not wall and not is_nan(start):
						_run(parent, kind, edge, start, t, a, h)
						start = NAN
					t += CELL
	for name in CEILINGS:
		var c: Array = CEILINGS[name]
		var a: Array = _area_named(name)
		if c[0] != "roof" or a[2] is Array or c[1] - a[2] < 200:
			continue
		_tray(parent, name, a[1], c[1] - 56)

## A run of panels along one edge of an area. Along a ramp's slope it steps up in 48-unit
## pieces, each standing on the floor where it's lowest.
func _run(parent: Node3D, kind: String, edge: Array, start: float, end: float, a: Array, h: float) -> void:
	var at := func(t: float) -> float:
		return _floor_z(a, t, edge[1]) if edge[0] else _floor_z(a, edge[1], t)
	var sloped: bool = a[2] is Array and ((a[3].axis == "x") == edge[0])
	if not sloped:
		_panel(parent, kind, edge, start, end, at.call(start), h)
		return
	var t := start
	while t < end:
		var e := minf(t + 48.0, end)
		_panel(parent, kind, edge, t, e, minf(at.call(t), at.call(e)), h)
		t = e

## An area's floor height at a point (ramps: along their slope).
func _floor_z(a: Array, x: float, y: float) -> float:
	if not (a[2] is Array):
		return a[2]
	var r: Array = a[1]
	var k: float = (x - r[0]) / (r[2] - r[0]) if a[3].axis == "x" else (y - r[1]) / (r[3] - r[1])
	return lerpf(a[2][0], a[2][1], clampf(k, 0.0, 1.0))

func _area_named(name: String) -> Array:
	for a in AREAS:
		if a[0] == name:
			return a
	return []

## One run of panels: wall from `start` to `end` along an edge, `h` tall from the floor `z`.
func _panel(parent: Node3D, kind: String, edge: Array, start: float, end: float, z: float, h: float) -> void:
	var mesh := QuadMesh.new()
	mesh.size = Vector2(end - start, h) * METRES
	mesh.material = _dress_mats[kind]
	var m := MeshInstance3D.new()
	m.name = "%s%d" % [kind, parent.get_child_count()]
	m.mesh = mesh
	m.set_instance_shader_parameter("size", mesh.size)
	var mid := (start + end) / 2.0
	var inward: float = edge[3]
	# a quad faces +z; turn it to face into the area
	if edge[0]:
		m.position = _p(mid, edge[1] + inward, z + h / 2.0)
		m.rotation.y = 0.0 if inward < 0 else PI
	else:
		m.position = _p(edge[1] + inward, mid, z + h / 2.0)
		m.rotation.y = PI / 2.0 if inward > 0 else -PI / 2.0
	_add(parent, m)

## A cable tray along an area's long side, a third of the way across, hung from the roof.
func _tray(parent: Node3D, name: String, r: Array, z: float) -> void:
	var along_x: bool = r[2] - r[0] >= r[3] - r[1]
	var length: float = (r[2] - r[0] if along_x else r[3] - r[1]) - 32.0
	var at: float = (r[1] + (r[3] - r[1]) / 3.0) if along_x else (r[0] + (r[2] - r[0]) / 3.0)
	var mid: float = (r[0] + r[2]) / 2.0 if along_x else (r[1] + r[3]) / 2.0
	var tray := MeshInstance3D.new()
	tray.name = name + "_tray"
	var box := BoxMesh.new()
	box.size = Vector3(length if along_x else 14.0, 3.0, 14.0 if along_x else length) * METRES
	box.material = _tray_mat()
	tray.mesh = box
	tray.position = _p(mid, at, z) if along_x else _p(at, mid, z)
	_add(parent, tray)

var _tray_material: StandardMaterial3D
func _tray_mat() -> StandardMaterial3D:
	if _tray_material == null:
		_tray_material = StandardMaterial3D.new()
		_tray_material.albedo_color = Color(0.42, 0.44, 0.46)
		_tray_material.metallic = 0.6
		_tray_material.roughness = 0.45
	return _tray_material

## Cooling towers outside the walls west of the dock, seen over them, steam rising off the top.
func _landmark(parent: Node3D) -> void:
	var concrete := StandardMaterial3D.new()
	concrete.albedo_color = Color(0.62, 0.62, 0.60)
	concrete.roughness = 0.95
	for i in TOWERS.size():
		var t: Array = TOWERS[i]
		var height: float = t[2]
		var waist := height * 0.7
		for part in [[0.0, waist, 330.0, 220.0], [waist, height, 220.0, 250.0]]:
			var c := CylinderMesh.new()
			c.bottom_radius = part[2] * METRES
			c.top_radius = part[3] * METRES
			c.height = (part[1] - part[0]) * METRES
			c.radial_segments = 32
			c.rings = 1
			c.cap_top = false
			c.material = concrete
			var m := MeshInstance3D.new()
			m.name = "Tower%d_%d" % [i, int(part[0])]
			m.mesh = c
			m.position = _p(t[0], t[1], (part[0] + part[1]) / 2.0)
			_add(parent, m)
		_add(parent, _steam("Steam%d" % i, _p(t[0], t[1], height + 20)))

func _steam(name: String, at: Vector3) -> GPUParticles3D:
	var p := GPUParticles3D.new()
	p.name = name
	p.position = at
	p.amount = 48
	p.lifetime = 10.0
	p.preprocess = 10.0
	p.visibility_aabb = AABB(Vector3(-30, -5, -30), Vector3(60, 70, 60))
	var pm := ParticleProcessMaterial.new()
	pm.direction = Vector3.UP
	pm.spread = 10.0
	pm.initial_velocity_min = 2.5
	pm.initial_velocity_max = 4.0
	pm.gravity = Vector3(0.5, 0.3, 0.2)
	pm.emission_shape = ParticleProcessMaterial.EMISSION_SHAPE_SPHERE
	pm.emission_sphere_radius = 4.0
	pm.scale_min = 0.8
	pm.scale_max = 1.3
	var grow := Curve.new()
	grow.add_point(Vector2(0, 0.4))
	grow.add_point(Vector2(1, 1.6))
	var grow_tex := CurveTexture.new()
	grow_tex.curve = grow
	pm.scale_curve = grow_tex
	var fade := Gradient.new()
	fade.set_color(0, Color(1, 1, 1, 0))
	fade.set_color(1, Color(1, 1, 1, 0))
	fade.add_point(0.15, Color(1, 1, 1, 0.55))
	fade.add_point(0.6, Color(1, 1, 1, 0.3))
	var fade_tex := GradientTexture1D.new()
	fade_tex.gradient = fade
	pm.color_ramp = fade_tex
	p.process_material = pm
	var puff := Gradient.new()
	puff.set_color(0, Color(1, 1, 1, 1))
	puff.set_color(1, Color(1, 1, 1, 0))
	var puff_tex := GradientTexture2D.new()
	puff_tex.gradient = puff
	puff_tex.fill = GradientTexture2D.FILL_RADIAL
	puff_tex.fill_from = Vector2(0.5, 0.5)
	puff_tex.fill_to = Vector2(1.0, 0.5)
	var mat := StandardMaterial3D.new()
	mat.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	mat.billboard_mode = BaseMaterial3D.BILLBOARD_PARTICLES
	mat.vertex_color_use_as_albedo = true
	mat.albedo_color = Color(0.93, 0.95, 0.97)
	mat.albedo_texture = puff_tex
	var quad := QuadMesh.new()
	quad.size = Vector2(12, 12)
	quad.material = mat
	p.draw_pass_1 = quad
	return p

## The sky over everything: stops grenades and bullets leaving the map (not drawn).
func _sky() -> void:
	var body := StaticBody3D.new()
	body.name = "Sky"
	body.set_meta("contents", "sky")
	_add(_root, body)
	var shape := CollisionShape3D.new()
	shape.name = "Box"
	var box := BoxShape3D.new()
	box.size = Vector3(6400, 32, 6400) * METRES
	shape.shape = box
	shape.position = _p(0, 1000, TOP + 16)
	_add(body, shape)

func _spawns() -> void:
	var g := _group("Spawns")
	var i := 0
	for s in SPAWNS:
		i += 1
		var m := Marker3D.new()
		m.name = "%s%d" % [s[0].to_upper(), i]
		m.position = _p(s[1], s[2], s[3] + 40)
		m.rotation.y = deg_to_rad(s[4] - 90.0)
		m.add_to_group("spawn_" + s[0], true)
		_add(g, m)

func _zones() -> void:
	var g := _group("Zones")
	for z in ZONES:
		var b: Array = z[1]
		var area := Area3D.new()
		area.name = z[0].capitalize().replace(" ", "")
		area.add_to_group(z[0], true)
		area.position = _p((b[0] + b[3]) / 2, (b[1] + b[4]) / 2, (b[2] + b[5]) / 2)
		_add(g, area)
		var shape := CollisionShape3D.new()
		shape.name = "Box"
		var box := BoxShape3D.new()
		box.size = Vector3(b[3] - b[0], b[5] - b[2], b[4] - b[1]) * METRES
		shape.shape = box
		_add(area, shape)

## Each site: a big letter on its back wall, one painted on its floor, a light in its colour.
func _marks() -> void:
	var g := _group("Sites")
	var font: Font = load("res://fonts/anton/Anton-Regular.ttf")
	for s in SITES:
		var colour: Color = s[1]
		var light := OmniLight3D.new()
		light.name = "Light" + s[0]
		light.light_color = colour
		light.light_energy = 3.0
		light.omni_range = 14.0
		light.position = _p(s[2][0], s[2][1], s[2][2])
		_add(g, light)
		var wall := _letter(s[0], font, colour)
		wall.name = "Letter" + s[0]
		wall.position = _p(s[4][0], s[4][1], s[4][2])
		# (a label reads from its +z side)
		wall.rotation.y = deg_to_rad(s[5] + 90.0)
		_add(g, wall)
		var painted := _letter(s[0], font, colour)
		painted.name = "Floor" + s[0]
		painted.position = _p(s[3][0], s[3][1], _floor_at(s[3][0], s[3][1]) + 1)
		painted.rotation.x = -PI / 2
		_add(g, painted)

func _letter(text: String, font: Font, colour: Color) -> Label3D:
	var l := Label3D.new()
	l.text = text
	l.font = font
	l.font_size = 256
	l.pixel_size = 0.012
	l.modulate = colour
	l.shaded = false
	l.double_sided = false
	return l

## The floor height of the area under a point (the highest one there).
func _floor_at(x: float, y: float) -> float:
	var best := -INF
	for a in AREAS:
		var r: Array = a[1]
		if x >= r[0] and x <= r[2] and y >= r[1] and y <= r[3]:
			best = max(best, a[2] if not (a[2] is Array) else max(a[2][0], a[2][1]))
	return best
