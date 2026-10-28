## The game: a practice match against bots, or the match the lobby found
## (`LobbyLink.pending`). Shows the map's scene, hands the session the map's `.bmap` (what the
## rules collide with, exported from the same scene), gives the session to the parts that use
## it, reads what the HUD shows once a frame (`Session.hud`) and hands it to every part that
## uses it (`on_hud`, or a `hud` property), and passes them the game's events (`on_event`).
## Esc pauses, B buys; leaving, a failed join or the match's end go back to the menu.
extends Node3D

## Something happened in the game (`Session.take_events`): kills, hits, rounds, the bomb.
signal game_event(e: Dictionary)

const PRACTICE_MAP := "data_center"
## A server that hasn't welcomed us in this long isn't coming.
const JOIN_WAIT := 10.0

var session := Session.new()
var _joining := false
var _waited := 0.0
var _left := false
var _buy := preload("res://scripts/hud/buy_menu.gd").new()
var _arms := preload("res://scripts/arms.gd").new()
## The parts that get this frame's `Session.hud` (set in `_play`).
var _readers: Array[Node] = []

func _ready() -> void:
	Binds.install()
	var pause := preload("res://scripts/pause.gd").new()
	pause.leave.connect(_back)
	add_child(pause)
	# after the pause menu, so Esc closes the buy menu first
	add_child(_buy)
	_buy.opened.connect(func(on: bool): $Player.menu_open = on)
	_buy.notice.connect($Hud.notice)
	$Hud.leave.connect(_back)
	var seat: Dictionary = LobbyLink.pending
	LobbyLink.pending = {}
	if seat.is_empty():
		_practice()
	elif session.connect_match(seat.address, seat.ticket, seat.key, LobbyLink.player_name()):
		_joining = true
	else:
		_back("Couldn't reach the match server.")
	if OS.get_environment("BOOTLEG16_BENCH") != "":
		add_child(preload("res://scripts/bench.gd").new())

func _practice() -> void:
	var text := _load_map(PRACTICE_MAP)
	if not session.start_match(text, LobbyLink.player_name()):
		_back("Practice didn't start.")
		return
	_play()

## Waits for the match server to welcome us and name its map, then loads that map.
func _process(delta: float) -> void:
	if _joining:
		_waited += delta
		match session.status():
			"rejected":
				_back("The server refused you: %s" % session.reject_reason())
			"connected" when session.map_name() != "":
				_joining = false
				var name := session.map_name()
				if not FileAccess.file_exists("res://maps/%s.bmap" % name):
					_back("This match is on %s, which this version of the game doesn't have." % name)
				elif not session.set_map(_load_map(name)):
					_back("Your copy of %s is a different version from the server's." % name)
				else:
					_play()
			_ when _waited > JOIN_WAIT:
				_back("The match server didn't answer.")
	elif not _readers.is_empty():
		# voice: talk keys count only while you're playing (not paused, no menu over the game)
		var free := Input.mouse_mode == Input.MOUSE_MODE_CAPTURED
		session.voice_frame(free and Input.is_action_pressed("team_voice"), free and Input.is_action_pressed("near_voice"), not free)
		var hud := session.hud()
		for node in _readers:
			if node.has_method("on_hud"):
				node.on_hud(hud)
			else:
				node.hud = hud
		for e in session.take_events():
			game_event.emit(e)
		if session.match_ended():
			_back()

## Shows a map's scene and sets up its pieces and doors; returns its `.bmap` text.
func _load_map(name: String) -> String:
	var path := "res://maps/" + name
	var scene: Node = load(path + ".tscn").instantiate()
	$Room.add_child(scene)
	var text := FileAccess.get_file_as_string(path + ".bmap")
	var map: Dictionary = JSON.parse_string(text)
	$Pieces.setup(scene, map)
	$Doors.setup(scene, map)
	return text

func _play() -> void:
	# simulation steps at the server's rate (followed in _physics_process: a server says its
	# rate when it welcomes us); frames are drawn between them
	Engine.physics_ticks_per_second = session.tick_rate()
	Engine.max_physics_steps_per_frame = 16
	for node in [$Player, $Player/Gun, $Others, $Grenades, $Bomb, $Pieces, $Doors, $Hud, _buy]:
		node.session = session
		if node.has_method("on_event"):
			game_event.connect(node.on_event)
	_arms.camera = $Player
	_arms.gun = $Player/Gun
	_arms.session = session
	$Player.add_child(_arms)
	_readers = [$Hud, _buy, $Player/Gun, $Bomb, _arms]
	$Hud.camera = $Player

func _physics_process(_delta: float) -> void:
	# prediction must step exactly as often as the server, or it's corrected all the time
	var rate := session.tick_rate()
	if Engine.physics_ticks_per_second != rate:
		Engine.physics_ticks_per_second = rate

## Back to the menu, saying why if something went wrong.
func _back(why := "") -> void:
	if _left:
		return
	_left = true
	_joining = false
	if why != "":
		LobbyLink.tell(why)
	session.stop()
	get_tree().change_scene_to_file.call_deferred("res://scenes/menu.tscn")

func _exit_tree() -> void:
	session.stop()
