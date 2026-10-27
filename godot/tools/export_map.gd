## Writes a map scene's `.bmap` (what the rules need: brushes, spawns, zones) next to it:
##   godot --headless --path godot --script res://tools/export_map.gd -- res://maps/<name>.tscn
##
## How a map scene is read:
## - Brushes: every CSGBox3D, and every CollisionShape3D with a BoxShape3D under a
##   StaticBody3D, rotated and scaled as placed. Metadata on the node (or the body):
##   `contents` = "solid" (default), "clear" (glass, fences), "sky", "water";
##   `material` = CS's letter: "C" concrete (default), "M" metal, "W" wood, "G" grate,
##   "V" vent, "T" tile, "P" computer, "Y" glass, "D" dirt.
## - Breakables: a brush with `health` metadata breaks (its piece number is given here). Also
##   `breaks` = "glass" (default), "wood", "metal", "concrete", "computer", "rocks",
##   "ceilingtile", "flesh" (its sound and debris); `trigger_only` = true: only the bomb at a
##   site naming its `targetname` breaks it.
## - Spawns: Marker3D nodes in group "spawn_t" (Strays) or "spawn_ct" (Bureau); they face
##   the way the marker faces (its -z).
## - Zones: Area3D nodes in group "buy_t", "buy_ct", "buy" (both sides), "site_a", "site_b"
##   or "ladder", each with a CollisionShape3D box (its bounding box is used). A site's
##   `target` metadata names the pieces the bomb breaks when it goes off there.
## - The root may carry `bomb_radius` metadata (default 500).
extends SceneTree

## Metres per GoldSrc unit (the simulation's units: z up, 1 unit = 1 inch).
const METRES := 0.0254

var _scene: Node
var _path := ""
var _done := false
var _next_model := 1

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	if args.size() != 1:
		_fail("usage: -- res://maps/<name>.tscn")
		return
	var path: String = args[0]
	var packed := load(path) as PackedScene
	if packed == null:
		_fail("can't load " + path)
		return
	_scene = packed.instantiate()
	_path = path
	root.add_child(_scene)

## (one frame in, the scene is in the tree and placed)
func _process(_delta: float) -> bool:
	if _done:
		return true
	_done = true
	var scene := _scene
	var path := _path
	var map := {
		"name": path.get_file().get_basename(),
		"brushes": [], "spawns": [], "buy_zones": [], "bomb_sites": [], "ladders": [], "breakables": [],
		"bomb_radius": scene.get_meta("bomb_radius", 500.0),
	}
	_walk(scene, map)
	var out := path.get_basename() + ".bmap"
	var f := FileAccess.open(out, FileAccess.WRITE)
	f.store_string(JSON.stringify(map, "\t"))
	f.close()
	print("%s: %d brushes, %d spawns, %d buy zones, %d sites, %d ladders, %d breakables" % [out, map.brushes.size(), map.spawns.size(), map.buy_zones.size(), map.bomb_sites.size(), map.ladders.size(), map.breakables.size()])
	quit(0)
	return true

func _walk(node: Node, map: Dictionary) -> void:
	if node is CSGBox3D:
		map.brushes.append(_brush(node.global_transform, node.size, node, map))
	elif node is CollisionShape3D and node.shape is BoxShape3D and node.get_parent() is StaticBody3D:
		map.brushes.append(_brush(node.global_transform, node.shape.size, node, map))
	elif node is Marker3D:
		for side in ["t", "ct"]:
			if node.is_in_group("spawn_" + side):
				# the marker's forward (-z) as a GoldSrc yaw
				var f: Vector3 = -node.global_transform.basis.z
				map.spawns.append({"side": side, "origin": _arr(_point(node.global_position)), "yaw": rad_to_deg(atan2(-f.z, f.x))})
	elif node is Area3D:
		var box := _area_box(node)
		if not box.is_empty():
			if node.is_in_group("buy"):
				map.buy_zones.append(box)
			for side in ["t", "ct"]:
				if node.is_in_group("buy_" + side):
					map.buy_zones.append(box.merged({"side": side}))
			for letter in ["a", "b"]:
				if node.is_in_group("site_" + letter):
					map.bomb_sites.append(box.merged({"letter": letter, "target": node.get_meta("target", "")}))
			if node.is_in_group("ladder"):
				map.ladders.append(box)
	for child in node.get_children():
		_walk(child, map)

## A box placed by `t` as a brush: its six faces as planes, its corners' bounds.
func _brush(t: Transform3D, size: Vector3, node: Node, map: Dictionary) -> Dictionary:
	var corners := _corners(t, size)
	var planes := []
	var normals := t.basis.inverse().transposed()
	for axis in [Vector3.RIGHT, Vector3.UP, Vector3.BACK]:
		for s in [1.0, -1.0]:
			var n := _direction((normals * (axis * s)).normalized())
			var d := -INF
			for c in corners:
				d = max(d, n.dot(c))
			planes.append([[n.x, n.y, n.z], d])
	var info: Node = node if node.has_meta("contents") or node.has_meta("material") or not (node.get_parent() is StaticBody3D) else node.get_parent()
	var b := _bounds(corners)
	b["planes"] = planes
	b["contents"] = info.get_meta("contents", "solid")
	b["material"] = info.get_meta("material", "C")
	b["model"] = 0
	if info.has_meta("health"):
		b["model"] = _next_model
		map.breakables.append({
			"model": _next_model,
			"health": float(info.get_meta("health")),
			"material": info.get_meta("breaks", "glass"),
			"trigger_only": bool(info.get_meta("trigger_only", false)),
			"targetname": info.get_meta("targetname", ""),
			"node": str(_scene.get_path_to(info)),
		})
		_next_model += 1
	return b

## An area's box (the bounding box of its first box shape), or {} if it has none.
func _area_box(area: Area3D) -> Dictionary:
	for c in area.get_children():
		if c is CollisionShape3D and c.shape is BoxShape3D:
			return _bounds(_corners(c.global_transform, c.shape.size))
	return {}

func _corners(t: Transform3D, size: Vector3) -> Array:
	var out := []
	for x in [-0.5, 0.5]:
		for y in [-0.5, 0.5]:
			for z in [-0.5, 0.5]:
				out.append(_point(t * (size * Vector3(x, y, z))))
	return out

func _bounds(corners: Array) -> Dictionary:
	var lo := Vector3(INF, INF, INF)
	var hi := -lo
	for c in corners:
		lo = lo.min(c)
		hi = hi.max(c)
	return {"lo": [lo.x, lo.y, lo.z], "hi": [hi.x, hi.y, hi.z]}

## A Godot point (metres, y up) in GoldSrc units (z up).
func _point(v: Vector3) -> Vector3:
	return Vector3(v.x, -v.z, v.y) / METRES

func _arr(v: Vector3) -> Array:
	return [v.x, v.y, v.z]

## A Godot direction as a GoldSrc one (no scaling).
func _direction(v: Vector3) -> Vector3:
	return Vector3(v.x, -v.z, v.y)

func _fail(why: String) -> void:
	push_error("export_map: " + why)
	quit(1)
