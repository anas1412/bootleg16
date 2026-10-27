## Everyone else as plain figures in their side's colour (Bureau blue, Strays pink), where the
## simulation says, until the P4 characters: a body, a head and the gun they hold, pointing
## where they face. The dead lie where they fell, greyed, until the next round.
extends Node3D

## A player is 72 units tall standing, 36 crouched (metres).
const TALL := 1.83
const CROUCHED := 0.91
const RADIUS := 0.4

var session: Session
var _looks := {}

func _process(_delta: float) -> void:
	if session == null:
		return
	var others := session.others(Engine.get_physics_interpolation_fraction())
	while get_child_count() < others.size():
		add_child(_figure())
	for i in get_child_count():
		var fig := get_child(i) as Node3D
		fig.visible = i < others.size()
		if not fig.visible:
			continue
		var o: Dictionary = others[i]
		var tall := CROUCHED if o.crouched else TALL
		var body := fig.get_child(0) as MeshInstance3D
		var capsule := body.mesh as CapsuleMesh
		if capsule.height != tall - 0.25:  # (a new height rebuilds the mesh)
			capsule.height = tall - 0.25
		body.position.y = (tall - 0.25) / 2.0
		fig.get_child(1).position.y = tall - 0.13
		fig.get_child(2).visible = o.alive
		fig.get_child(2).position.y = tall * 0.7
		body.material_override = _look(o.team, o.alive)
		fig.get_child(1).material_override = body.material_override
		fig.position = o.feet
		fig.rotation = Vector3(0, o.yaw, 0)
		if not o.alive:
			# on their back, along the floor
			fig.rotation.x = -PI / 2.0
			fig.position.y += RADIUS * 0.75

## A body (capsule), a head and a gun (a box pointing ahead), coloured by `_look`.
func _figure() -> Node3D:
	var fig := Node3D.new()
	var body := MeshInstance3D.new()
	var capsule := CapsuleMesh.new()
	capsule.radius = RADIUS * 0.75
	body.mesh = capsule
	fig.add_child(body)
	var head := MeshInstance3D.new()
	var ball := SphereMesh.new()
	ball.radius = 0.13
	ball.height = 0.26
	head.mesh = ball
	fig.add_child(head)
	var gun := MeshInstance3D.new()
	var box := BoxMesh.new()
	box.size = Vector3(0.06, 0.08, 0.5)
	gun.mesh = box
	gun.position = Vector3(0.18, 0, -0.35)
	var dark := StandardMaterial3D.new()
	dark.albedo_color = Color(0.1, 0.1, 0.12)
	gun.material_override = dark
	fig.add_child(gun)
	return fig

## One material per side and state, shared by every figure: the side's colour, lit a little
## from inside so they read in dark halls; greyed when dead.
func _look(team: String, alive: bool) -> StandardMaterial3D:
	var key := team + str(alive)
	if not _looks.has(key):
		var c := Tokens.BUREAU if team == "ct" else Tokens.STRAYS
		var m := StandardMaterial3D.new()
		m.albedo_color = c if alive else c.lerp(Color(0.2, 0.2, 0.22), 0.7)
		m.emission_enabled = alive
		m.emission = c
		m.emission_energy_multiplier = 0.25
		m.roughness = 0.7
		_looks[key] = m
	return _looks[key]
