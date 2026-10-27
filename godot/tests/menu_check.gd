## Headless check of the menu and settings: presets set every option, changing an option makes
## the preset Custom (or the preset it now matches), settings survive a reload, the menu builds
## both pages, and Deploy (Practice) starts the game and Leave comes back. Uses its own settings
## file. Run:
##   BOOTLEG16_SETTINGS=/tmp/menu_check.cfg godot --headless --path godot --script res://tests/menu_check.gd
extends SceneTree

var failed := false

func _initialize() -> void:
	await process_frame
	var s: Node = root.get_node("Settings")
	if not s.path.begins_with("/"):
		_fail("BOOTLEG16_SETTINGS isn't set: refusing to touch your real settings")
		quit(1)
		return
	DirAccess.remove_absolute(s.path)

	for i in 4:
		s.set_preset(i)
		for key in s.GRAPHICS:
			_check(s.graphics[key] == s.GRAPHICS[key].preset[i], "preset %d sets %s" % [i, key])
	s.set_preset(2)
	s.set_graphics("ssr", 1)  # High with reflections on is no preset
	_check(s.preset == s.CUSTOM, "an option change makes it Custom")
	s.set_graphics("ssr", 0)
	_check(s.preset == 2, "changing it back finds High again")
	s.set_value("sensitivity", 3.25)
	s.set_value("fps_cap", 3)

	var again: Node = load("res://scripts/settings.gd").new()
	again._load()
	_check(again.preset == 2 and is_equal_approx(again.sensitivity, 3.25) and again.fps_cap == 3, "settings survive a reload")
	again.free()

	change_scene_to_file("res://scenes/menu.tscn")
	for f in 10:
		await process_frame
	var menu: Node = current_scene
	var ui: Control = menu.get_node("UI/Menu")
	ui._tabs.pick(1)
	for f in 30:
		await process_frame
	_check(ui._pages[1].visible, "Settings opens")
	_check(ui._pages[1]._rows.get_child_count() == s.GRAPHICS.size() + 4, "graphics rows: preset, options, display, vsync, frame cap")
	var ys: Array = ui._pages[1]._rows.get_children().map(func(r): return r.position.y)
	var stacked := true
	for i in range(1, ys.size()):
		stacked = stacked and is_equal_approx(ys[i] - ys[i - 1], 48 + Tokens.SPACE_XS)
	_check(stacked, "settings rows sit one under the other, 52 apart (were piled up): %s" % [ys])
	ui._tabs.pick(0)
	for f in 30:
		await process_frame
	_check(not ui._pages[1].visible and ui._pages[0].visible, "back to Play")

	# Deploy in Practice starts the game; Leave in the pause overlay comes back to the menu
	ui._pages[0].deploy.emit(2)
	for f in 30:
		await process_frame
	_check(current_scene.name == "Main", "Deploy starts practice")
	var pause: Node = current_scene.find_children("*", "CanvasLayer", false, false).filter(func(n): return n.has_signal("leave") and n.has_method("open")).front()
	pause.open()
	_check(pause.is_open() and Input.mouse_mode == Input.MOUSE_MODE_VISIBLE, "Esc pauses and frees the mouse")
	pause.leave.emit()
	for f in 30:
		await process_frame
	_check(current_scene.name == "MainMenu", "Leave goes back to the menu")
	DirAccess.remove_absolute(s.path)
	print("menu_check: ", "FAILED" if failed else "ok")
	quit(1 if failed else 0)

func _check(ok: bool, what: String) -> void:
	if not ok:
		_fail(what)

func _fail(what: String) -> void:
	failed = true
	printerr("menu_check failed: ", what)
