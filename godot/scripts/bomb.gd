## The bomb in the world once it's down: a small case with a red light that blinks as it
## beeps, faster as it nears the end (CS: you hear the timer, you don't see it). Dropped,
## it lies there unlit (only the Strays are told where).
extends Node3D

var session: Session
## This frame's `Session.hud()`, read once by main.gd for every part.
var hud := {}
var _light := OmniLight3D.new()
var _lamp := MeshInstance3D.new()
var _since_beep := 0.0

func _ready() -> void:
	var case := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.3, 0.1, 0.2)
	case.mesh = box
	var tan := StandardMaterial3D.new()
	tan.albedo_color = Color(0.42, 0.36, 0.26)
	case.material_override = tan
	case.position.y = 0.05
	add_child(case)
	var bulb := BoxMesh.new()
	bulb.size = Vector3(0.04, 0.02, 0.04)
	_lamp.mesh = bulb
	var red := StandardMaterial3D.new()
	red.albedo_color = Tokens.ALERT
	red.emission_enabled = true
	red.emission = Tokens.ALERT
	red.emission_energy_multiplier = 4.0
	_lamp.material_override = red
	_lamp.position = Vector3(0.08, 0.11, 0)
	add_child(_lamp)
	_light.light_color = Tokens.ALERT
	_light.omni_range = 2.0
	_light.position.y = 0.2
	add_child(_light)
	visible = false

func _process(delta: float) -> void:
	if session == null:
		return
	visible = hud.has("bomb_at")
	if not visible:
		return
	position = hud.bomb_at
	var planted: bool = hud.bomb == "planted"
	_since_beep += delta
	# CS 1.6's beeps: from every 1.4 s to every 0.1 s over the last 40 seconds
	var gap := clampf(0.1 + 1.3 * hud.bomb_left / 40.0, 0.1, 1.4)
	if planted and _since_beep >= gap:
		_since_beep = 0.0
	var on := planted and _since_beep < 0.08
	_lamp.visible = on
	_light.light_energy = 2.0 if on else 0.0
