## Headless check of the lobby connection against a lobby running on this machine: the game
## signs in and leads a party of one, switches it to custom, a second player joins with the
## party's code, and the squad panel shows them both. Run (a lobby on port 27910 first):
##   target/debug/lobby 27910 &
##   BOOTLEG16_LOBBY=127.0.0.1:27910 BOOTLEG16_SETTINGS=/tmp/lobby_check.cfg \
##     godot --headless --path godot --script res://tests/lobby_check.gd
extends SceneTree

var failed := false

func _initialize() -> void:
	var link: Node = root.get_node("LobbyLink")
	if not await _until(func(): return link.online(), 5.0):
		_fail("never signed in to the lobby")
		return _done()
	var s: Dictionary = link.state
	_check(s.code != "" and link.leading() and s.members.size() == 1, "a party of one, led by you")

	link.lobby.set_mode(1)
	_check(await _until(func(): return link.state.mode == 1, 3.0), "the leader switches to custom")

	var mate := Lobby.new()
	mate.start("Mate", PackedStringArray(["test_arena"]))
	var mate_online := func():
		mate.tick()
		return mate.is_online()
	_check(await _until(mate_online, 5.0), "a second player signs in")
	mate.join_party(s.code)
	var joined := func():
		mate.tick()
		return link.state.members.size() == 2
	_check(await _until(joined, 5.0), "they join with the code")

	change_scene_to_file("res://scenes/menu.tscn")
	await _until(func(): return current_scene != null and current_scene.name == "MainMenu", 2.0)
	for f in 10:
		await process_frame
	var squad: Node = current_scene.get_node("UI/Menu")._pages[0]._squad
	var names: Array = squad.find_children("*", "Label", true, false).map(func(l): return l.text)
	_check(names.has("Mate"), "the squad panel shows them")
	mate.leave_party()
	_done()

## Waits (ticking frames) until `ok` is true or `seconds` pass.
func _until(ok: Callable, seconds: float) -> bool:
	var end := Time.get_ticks_msec() + seconds * 1000.0
	while Time.get_ticks_msec() < end:
		if ok.call():
			return true
		await process_frame
	return false

func _done() -> void:
	print("lobby_check: ", "FAILED" if failed else "ok")
	quit(1 if failed else 0)

func _check(ok: bool, what: String) -> void:
	if not ok:
		_fail(what)

func _fail(what: String) -> void:
	failed = true
	printerr("lobby_check failed: ", what)
