## Headless check of a practice match on data_center: Deploy puts you on the Strays with nine
## bots (5 v 5, the top bar marks them bots) and the HUD up; the buy menu sells (money drops,
## owned and unaffordable cards say so) and undo sells it back; you switch weapons, shoot, reload and throw an HE (the gun in your hands follows);
## a kill reaches the kill feed and the round ends. Your inputs are driven from here (headless
## has no mouse to capture). Takes about two minutes of real time. Run:
##   BOOTLEG16_SETTINGS=/tmp/match_check.cfg godot --headless --path godot --script res://tests/match_check.gd
extends SceneTree

const ATTACK := 16
const RELOAD := 64

var failed := false
var main: Node
var session: Session
var events := []
var buttons := 0
var slot := 0

func _initialize() -> void:
	await process_frame
	change_scene_to_file("res://main.tscn")
	await process_frame
	await process_frame
	main = current_scene
	session = main.session
	main.game_event.connect(func(e): events.append(e))
	# drive your inputs from here
	var player: Camera3D = main.get_node("Player")
	player.set_physics_process(false)
	physics_frame.connect(_drive)

	if not await _until(func(): return session.hud().get("team", "") == "t", 15.0):
		_fail("never joined the Strays: %s" % session.hud())
		_end()
		return
	await _until(func(): return _count("t") == 5 and _count("ct") == 5, 10.0)
	_check(_count("t") == 5 and _count("ct") == 5, "nine bots, 4 with you and 5 against: %d t, %d ct" % [_count("t"), _count("ct")])
	var hud := session.hud()
	_check(hud.phase in ["freeze", "warmup"] and hud.money == 800, "a round starts with 800: %s %d" % [hud.phase, hud.money])
	_check(main.get_node("Hud")._root.visible, "the HUD is up")
	# (the scoreboard comes a moment after the players themselves)
	await _until(func(): return session.players().size() == 10, 10.0)
	var players: Array = session.players()
	_check(players.size() == 10 and players.filter(func(p): return p.bot).size() == 9, "the top bar has 10 players, 9 of them bots: %s" % [players.map(func(p): return [p.name, p.bot])])

	# buy: an HE and a flashbang leave 300, not enough for kevlar
	await _until(func(): return session.hud().can_buy, 5.0)
	var buy: CanvasLayer = main._buy
	buy.open()
	await _frames(10)
	_check(buy._cards.size() == session.shop().size() and buy._cards.has("ak47") and not buy._cards.has("m4a1"), "the Strays' shop: %s" % [buy._cards.keys()])
	buy._cards["hegrenade"].button.pressed.emit()
	buy._cards["flashbang"].button.pressed.emit()
	await _until(func(): return session.hud().money == 300, 3.0)
	await _frames(20)
	_check(session.hud().money == 300, "money after buying: %d" % session.hud().money)
	_check(buy._cards["hegrenade"].price.text == "HAVE IT" and buy._cards["hegrenade"].button.disabled, "a full HE slot shows Have it")
	_check(buy._cards["kevlar"].button.disabled and buy._cards["kevlar"].price.text == "$650", "kevlar is out of reach")
	# undo sells both back; then the HE again, for the throw below
	buy._undo.pressed.emit()
	_check(await _until(func(): return session.hud().money == 800, 3.0), "undo gives the money back: %d" % session.hud().money)
	await _frames(10)
	buy._cards["hegrenade"].button.pressed.emit()
	await _until(func(): return session.hud().money == 500, 3.0)
	buy.close()
	await _frames(20)

	# weapons: the knife comes out, and the gun in your hands follows
	slot = 3
	await _until(func(): return session.hud().weapon == "knife", 3.0)
	await _frames(5)
	_check(main.get_node("Player/Gun")._weapon == "knife", "the viewmodel shows the knife")
	if not await _until(func(): return session.hud().phase == "live", 25.0):
		_fail("the round never went live")
		_end()
		return

	# shoot the pistol, reload it
	slot = 2
	await _until(func(): return session.hud().weapon == "glock18", 3.0)
	await _frames(40)
	var before: int = session.hud().clip
	buttons = ATTACK
	await _until(func(): return session.hud().clip < before, 2.0)
	buttons = 0
	await _frames(10)
	_check(session.hud().clip < before, "shooting spends rounds: %d -> %d" % [before, session.hud().clip])
	_check(_seen("shot"), "your shot comes back as an event")
	buttons = RELOAD
	_check(await _until(func(): return session.hud().reloading, 2.0), "reloading")
	buttons = 0

	# throw the HE: pull the pin, let go
	slot = 4
	await _until(func(): return session.hud().weapon == "hegrenade", 3.0)
	await _frames(100)
	buttons = ATTACK
	await _frames(90)
	buttons = 0
	_check(await _until(func(): return _seen("throw"), 2.0), "the HE was thrown")
	_check(await _until(func(): return _seen("explosion"), 4.0), "the HE went off")

	# the round plays out: someone dies (kill feed), the round ends
	_check(await _until(func(): return _seen("kill"), 120.0), "someone was killed")
	await _frames(5)
	_check(main.get_node("Hud")._feed._list.get_child_count() > 0, "the kill shows in the feed")
	_check(await _until(func(): return _seen("round_end"), 150.0), "the round ended")
	var lying := false
	for fig in main.get_node("Others").get_children():
		lying = lying or (fig.visible and absf(fig.rotation.x) > 1.0)
	_check(lying or session.hud().phase != "over", "the dead lie where they fell")
	_end()

func _drive() -> void:
	if is_instance_valid(main) and main.session:
		var player: Camera3D = main.get_node("Player")
		session.tick(0, 0, player.yaw, player.pitch, buttons, slot)
		slot = 0

func _end() -> void:
	physics_frame.disconnect(_drive)
	main._back()
	await _frames(5)
	print("match_check: %s" % ("FAILED" if failed else "ok"))
	quit(1 if failed else 0)

## Players on a side, from the scoreboard (you're one of the Strays). Not from where players
## are drawn: the server sends only the enemies you can see (anti-wallhack).
func _count(team: String) -> int:
	return session.players().filter(func(o): return o.team == team).size()

func _seen(type: String) -> bool:
	return events.any(func(e): return e.type == type)

func _frames(n: int) -> void:
	for i in n:
		await physics_frame

## Waits until `cond` holds or `secs` pass; whether it held.
func _until(cond: Callable, secs: float) -> bool:
	var end := Time.get_ticks_msec() + secs * 1000.0
	while not cond.call():
		if Time.get_ticks_msec() > end:
			return false
		await physics_frame
	return true

func _check(ok: bool, what: String) -> void:
	if ok:
		print("  ok: " + what)
	else:
		_fail(what)

func _fail(why: String) -> void:
	failed = true
	push_error("match_check: " + why)
