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
## The Quaternius gun for each weapon (`weapons/quaternius`, CC0, used as they are) and how
## long it is in your hands (metres along the barrel). Grenades and the bomb have none in the
## pack: they stay the box shapes above.
const MODELS := {
	"ak47": ["AssaultRifle_4", 0.62], "galil": ["AssaultRifle_5", 0.6], "m4a1": ["AssaultRifle2_1", 0.6],
	"famas": ["Bullpup_1", 0.5], "p90": ["Bullpup_2", 0.45], "scout": ["SniperRifle_1", 0.75], "awp": ["SniperRifle_3", 0.8],
	"mac10": ["SubmachineGun_1", 0.3], "tmp": ["SubmachineGun_2", 0.32], "mp5navy": ["SubmachineGun_3", 0.45], "ump45": ["SubmachineGun_4", 0.45],
	"glock18": ["Pistol_5", 0.19], "usp": ["Pistol_6", 0.22], "p228": ["Pistol_1", 0.19], "deagle": ["Pistol_4", 0.25],
	"knife": ["Bayonet", 0.28],
}
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
## Where your hands hold it (`arms.gd` reaches them): the right hand on the grip, the left on
## the handguard (under the right for a pistol); null where that hand is free.
var grip_right: Marker3D
var grip_left: Marker3D

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
	grip_right = null
	grip_left = null
	var model := _model(weapon)
	if model:
		add_child(model)
		return
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
	# a grenade or the bomb: in the right hand, the left one free
	grip_right = _grip(self, Vector3(0.02, -0.03, 0.02))

## The weapon's gun model, its barrel pointing ahead, its middle a little ahead of the grip,
## drawn with the hands' shader (never into a wall) in the colours it came with. Null when the
## pack has none for it.
func _model(weapon: String) -> Node3D:
	if not MODELS.has(weapon):
		return null
	var spec: Array = MODELS[weapon]
	var m: Node3D = load("res://weapons/quaternius/%s.fbx" % spec[0]).instantiate()
	var box := AABB()
	var first := true
	for mi in m.find_children("*", "MeshInstance3D", true, false):
		var mesh := mi as MeshInstance3D
		# the mesh's place in the model as the file turns it (it isn't in the tree yet: no
		# global transforms), so the box's x is the barrel whatever the file's root holds
		var to_model := Transform3D()
		var n: Node = mesh
		while n != m:
			to_model = (n as Node3D).transform * to_model
			n = n.get_parent()
		to_model = Transform3D(m.basis, Vector3.ZERO) * to_model
		var b := to_model * mesh.get_aabb()
		box = b if first else box.merge(b)
		first = false
		mesh.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		for i in mesh.mesh.get_surface_count():
			var src := mesh.get_active_material(i)
			var paint := ShaderMaterial.new()
			paint.shader = SHADER
			paint.set_shader_parameter("albedo", (src as BaseMaterial3D).albedo_color if src is BaseMaterial3D else METAL)
			mesh.set_surface_override_material(i, paint)
	var length: float = spec[1]
	var s := length / maxf(box.size.x, 0.001)
	var holder := Node3D.new()
	holder.add_child(m)
	# the files lie along +x: a quarter turn points them ahead (-z)
	var turn := Basis(Vector3.UP, PI / 2.0).scaled(Vector3.ONE * s)
	m.basis = turn * m.basis
	m.position = -(turn * box.get_center()) + Vector3(0, 0, -length * 0.15)
	# the hands: the right a little behind the middle and low (the grip), the left ahead on
	# the handguard; a pistol's left hand cups the right, a knife's is free
	var height := box.size.y * s
	match family(weapon):
		"pistol":
			grip_right = _grip(holder, Vector3(0, -height * 0.3, 0.0))
			grip_left = _grip(holder, Vector3(-0.025, -height * 0.4, 0.01))
		"knife":
			grip_right = _grip(holder, Vector3(0, 0, length * 0.2))
		_:
			grip_right = _grip(holder, Vector3(0, -height * 0.3, length * 0.05))
			grip_left = _grip(holder, Vector3(0, -height * 0.15, -length * 0.35))
	return holder

## A hand's place on the held weapon (it moves with the weapon's kick, bob and sway).
func _grip(on: Node3D, at: Vector3) -> Marker3D:
	var g := Marker3D.new()
	g.position = at
	on.add_child(g)
	return g
