## Grenades in the air, smoke clouds and blasts, until the P5 effects: a small ball per
## grenade, a grey cloud the size the server blocks sight with (it swells in and thins out),
## and a short flash of light where an HE or the bomb goes off.
extends Node3D

## The server's smoke: 140 units round (`net::server::visibility`), 21 seconds.
const CLOUD := 140 * 0.0254
const SMOKE_SECONDS := 21.0
const COLOURS := {"he": Color(0.3, 0.34, 0.2), "flash": Color(0.8, 0.82, 0.85), "smoke": Color(0.6, 0.62, 0.64)}

var session: Session
var _balls: Array[MeshInstance3D] = []
var _clouds: Array[MeshInstance3D] = []
var _cloud_look := StandardMaterial3D.new()

func _ready() -> void:
	_cloud_look.albedo_color = Color(0.62, 0.64, 0.66, 0.96)
	_cloud_look.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	# seen from inside too (walking into a smoke)
	_cloud_look.cull_mode = BaseMaterial3D.CULL_DISABLED
	_cloud_look.roughness = 1.0

func on_event(e: Dictionary) -> void:
	if e.type in ["explosion", "exploded"]:
		_blast(e.at, 1.0 if e.type == "explosion" else 2.5)

func _process(_delta: float) -> void:
	if session == null:
		return
	var flying := []
	var clouds := []
	for g in session.grenades():
		(clouds if g.kind == "cloud" else flying).append(g)
	_fit(_balls, flying.size(), _ball)
	_fit(_clouds, clouds.size(), _cloud)
	for i in flying.size():
		_balls[i].position = flying[i].at
		(_balls[i].material_override as StandardMaterial3D).albedo_color = COLOURS[flying[i].kind]
	for i in clouds.size():
		var c: Dictionary = clouds[i]
		var age: float = SMOKE_SECONDS - c.left
		_clouds[i].position = c.at
		_clouds[i].scale = Vector3.ONE * clampf(0.3 + age, 0.3, 1.0)
		_clouds[i].transparency = 1.0 - clampf(c.left / 2.0, 0.0, 1.0)

## Shows `n` of a pool, making more with `make` as needed.
func _fit(pool: Array, n: int, make: Callable) -> void:
	while pool.size() < n:
		var m: MeshInstance3D = make.call()
		add_child(m)
		pool.append(m)
	for i in pool.size():
		pool[i].visible = i < n

func _ball() -> MeshInstance3D:
	var m := MeshInstance3D.new()
	var s := SphereMesh.new()
	s.radius = 0.05
	s.height = 0.1
	m.mesh = s
	m.material_override = StandardMaterial3D.new()
	return m

func _cloud() -> MeshInstance3D:
	var m := MeshInstance3D.new()
	var s := SphereMesh.new()
	s.radius = CLOUD
	s.height = CLOUD * 2.0
	m.mesh = s
	m.material_override = _cloud_look
	m.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return m

## A flash of light and a fireball that swells and fades.
func _blast(at: Vector3, size: float) -> void:
	var light := OmniLight3D.new()
	light.light_color = Color(1.0, 0.7, 0.35)
	light.omni_range = 8.0 * size
	light.light_energy = 6.0
	var ball := MeshInstance3D.new()
	var s := SphereMesh.new()
	s.radius = 0.5
	s.height = 1.0
	ball.mesh = s
	var fire := StandardMaterial3D.new()
	fire.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	fire.albedo_color = Color(1.0, 0.65, 0.3)
	fire.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	ball.material_override = fire
	var root := Node3D.new()
	root.position = at
	root.add_child(light)
	root.add_child(ball)
	add_child(root)
	var t := create_tween().set_parallel().set_trans(Tween.TRANS_EXPO).set_ease(Tween.EASE_OUT)
	t.tween_property(ball, "scale", Vector3.ONE * 3.0 * size, 0.4)
	t.tween_property(fire, "albedo_color:a", 0.0, 0.4)
	t.tween_property(light, "light_energy", 0.0, 0.5)
	t.chain().tween_callback(root.queue_free)
