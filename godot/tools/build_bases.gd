## Builds the two faction bases behind the home screen, grey boxes from our own blocks and
## shaders (shaders/base.gdshader, shaders/prop.gdshader):
##   godot --headless --path godot --script res://tools/build_bases.gd
## base_strays.tscn: the Strays' hideout, a lock-up under an overpass. A half-open roller shutter
## with their mark sprayed on it, the street's pink light under it, a neon tube on a pillar,
## pallets, tyres, a workbench and a string of bulbs.
## base_bureau.tscn: the Bureau's ops room. A wall of blinds with the city behind (its light
## falls through the slats onto the floor), their seal on a dark pier over the squad, a lit map
## table and server cabinets on a polished stone floor.
## Both: five slots for the squad (you in front, the rest in a V behind), lit from the front
## and rimmed in the side's colour from behind; base.gd puts the figures in.
## Metres, y up, the camera looking down -z at the squad.
extends SceneTree

## [x, z, yaw (degrees)] of each slot; slot 0 is you.
const SLOTS := [[0.0, 0.0, 0.0], [-1.05, -0.85, 14.0], [1.05, -0.85, -14.0], [-2.0, -1.7, 22.0], [2.0, -1.7, -22.0]]
const POSES := {
	"strays": ["Idle", "Idle_Talking", "Idle", "Sitting_Idle", "Idle"],
	"bureau": ["Idle", "Idle", "Idle_Talking", "Idle_Talking", "Idle"],
}
const WARM := Color(1.0, 0.86, 0.66)

var _root: Node3D
var _mats := {}

func _initialize() -> void:
	_build("strays", "BaseStrays", Tokens.STRAYS)
	_build("bureau", "BaseBureau", Tokens.BUREAU)
	quit()

func _build(side: String, name: String, color: Color) -> void:
	_root = Node3D.new()
	_root.name = name
	_root.set_script(load("res://scripts/menu/base.gd"))
	_root.set("side", side)
	_environment(side, color)
	var cam := Camera3D.new()
	cam.name = "Camera"
	cam.fov = 30.0
	cam.current = true
	cam.attributes = CameraAttributesPractical.new()
	cam.attributes.dof_blur_far_enabled = true
	cam.attributes.dof_blur_far_distance = 10.0
	cam.attributes.dof_blur_far_transition = 6.0
	cam.attributes.dof_blur_amount = 0.05
	_add(_root, cam)
	var slots := _group("Slots")
	for i in SLOTS.size():
		var m := Marker3D.new()
		m.name = str(i)
		m.position = Vector3(SLOTS[i][0], 0, SLOTS[i][1])
		m.rotation_degrees.y = SLOTS[i][2]
		m.set_meta("pose", POSES[side][i])
		_add(slots, m)
	var lights := _group("Lights")
	# rims from behind on both sides, in the side's colour (base.gd set_side recolours them)
	for s in [-1, 1]:
		var rim := _spot(lights, "Rim" + ("L" if s < 0 else "R"), Vector3(3.2 * s, 2.9, -3.6), Vector3(0, 1.3, -0.8), color, 9.0 if s < 0 else 7.0, 26.0)
		rim.set_meta("rim", true)
		rim.light_volumetric_fog_energy = 0.6
	if side == "strays":
		_strays(lights)
	else:
		_bureau(lights)
	var packed := PackedScene.new()
	packed.pack(_root)
	var out := "res://scenes/base_%s.tscn" % side
	print("%s: %s" % [out, error_string(ResourceSaver.save(packed, out))])
	_root.free()

func _environment(side: String, color: Color) -> void:
	var e := Environment.new()
	e.background_mode = Environment.BG_COLOR
	e.background_color = Color(0.008, 0.009, 0.012)
	e.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	e.ambient_light_color = Color(0.35, 0.36, 0.4).lerp(color, 0.1)
	e.ambient_light_energy = 0.12
	e.tonemap_mode = Environment.TONE_MAPPER_ACES
	e.tonemap_exposure = 1.05
	e.glow_enabled = true
	e.glow_intensity = 0.45
	e.glow_bloom = 0.04
	e.ssr_enabled = side == "bureau"  # the polished floor; the hideout's wet patches do with specular
	e.ssao_enabled = true
	e.ssao_intensity = 1.6
	e.volumetric_fog_enabled = true
	e.volumetric_fog_density = 0.008
	e.volumetric_fog_albedo = Color(0.85, 0.85, 0.88)
	var w := WorldEnvironment.new()
	w.name = "Environment"
	w.environment = e
	_add(_root, w)

func _strays(lights: Node3D) -> void:
	var room := _group("Room")
	var concrete := _mat(0, Color(0.34, 0.33, 0.32))
	var blocks := _mat(1, Color(0.3, 0.29, 0.29))
	_box(room, "Floor", Vector3(0, -0.05, -3), Vector3(20, 0.1, 18), concrete)
	_box(room, "Ceiling", Vector3(0, 4.75, -2), Vector3(20, 0.3, 14), concrete)
	# the back wall round the shutter's opening (4.4 x 3.4 m)
	_box(room, "WallL", Vector3(-5.6, 2.5, -4.8), Vector3(6.8, 5, 0.4), blocks)
	_box(room, "WallR", Vector3(5.6, 2.5, -4.8), Vector3(6.8, 5, 0.4), blocks)
	_box(room, "WallTop", Vector3(0, 4.2, -4.8), Vector3(4.4, 1.6, 0.4), blocks)
	var shutter := _mat(2, Color(0.34, 0.35, 0.37))
	_box(room, "Shutter", Vector3(0, 2.2, -4.72), Vector3(4.4, 2.4, 0.05), shutter)
	_box(room, "ShutterBar", Vector3(0, 1.02, -4.7), Vector3(4.4, 0.06, 0.09), _plain(Color(0.08, 0.08, 0.09), 0.7))
	for s in [-1, 1]:
		_box(room, "Guide" + str(s), Vector3(2.24 * s, 1.7, -4.66), Vector3(0.08, 3.4, 0.12), _plain(Color(0.12, 0.12, 0.13), 0.6))
		_box(room, "Pillar" + str(s), Vector3(3.7 * s, 2.35, -3.0), Vector3(0.55, 4.7, 0.55), concrete)
	# the Strays' mark, sprayed on the shutter
	_mark(room, "res://ui/marks/strays.svg", Vector3(0, 2.35, -4.69), 1.25, Tokens.STRAYS * 0.85, false)
	# the street outside, under the shutter: lit pink, its light running in across the floor
	_box(room, "Street", Vector3(0, 2.0, -10), Vector3(8, 4, 0.1), _neon(Color(0.9, 0.3, 0.5), 0.25))
	var street := _spot(lights, "Street", Vector3(0, 0.9, -7.5), Vector3(0, 0, 1.5), Color(1.0, 0.42, 0.62), 9.0, 30.0)
	street.light_volumetric_fog_energy = 1.5
	# the neon tube on the left pillar, and its glow
	_box(room, "Neon", Vector3(-3.7, 1.9, -2.71), Vector3(0.045, 1.6, 0.045), _neon(Tokens.STRAYS, 5.0))
	_omni(lights, "NeonGlow", Vector3(-3.6, 1.9, -2.45), Tokens.STRAYS, 1.4, 3.2)
	# pallets and crates, back left; the crate slot 3 sits on
	var wood := Color(0.36, 0.29, 0.21)
	var props := _group("Props")
	_prop(props, "Pallets", Vector3(-5.3, 0.36, -3.7), Vector3(1.3, 0.72, 1.0), 1, wood)
	_prop(props, "PalletsTop", Vector3(-5.1, 0.97, -3.8), Vector3(1.05, 0.5, 0.9), 1, wood)
	_prop(props, "Crate", Vector3(-2.9, 0.28, -4.0), Vector3(0.62, 0.56, 0.62), 1, wood.darkened(0.15))
	var seat := _prop(props, "Seat", Vector3(-2.03, 0.22, -1.86), Vector3(0.62, 0.44, 0.5), 1, wood.darkened(0.25))
	seat.add_to_group("slot_3", true)
	# a drum, a stack of tyres, the workbench and its lamp, back right
	var drum := MeshInstance3D.new()
	drum.name = "Drum"
	drum.mesh = CylinderMesh.new()
	drum.mesh.top_radius = 0.29
	drum.mesh.bottom_radius = 0.29
	drum.mesh.height = 0.88
	drum.position = Vector3(-3.25, 0.44, -2.2)
	drum.material_override = _plain(Color(0.16, 0.2, 0.24), 0.5, 0.5)
	_add(props, drum)
	for i in 3:
		var tyre := MeshInstance3D.new()
		tyre.name = "Tyre%d" % i
		tyre.mesh = TorusMesh.new()
		tyre.mesh.inner_radius = 0.2
		tyre.mesh.outer_radius = 0.36
		tyre.position = Vector3(3.15 + i * 0.03, 0.08 + i * 0.165, -2.3)
		tyre.scale = Vector3(1, 1.05, 1)
		tyre.material_override = _plain(Color(0.03, 0.03, 0.03), 0.8)
		_add(props, tyre)
	_prop(props, "Bench", Vector3(4.9, 0.46, -3.7), Vector3(2.1, 0.92, 0.8), 2, Color(0.2, 0.21, 0.23))
	_box(props, "Lamp", Vector3(4.4, 1.05, -3.75), Vector3(0.12, 0.2, 0.12), _neon(WARM, 4.0))
	_omni(lights, "BenchLamp", Vector3(4.4, 1.3, -3.5), WARM, 1.2, 3.0)
	# a string of bulbs between the pillars, sagging
	for i in 15:
		var t := i / 14.0
		var bulb := MeshInstance3D.new()
		bulb.name = "Bulb%d" % i
		bulb.mesh = SphereMesh.new()
		bulb.mesh.radius = 0.035
		bulb.mesh.height = 0.07
		# hung low enough to stay under the menu's tabs and high enough to clear the names
		bulb.position = Vector3(lerpf(-3.4, 3.4, t), 3.6 - 0.85 * sin(t * PI), -3.0)
		bulb.material_override = _neon(WARM, 6.0)
		bulb.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		_add(props, bulb)
	_omni(lights, "Bulbs", Vector3(0, 3.1, -2.6), WARM, 0.6, 5.0)
	# the key: a work lamp hanging over the squad, warm white, from the front left
	_spot(lights, "Key", Vector3(-1.6, 4.2, 3.6), Vector3(0, 1.0, -0.8), Color(1.0, 0.93, 0.85), 7.0, 26.0, true)
	_omni(lights, "Fill", Vector3(2.8, 1.6, 4.2), Color(0.7, 0.75, 0.9), 0.35, 9.0)

func _bureau(lights: Node3D) -> void:
	var room := _group("Room")
	_box(room, "Floor", Vector3(0, -0.05, -3), Vector3(20, 0.1, 18), _mat(4, Color(0.07, 0.075, 0.085)))
	var panel := _plain(Color(0.045, 0.048, 0.055), 0.35, 0.3)
	_box(room, "Ceiling", Vector3(0, 4.75, -2), Vector3(20, 0.3, 14), panel)
	# the window wall: blinds either side of a dark pier with the Bureau's seal over the squad,
	# lit from below; mullions every 1.25 m, a sill, a header
	var blind := _mat(3, Color(0.5, 0.52, 0.55), Color(0.55, 0.78, 1.0), 1.8)
	for s in [-1, 1]:
		var b := _box(room, "Blinds" + str(s), Vector3(4.7 * s, 2.25, -4.75), Vector3(7.4, 3.9, 0.04), blind)
		b.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		for i in 6:
			_box(room, "Mullion%d_%d" % [s, i], Vector3(s * (1.0 + i * 1.25), 2.25, -4.68), Vector3(0.07, 3.9, 0.14), _plain(Color(0.02, 0.02, 0.025), 0.4, 0.6))
	_box(room, "Sill", Vector3(0, 0.15, -4.7), Vector3(20, 0.3, 0.3), panel)
	_box(room, "Header", Vector3(0, 4.45, -4.75), Vector3(20, 0.5, 0.3), panel)
	_box(room, "Pier", Vector3(0, 2.4, -4.8), Vector3(2.0, 4.8, 0.3), panel)
	_mark(room, "res://ui/marks/bureau.svg", Vector3(0, 2.75, -4.62), 1.0, Tokens.BUREAU, true)
	_spot(lights, "SealWash", Vector3(0, 0.4, -4.1), Vector3(0, 2.8, -4.65), Color(0.8, 0.92, 1.0), 2.5, 30.0)
	# the city's light through the slats, striped across the floor
	var city := _spot(lights, "City", Vector3(2.5, 4.3, -7.0), Vector3(0.6, 0, -2.4), Color(0.62, 0.8, 1.0), 16.0, 28.0, true)
	city.light_projector = _slats()
	city.light_volumetric_fog_energy = 1.5
	# server cabinets, far left; the lit map table, back right
	var props := _group("Props")
	for i in 2:
		_prop(props, "Cabinet%d" % i, Vector3(-6.2 + i * 0.75, 1.0, -3.4), Vector3(0.72, 2.0, 0.9), 0, Color(0.1, 0.105, 0.12), Tokens.BUREAU)
	_box(props, "Table", Vector3(4.1, 0.43, -2.7), Vector3(2.4, 0.86, 1.2), panel)
	_box(props, "Map", Vector3(4.1, 0.875, -2.7), Vector3(2.3, 0.03, 1.1), _mat(5, Color(0.02, 0.02, 0.02), Tokens.BUREAU, 1.6))
	_omni(lights, "MapGlow", Vector3(4.1, 1.2, -2.7), Tokens.BUREAU, 1.0, 2.6)
	_spot(lights, "Key", Vector3(1.8, 4.2, 3.6), Vector3(0, 1.0, -0.8), Color(0.93, 0.96, 1.0), 8.0, 26.0, true)
	_omni(lights, "Fill", Vector3(-2.8, 1.6, 4.2), Color(1.0, 0.9, 0.8), 0.3, 9.0)

## Horizontal bars for the city light's projector: the blinds' shadow.
func _slats() -> ImageTexture:
	var img := Image.create(8, 128, false, Image.FORMAT_L8)
	for y in 128:
		var lit := (y % 8) < 3
		for x in 8:
			img.set_pixel(x, y, Color.WHITE if lit else Color(0.05, 0.05, 0.05))
	return ImageTexture.create_from_image(img)

func _mat(kind: int, body: Color, glow := Color.BLACK, energy := 1.0) -> ShaderMaterial:
	var key := [kind, body, glow, energy]
	if not _mats.has(key):
		var m := ShaderMaterial.new()
		m.shader = load("res://shaders/base.gdshader")
		m.set_shader_parameter("kind", kind)
		m.set_shader_parameter("body", body)
		m.set_shader_parameter("glow", glow)
		m.set_shader_parameter("glow_energy", energy)
		_mats[key] = m
	return _mats[key]

func _plain(color: Color, rough: float, metal := 0.0) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = color
	m.roughness = rough
	m.metallic = metal
	return m

func _neon(color: Color, energy: float) -> StandardMaterial3D:
	var m := StandardMaterial3D.new()
	m.albedo_color = Color.BLACK
	m.emission_enabled = true
	m.emission = color
	m.emission_energy_multiplier = energy
	return m

func _box(parent: Node, name: String, at: Vector3, size: Vector3, mat: Material) -> MeshInstance3D:
	var b := MeshInstance3D.new()
	b.name = name
	b.mesh = BoxMesh.new()
	b.mesh.size = size
	b.position = at
	b.material_override = mat
	_add(parent, b)
	return b

## A box dressed by prop.gdshader (0 cabinet, 1 crate, 2 machine).
func _prop(parent: Node, name: String, at: Vector3, size: Vector3, kind: int, body: Color, accent := Color(0.25, 1.0, 0.55)) -> MeshInstance3D:
	var key := ["prop", kind, body, accent]
	if not _mats.has(key):
		var m := ShaderMaterial.new()
		m.shader = load("res://shaders/prop.gdshader")
		m.set_shader_parameter("kind", kind)
		m.set_shader_parameter("body", body)
		m.set_shader_parameter("accent", accent)
		_mats[key] = m
	var b := _box(parent, name, at, size, _mats[key])
	b.set_instance_shader_parameter("size", size)
	return b

## A side's mark on a wall: sprayed (lit by the room) or a lit sign.
func _mark(parent: Node, path: String, at: Vector3, width: float, color: Color, lit: bool) -> void:
	var s := Sprite3D.new()
	s.name = "Mark"
	s.texture = load(path)
	s.pixel_size = width / s.texture.get_width()
	s.position = at
	s.modulate = color
	s.shaded = not lit
	s.texture_filter = BaseMaterial3D.TEXTURE_FILTER_LINEAR_WITH_MIPMAPS
	_add(parent, s)

func _spot(parent: Node, name: String, at: Vector3, target: Vector3, color: Color, energy: float, angle: float, shadow := false) -> SpotLight3D:
	var l := SpotLight3D.new()
	l.name = name
	var up := Vector3.UP if absf((target - at).normalized().y) < 0.99 else Vector3.FORWARD
	l.transform = Transform3D(Basis.looking_at(target - at, up), at)
	_add(parent, l)
	l.light_color = color
	l.light_energy = energy
	l.spot_angle = angle
	# far enough that no cone stops short in view (a cut-off shows as blocks on the floor)
	l.spot_range = 24.0
	l.spot_angle_attenuation = 0.8
	l.shadow_enabled = shadow
	return l

func _omni(parent: Node, name: String, at: Vector3, color: Color, energy: float, reach: float) -> OmniLight3D:
	var l := OmniLight3D.new()
	l.name = name
	l.position = at
	l.light_color = color
	l.light_energy = energy
	l.omni_range = reach
	_add(parent, l)
	return l

func _group(name: String) -> Node3D:
	var n := Node3D.new()
	n.name = name
	_add(_root, n)
	return n

func _add(parent: Node, child: Node) -> void:
	parent.add_child(child)
	child.owner = _root
