## Practice on a map: shows the map's scene, starts a practice session on its `.bmap` (what
## the rules collide with, exported from the same scene) and hands the session to the parts
## that use it.
extends Node3D

const MAP := "res://maps/test_arena"

var session := Session.new()

func _ready() -> void:
	Binds.install()
	var scene: Node = load(MAP + ".tscn").instantiate()
	$Room.add_child(scene)
	var text := FileAccess.get_file_as_string(MAP + ".bmap")
	var map: Dictionary = JSON.parse_string(text)
	$Pieces.setup(scene, map)
	$Doors.setup(scene, map)
	if not session.start_practice(text, 3):
		push_error("bootleg16: practice didn't start (see the log)")
		return
	# simulation steps at the server's rate (followed in _physics_process: a server says its
	# rate when it welcomes us); frames are drawn between them
	Engine.physics_ticks_per_second = session.tick_rate()
	Engine.max_physics_steps_per_frame = 16
	for node in [$Player, $Others, $Pieces, $Doors, $Hud/Speed]:
		node.session = session
	if OS.get_environment("BOOTLEG16_BENCH") != "":
		add_child(preload("res://scripts/bench.gd").new())

func _physics_process(_delta: float) -> void:
	# prediction must step exactly as often as the server, or it's corrected all the time
	var rate := session.tick_rate()
	if Engine.physics_ticks_per_second != rate:
		Engine.physics_ticks_per_second = rate

func _exit_tree() -> void:
	session.stop()
