## The gun in your hands, a placeholder of boxes until the P4 gun kits: its shape follows the
## weapon you hold, it comes up when you switch, kicks when you shoot, dips to reload, bobs as
## you walk and lags a little behind your aim. Hidden when you're dead or looking down a scope.
## A strip on it shows your side's colour (sides swap at halftime).
extends Node3D

const SHADER := preload("res://shaders/viewmodel.gdshader")
const METAL := Color(0.12, 0.13, 0.15)
const STEEL := Color(0.55, 0.57, 0.6)
const GRIP := Color(0.06, 0.06, 0.07)
const TAN := Color(0.42, 0.36, 0.26)
## A scope zooms below this vertical field of view (degrees): the gun goes away.
const ZOOMED_FOV := 60.0

## Each family's parts: [size, centre, colour or "glow"], metres, -z forward from the grip;
## the strip is on the left face, the one you see.
const RIFLE := [
	[Vector3(0.055, 0.075, 0.42), Vector3(0, 0.02, -0.18), METAL],
	[Vector3(0.022, 0.022, 0.22), Vector3(0, 0.035, -0.5), GRIP],
	[Vector3(0.035, 0.12, 0.05), Vector3(0, -0.06, -0.2), GRIP],
	[Vector3(0.045, 0.07, 0.16), Vector3(0, 0.0, 0.1), METAL],
	[Vector3(0.004, 0.012, 0.3), Vector3(-0.029, 0.03, -0.18), "glow"],
]
const SCOPE := [Vector3(0.035, 0.035, 0.18), Vector3(0, 0.08, -0.15), GRIP]
const SMG := [
	[Vector3(0.05, 0.07, 0.3), Vector3(0, 0.02, -0.12), METAL],
	[Vector3(0.02, 0.02, 0.1), Vector3(0, 0.03, -0.32), GRIP],
	[Vector3(0.03, 0.1, 0.04), Vector3(0, -0.05, -0.12), GRIP],
	[Vector3(0.004, 0.012, 0.2), Vector3(-0.027, 0.03, -0.12), "glow"],
]
const PISTOL := [
	[Vector3(0.035, 0.035, 0.19), Vector3(0, 0.035, -0.06), METAL],
	[Vector3(0.03, 0.09, 0.045), Vector3(0, -0.02, 0.0), GRIP],
	[Vector3(0.004, 0.008, 0.1), Vector3(-0.019, 0.04, -0.06), "glow"],
]
const KNIFE := [
	[Vector3(0.025, 0.03, 0.1), Vector3(0, 0, 0), GRIP],
	[Vector3(0.008, 0.035, 0.18), Vector3(0, 0.005, -0.14), STEEL],
]
const BOMB := [
	[Vector3(0.16, 0.06, 0.11), Vector3(0, 0, -0.05), TAN],
	[Vector3(0.1, 0.004, 0.04), Vector3(0, 0.032, -0.05), "glow"],
]
const GRENADE_COLOURS := {"hegrenade": Color(0.3, 0.34, 0.2), "flashbang": Color(0.5, 0.52, 0.55), "smokegrenade": Color(0.7, 0.72, 0.74)}
## Where each family rests in front of the eye.
const REST := {"rifle": Vector3(0.2, -0.2, -0.36), "pistol": Vector3(0.2, -0.19, -0.4), "knife": Vector3(0.2, -0.19, -0.34), "grenade": Vector3(0.19, -0.19, -0.34), "bomb": Vector3(0.08, -0.24, -0.38)}

var session: Session
## This frame's `Session.hud()`, read once by main.gd for every part.
var hud := {}
var _weapon := ""
var _family := "rifle"
var _raise := 0.0
var _kick := 0.0
var _swing := 0.0
var _throw := 0.0
var _reload := 0.0
var _plant := 0.0
var _bob := 0.0
var _bob_amount := 0.0
var _sway := Vector2.ZERO
var _last_look := Vector2.ZERO

static func family(weapon: String) -> String:
	match weapon:
		"knife":
			return "knife"
		"glock18", "usp", "p228", "deagle":
			return "pistol"
		"hegrenade", "flashbang", "smokegrenade":
			return "grenade"
		"c4":
			return "bomb"
	return "rifle"

func on_event(e: Dictionary) -> void:
	match e.type:
		"shot":
			_kick = 1.0
		"swing":
			_swing = 1.0
		"throw":
			_throw = 1.0

func _process(delta: float) -> void:
	var cam := get_parent() as Camera3D
	if session == null:
		visible = false
		return
	visible = hud.get("alive", false) and cam.fov > ZOOMED_FOV
	if not visible:
		return
	if hud.weapon != _weapon:
		_build(hud.weapon, hud.team)
	# eased towards their targets, the same at any frame rate
	var k := func(rate: float) -> float: return 1.0 - exp(-rate * delta)
	_raise = lerpf(_raise, 1.0, k.call(12.0))
	_kick = lerpf(_kick, 0.0, k.call(16.0 if _weapon != "awp" else 8.0))
	_swing = lerpf(_swing, 0.0, k.call(10.0))
	_throw = lerpf(_throw, 0.0, k.call(8.0))
	_reload = lerpf(_reload, 1.0 if hud.reloading else 0.0, k.call(10.0))
	_plant = lerpf(_plant, 1.0 if hud.planting >= 0.0 else 0.0, k.call(10.0))
	var speed := clampf(session.speed() / 250.0, 0.0, 1.0)
	_bob_amount = lerpf(_bob_amount, speed, k.call(8.0))
	_bob += delta * (4.0 + 6.0 * speed)
	# the gun trails your aim a little and catches up
	var look := Vector2(cam.rotation.y, cam.rotation.x)
	var turned := (look - _last_look) / maxf(delta, 0.001)
	_last_look = look
	_sway = _sway.lerp((turned * 0.004).limit_length(0.03), k.call(10.0))

	var at: Vector3 = REST[_family]
	at += Vector3(sin(_bob) * 0.006, -absf(cos(_bob)) * 0.008, 0.0) * _bob_amount
	at += Vector3(_sway.x, -_sway.y, 0.0)
	at.y -= (1.0 - _raise) * 0.25 + _reload * 0.08 + _plant * 0.06
	at.z += _kick * (0.09 if _weapon == "awp" else 0.04) - _throw * 0.15
	position = at
	rotation = Vector3(
		deg_to_rad((1.0 - _raise) * -40.0 + _kick * 6.0 - _reload * 10.0 + _throw * 30.0),
		deg_to_rad(_swing * 35.0),
		deg_to_rad(_reload * 25.0 - _swing * 20.0))

## Puts together the shape of `weapon`, lowered, to come up.
func _build(weapon: String, team: String) -> void:
	_weapon = weapon
	_family = family(weapon)
	_raise = 0.0
	for c in get_children():
		c.queue_free()
	var parts: Array
	match _family:
		"knife":
			parts = KNIFE
		"pistol":
			parts = PISTOL
		"grenade":
			parts = [[Vector3(0.06, 0.09, 0.06), Vector3.ZERO, GRENADE_COLOURS.get(weapon, METAL)]]
		"bomb":
			parts = BOMB
		_:
			parts = (RIFLE + [SCOPE]) if weapon in ["scout", "awp"] else (SMG if weapon in ["mac10", "tmp", "mp5navy", "ump45", "p90"] else RIFLE)
	var side := Tokens.BUREAU if team == "ct" else Tokens.STRAYS
	var big := 1.25 if weapon == "deagle" else 1.0
	for p in parts:
		var mesh := BoxMesh.new()
		mesh.size = p[0] * big
		var m := ShaderMaterial.new()
		m.shader = SHADER
		if p[2] is String:
			# the bomb's light is always red; a gun's strip is your side's
			var c: Color = Tokens.ALERT if _family == "bomb" else side
			m.set_shader_parameter("albedo", c)
			m.set_shader_parameter("glow", Color(c, 1.5))
		else:
			m.set_shader_parameter("albedo", p[2])
		mesh.material = m
		var part := MeshInstance3D.new()
		part.mesh = mesh
		part.position = p[1] * big
		part.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		add_child(part)
