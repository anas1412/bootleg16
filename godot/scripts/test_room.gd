## The P0 test room: one list of solid boxes (metres) that both draws the room and is the
## map the simulation collides with, so what you see is exactly what you bump into.
class_name TestRoom
extends RefCounted

## A player is 1.83 m tall (72 units); a step you walk up is at most 0.46 m (18 units).
const BOXES: Array[AABB] = [
	# floor and the four walls of a 40 x 40 m hall
	AABB(Vector3(-20, -1, -20), Vector3(40, 1, 40)),
	AABB(Vector3(-20, 0, -21), Vector3(40, 5, 1)),
	AABB(Vector3(-20, 0, 20), Vector3(40, 5, 1)),
	AABB(Vector3(-21, 0, -20), Vector3(1, 5, 40)),
	AABB(Vector3(20, 0, -20), Vector3(1, 5, 40)),
	# stairs: 5 steps of 0.4 m (16 units, walked up) to a platform
	AABB(Vector3(4, 0, -6), Vector3(3, 0.4, 0.6)),
	AABB(Vector3(4, 0, -6.6), Vector3(3, 0.8, 0.6)),
	AABB(Vector3(4, 0, -7.2), Vector3(3, 1.2, 0.6)),
	AABB(Vector3(4, 0, -7.8), Vector3(3, 1.6, 0.6)),
	AABB(Vector3(4, 0, -8.4), Vector3(3, 2.0, 0.6)),
	AABB(Vector3(4, 0, -12.4), Vector3(6, 2.0, 4)),
	# a block you jump onto (1 m) and one you need a crouch-jump for (1.3 m)
	AABB(Vector3(-4, 0, -6), Vector3(1.5, 1.0, 1.5)),
	AABB(Vector3(-7, 0, -6), Vector3(1.5, 1.3, 1.5)),
	# a low ceiling (1.2 m up): standing you can't get under it, crouched you can
	AABB(Vector3(-14, 1.2, -8), Vector3(4, 0.4, 6)),
	AABB(Vector3(-14.5, 0, -8), Vector3(0.5, 1.6, 6)),
	AABB(Vector3(-10, 0, -8), Vector3(0.5, 1.6, 6)),
	# a long clear run along the south wall for bunny hopping (nothing in the way)
]

## Where you start, and where the practice dummies stand.
const PLAYER_SPAWNS: Array[Vector3] = [Vector3(0, 1, 16)]
const DUMMY_SPAWNS: Array[Vector3] = [Vector3(-6, 1, 6), Vector3(0, 1, 4), Vector3(6, 1, 6)]

## The boxes as drawn: grey blocks with a 1 m grid (to read speed and size by).
static func build(parent: Node3D) -> void:
	var grid := ShaderMaterial.new()
	grid.shader = load("res://shaders/grid.gdshader")
	for b in BOXES:
		var box := CSGBox3D.new()
		box.size = b.size
		box.position = b.get_center()
		box.material = grid
		parent.add_child(box)
