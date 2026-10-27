## Headless check that the GDExtension loads and a practice session works: it connects,
## spawns on the test arena, walks forward and stays on the floor. Run:
##   godot --headless --path godot --script res://tests/session_check.gd
extends SceneTree

func _initialize() -> void:
	var s := Session.new()
	if not s.start_practice(FileAccess.get_file_as_string("res://maps/test_arena.bmap"), 2):
		_fail("practice didn't start")
		return
	var dt := 1.0 / s.tick_rate()
	var t := 0
	while not (s.is_playing() and s.has_snapshot()) and t < 2000:
		s.tick(0, 0, 0, 0, 0, 0)
		OS.delay_usec(int(dt * 1e6))
		t += 1
	if not s.is_playing():
		_fail("never got into the game")
		return
	var start := s.eye(1.0, dt)
	# run forward (Godot yaw 0 looks down -z) for one second
	for i in s.tick_rate():
		s.tick(1, 0, 0, 0, 0, 0)
		OS.delay_usec(int(dt * 1e6))
	var end := s.eye(1.0, dt)
	var moved := Vector2(end.x - start.x, end.z - start.z).length()
	print("eye %s -> %s, moved %.2f m, speed %d u/s, others %d" % [start, end, moved, s.speed(), s.others_feet(1.0).size()])
	s.stop()
	# 250 units a second for about a second is 6.35 m (less the start-up)
	if moved < 4.0 or end.y < 1.0 or end.y > 2.5:
		_fail("didn't walk as expected")
		return
	print("session_check: ok")
	quit(0)

func _fail(why: String) -> void:
	push_error("session_check: " + why)
	quit(1)
