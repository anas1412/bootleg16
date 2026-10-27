## Builds the data_center grey box (godot/maps/data_center.tscn) from the layout below:
##   godot --headless --path godot --script res://tools/build_data_center.gd
## then export its .bmap with tools/export_map.gd. The layout is the structure of the classic
## two-site map (docs/MAPS.md): its lanes, sites, chokes and distances, measured as numbers
## with `net/examples/mapstats.rs`, laid out again from our own blocks.
##
## Everything here is in GoldSrc units (x east, y north, z up, 1 unit = 1 inch), as the
## measurements are. Areas are where players walk: a floor (solid down to the base), a ramp
## (its `z` is [height at the low corner, height at the high corner] along `axis`) or a bridge
## (a thin slab over another area). Every 32-unit cell no area covers becomes wall. `roof`: a
## ceiling that high over the area's floor (the crawlspace). Blocks are cover and door frames.
extends SceneTree

const METRES := 0.0254
const CELL := 32
const BASE := -320
const TOP := 448
const OUT := "res://maps/data_center.tscn"

## [name, [x0, y0, x1, y1], z or [z0, z1], extra]
## extra: {"axis": "x" | "y"} for ramps, {"bridge": true}, {"roof": height}.
const AREAS := [
	# Strays side: the loading dock (spawn) and its ramps
	["dock", [-2304, -1024, -1024, -640], 128, {}],
	["dock_north", [-1792, -640, -1024, -256], 128, {}],
	["dock_east", [-1024, -1024, -512, -640], 128, {}],
	["dock_ramp", [-512, -1024, 0, -576], [128, 0], {"axis": "x"}],
	["dock_lane", [0, -768, 256, -576], 0, {}],
	# to B: the intake yard, then the cable crawlspace under the floor
	["intake_ramp", [-2048, -640, -1792, -128], [128, 0], {"axis": "y"}],
	["intake_steps", [-1632, -256, -1536, 32], [128, 0], {"axis": "y"}],
	["intake", [-2048, -128, -1536, 512], 0, {}],
	["crawl_mouth", [-2048, 512, -1536, 704], 0, {"roof": 144}],
	["crawl_neck", [-1792, 704, -1664, 1024], 0, {"roof": 144}],
	["crawl_cross", [-2048, 1024, -1408, 1280], 0, {"roof": 144}],
	["crawl_east", [-1408, 1088, -1184, 1248], 0, {"roof": 144}],
	["crawl_drop", [-1184, 1088, -1056, 1280], [0, -112], {"axis": "y", "roof": 144}],
	["crawl_low", [-1184, 1280, -512, 1536], -112, {"roof": 144}],
	["crawl_up", [-2048, 1280, -1920, 1792], 0, {"roof": 144}],
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

## Cover, racks and door frames: [name, [x0, y0, x1, y1], [z0, z1]]
const BLOCKS := [
	# the spine's security doors: a frame 192 wide (its two panels are LEAVES), a header
	["spine_door_w", [-512, 1592, -480, 1672], [-128, TOP]],
	["spine_door_e", [-288, 1592, -256, 1672], [-128, TOP]],
	["spine_door_head", [-480, 1592, -288, 1672], [0, TOP]],
	# the archive doors (B): a frame 176 wide (its panels are LEAVES), a header
	["b_door_s", [-1344, 2080, -1312, 2096], [0, TOP]],
	["b_door_head", [-1344, 2096, -1312, 2272], [128, TOP]],
	# a low header where the control room opens onto the ramp to A (under the walkway deck)
	["control_head", [528, 2176, 560, 2304], [0, 96]],
	# the aisle doors: a header over the 320-unit passage
	["aisle_door_head", [912, 768, 944, 1088], [144, TOP]],
	# cover
	["dock_rack", [-1152, -640, -1024, -448], [128, 256]],
	["dock_crate", [-1792, -256, -1664, -128], [128, 192]],
	["spine_rail", [-272, 768, -240, 1568], [-128, 32]],
	["walkway_crate", [-224, 1440, -160, 1504], [0, 64]],
	["walkway_end", [400, 1408, 496, 1520], [16, 96]],
	["yard_rack", [768, 1088, 928, 1184], [0, 128]],
	["aisle_crate", [1536, 1984, 1600, 2112], [0, 64]],
	["mouth_crate", [960, 288, 1056, 384], [0, 64]],
	["a_crate", [1024, 2432, 1152, 2560], [96, 160]],
	["a_rail", [1280, 2304, 1344, 2752], [96, 128]],
	["b_rack", [-1920, 2304, -1792, 2432], [0, 96]],
	["b_crate", [-1664, 2560, -1536, 2688], [0, 64]],
	["b_crate_low", [-1536, 2560, -1472, 2624], [0, 32]],
	["link_crate", [-160, 2208, -32, 2304], [-128, -64]],
	["crawl_pillar", [-896, 1280, -800, 1376], [-112, 32]],
]

## Door panels standing at an angle: [name, [x, y] one end, [x, y] the other, thickness, [z0, z1]].
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
	["A", Color(1.0, 0.76, 0.24), [1152, 2464, 320], [1216, 2368], [1344, 3070, 320], 270],
	["B", Color(0.24, 1.0, 0.63), [-1536, 2688, 224], [-1504, 2784], [-2110, 2688, 192], 0],
]

var _mats := {}
var _root: Node3D

func _initialize() -> void:
	_root = Node3D.new()
	_root.name = "DataCenter"
	_root.set_meta("bomb_radius", 500.0)
	_mats = {
		"floor": _grid(Color(0.40, 0.42, 0.45)),
		"wall": _grid(Color(0.47, 0.48, 0.50)),
		"cover": _grid(Color(0.40, 0.43, 0.48)),
		"roof": _grid(Color(0.26, 0.27, 0.29)),
	}
	var floors := _group("Floors")
	var walls := _group("Walls")
	var blocks := _group("Blocks")
	for a in AREAS:
		_area(floors, a)
	_walls(walls)
	for b in BLOCKS:
		_box(blocks, b[0], b[1][0], b[1][1], b[2][0], b[1][2], b[1][3], b[2][1], "cover", {"material": "M"})
	for l in LEAVES:
		_leaf(blocks, l)
	_sky()
	_spawns()
	_zones()
	_marks()
	var packed := PackedScene.new()
	packed.pack(_root)
	var err := ResourceSaver.save(packed, OUT)
	print("%s: %d areas, %d wall boxes, %d blocks (%s)" % [OUT, AREAS.size(), walls.get_child_count(), BLOCKS.size(), error_string(err)])
	_root.free()
	quit(0 if err == OK else 1)

func _grid(base: Color) -> ShaderMaterial:
	var m := ShaderMaterial.new()
	m.shader = load("res://shaders/grid.gdshader")
	m.set_shader_parameter("base", base)
	m.set_shader_parameter("line", base.darkened(0.3))
	return m

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
func _leaf(parent: Node, l: Array) -> void:
	var a := Vector2(l[1][0], l[1][1])
	var b := Vector2(l[2][0], l[2][1])
	var z: Array = l[4]
	var box := _box(parent, l[0], 0, 0, z[0], a.distance_to(b), l[3], z[1], "cover", {"material": "M"})
	var mid := (a + b) / 2.0
	box.position = _p(mid.x, mid.y, (z[0] + z[1]) / 2.0)
	# GoldSrc's angle from x towards y is a turn around Godot's up the same way
	box.rotation.y = (b - a).angle()

func _area(parent: Node, a: Array) -> void:
	var name: String = a[0]
	var r: Array = a[1]
	var extra: Dictionary = a[3]
	if a[2] is Array:
		_ramp(parent, name, r, a[2], extra)
	elif extra.get("bridge", false):
		_box(parent, name, r[0], r[1], a[2] - 24, r[2], r[3], a[2], "floor")
	else:
		_box(parent, name, r[0], r[1], BASE, r[2], r[3], a[2], "floor")
	if extra.has("roof"):
		var top: float = a[2] if not (a[2] is Array) else max(a[2][0], a[2][1])
		_box(parent, name + "_roof", r[0], r[1], top + extra.roof, r[2], r[3], TOP, "roof")

## A ramp: solid up to its low end, and a slab whose top runs from z0 at the low-coordinate
## edge to z1 at the other.
func _ramp(parent: Node, name: String, r: Array, z: Array, extra: Dictionary) -> void:
	var lo_z: float = min(z[0], z[1])
	_box(parent, name, r[0], r[1], BASE, r[2], r[3], lo_z, "floor")
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
	b.material = _mats.floor
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
