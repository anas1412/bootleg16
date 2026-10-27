## The P0 test room: builds the room, starts a practice session on it and hands the session
## to the parts that use it.
extends Node3D

var session := Session.new()

func _ready() -> void:
	Binds.install()
	TestRoom.build($Room)
	if not session.start_practice(TestRoom.BOXES, PackedVector3Array(TestRoom.PLAYER_SPAWNS), PackedVector3Array(TestRoom.DUMMY_SPAWNS), 3):
		push_error("bootleg16: practice didn't start (see the log)")
		return
	# simulation steps at the server's rate; frames are drawn between them
	Engine.physics_ticks_per_second = session.tick_rate()
	Engine.max_physics_steps_per_frame = 16
	for node in [$Player, $Others, $Hud/Speed]:
		node.session = session

func _exit_tree() -> void:
	session.stop()
