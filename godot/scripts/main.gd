## The game: practice on a map, or the match the lobby found (`LobbyLink.pending`). Shows the
## map's scene, hands the session the map's `.bmap` (what the rules collide with, exported from
## the same scene) and gives the session to the parts that use it. Esc pauses; leaving, a
## failed join or the match's end go back to the menu.
extends Node3D

const PRACTICE_MAP := "data_center"
## A server that hasn't welcomed us in this long isn't coming.
const JOIN_WAIT := 10.0

var session := Session.new()
var _joining := false
var _waited := 0.0

func _ready() -> void:
	Binds.install()
	var pause := preload("res://scripts/pause.gd").new()
	pause.leave.connect(_back)
	add_child(pause)
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
	if not session.start_practice(text, 3):
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
	elif session.match_ended():
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
	for node in [$Player, $Others, $Pieces, $Doors, $Hud/Speed]:
		node.session = session

func _physics_process(_delta: float) -> void:
	# prediction must step exactly as often as the server, or it's corrected all the time
	var rate := session.tick_rate()
	if Engine.physics_ticks_per_second != rate:
		Engine.physics_ticks_per_second = rate

## Back to the menu, saying why if something went wrong.
func _back(why := "") -> void:
	_joining = false
	if why != "":
		LobbyLink.tell(why)
	session.stop()
	get_tree().change_scene_to_file.call_deferred("res://scenes/menu.tscn")

func _exit_tree() -> void:
	session.stop()
