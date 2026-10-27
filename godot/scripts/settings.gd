## Your settings (autoload `Settings`): kept in user://settings.cfg, applied the moment they
## change. Graphics come as presets; changing any one option makes the preset Custom.
## Nothing here changes what other players see or how the game plays: the field of view,
## smoke and flashes stay the same at every setting.
extends Node

signal changed

## Where they're kept; tests point BOOTLEG16_SETTINGS elsewhere so yours are never touched.
var path := OS.get_environment("BOOTLEG16_SETTINGS") if OS.has_environment("BOOTLEG16_SETTINGS") else "user://settings.cfg"
const PRESETS := ["Low", "Medium", "High", "Ultra", "Custom"]
const CUSTOM := 4

## Each graphics option: its choices, and its value at Low, Medium, High, Ultra.
const GRAPHICS := {
	"render_scale": {"title": "Render scale", "choices": [50, 67, 77, 85, 100], "fmt": "%d%%", "preset": [2, 3, 4, 4]},
	"upscaler": {"title": "Upscaling", "choices": ["Bilinear", "FSR 1", "FSR 2"], "preset": [1, 1, 2, 2]},
	"aa": {"title": "Anti-aliasing", "choices": ["Off", "FXAA", "SMAA", "TAA", "MSAA 4x"], "preset": [1, 2, 2, 3]},
	"shadows": {"title": "Shadows", "choices": ["Low", "Medium", "High", "Ultra"], "preset": [0, 1, 2, 3]},
	"ssao": {"title": "Ambient occlusion", "choices": ["Off", "On"], "preset": [0, 0, 1, 1]},
	"ssr": {"title": "Reflections", "choices": ["Off", "On"], "preset": [0, 0, 0, 1]},
	"filtering": {"title": "Texture filtering", "choices": ["Bilinear", "2x", "4x", "8x", "16x"], "preset": [1, 2, 3, 4]},
	"detail": {"title": "Model detail", "choices": ["Low", "Medium", "High"], "preset": [0, 1, 2, 2]},
}
const DISPLAY := ["Windowed", "Borderless", "Fullscreen"]
const VSYNC := ["Off", "On", "Mailbox"]
const FPS := [0, 60, 120, 144, 165, 240, 300]

var preset := 2
var graphics := {}
var display := 0
var vsync := 2
var fps_cap := 0
var sensitivity := 2.0
var volume := 0.8

func _ready() -> void:
	_set_preset_values(preset)
	_load()
	get_tree().node_added.connect(_on_node_added)
	apply()

## Picks a preset (0 Low to 3 Ultra) and applies it.
func set_preset(i: int) -> void:
	preset = i
	if i != CUSTOM:
		_set_preset_values(i)
	_commit()

## Changes one graphics option; the preset becomes Custom unless it still matches one.
func set_graphics(key: String, index: int) -> void:
	graphics[key] = index
	preset = _matching_preset()
	_commit()

func set_value(key: String, value) -> void:
	set(key, value)
	_commit()

func _set_preset_values(i: int) -> void:
	for key in GRAPHICS:
		graphics[key] = GRAPHICS[key].preset[i]

func _matching_preset() -> int:
	for i in 4:
		if GRAPHICS.keys().all(func(k): return graphics[k] == GRAPHICS[k].preset[i]):
			return i
	return CUSTOM

func _commit() -> void:
	apply()
	_save()
	changed.emit()

## Puts every setting into effect.
func apply() -> void:
	var vp := get_viewport()
	var g := graphics
	vp.scaling_3d_scale = GRAPHICS.render_scale.choices[g.render_scale] / 100.0
	vp.scaling_3d_mode = [Viewport.SCALING_3D_MODE_BILINEAR, Viewport.SCALING_3D_MODE_FSR, Viewport.SCALING_3D_MODE_FSR2][g.upscaler]
	# FSR 2 does its own anti-aliasing; the others stack on top of it badly
	var aa: int = 3 if g.upscaler == 2 else g.aa
	vp.screen_space_aa = [Viewport.SCREEN_SPACE_AA_DISABLED, Viewport.SCREEN_SPACE_AA_FXAA, Viewport.SCREEN_SPACE_AA_SMAA, Viewport.SCREEN_SPACE_AA_DISABLED, Viewport.SCREEN_SPACE_AA_DISABLED][aa]
	vp.use_taa = aa == 3 and g.upscaler != 2
	vp.msaa_3d = Viewport.MSAA_4X if aa == 4 else Viewport.MSAA_DISABLED
	var atlas: int = [1024, 2048, 4096, 8192][g.shadows]
	vp.positional_shadow_atlas_size = atlas
	RenderingServer.directional_shadow_atlas_set_size(atlas, true)
	var soft: int = [RenderingServer.SHADOW_QUALITY_HARD, RenderingServer.SHADOW_QUALITY_SOFT_LOW, RenderingServer.SHADOW_QUALITY_SOFT_MEDIUM, RenderingServer.SHADOW_QUALITY_SOFT_HIGH][g.shadows]
	RenderingServer.directional_soft_shadow_filter_set_quality(soft)
	RenderingServer.positional_soft_shadow_filter_set_quality(soft)
	vp.anisotropic_filtering_level = [Viewport.ANISOTROPY_DISABLED, Viewport.ANISOTROPY_2X, Viewport.ANISOTROPY_4X, Viewport.ANISOTROPY_8X, Viewport.ANISOTROPY_16X][g.filtering]
	vp.mesh_lod_threshold = [4.0, 2.0, 1.0][g.detail]
	for env in get_tree().root.find_children("*", "WorldEnvironment", true, false):
		_apply_environment(env)

	DisplayServer.window_set_vsync_mode([DisplayServer.VSYNC_DISABLED, DisplayServer.VSYNC_ENABLED, DisplayServer.VSYNC_MAILBOX][vsync])
	Engine.max_fps = FPS[fps_cap]
	_apply_display()
	AudioServer.set_bus_volume_db(0, linear_to_db(volume))

func _apply_environment(node: WorldEnvironment) -> void:
	if node.environment:
		node.environment.ssao_enabled = graphics.ssao == 1
		node.environment.ssr_enabled = graphics.ssr == 1

func _on_node_added(node: Node) -> void:
	if node is WorldEnvironment:
		_apply_environment.call_deferred(node)

func _apply_display() -> void:
	# only touch the window when the mode really changes: switching modes is slow on some
	# desktops, and doing it for nothing flickers
	var want: int = [DisplayServer.WINDOW_MODE_WINDOWED, DisplayServer.WINDOW_MODE_FULLSCREEN, DisplayServer.WINDOW_MODE_EXCLUSIVE_FULLSCREEN][display]
	if DisplayServer.window_get_mode() != want:
		DisplayServer.window_set_mode(want)

func _save() -> void:
	var cf := ConfigFile.new()
	cf.set_value("graphics", "preset", preset)
	for key in graphics:
		cf.set_value("graphics", key, graphics[key])
	for key in ["display", "vsync", "fps_cap"]:
		cf.set_value("display", key, get(key))
	cf.set_value("controls", "sensitivity", sensitivity)
	cf.set_value("audio", "volume", volume)
	cf.save(path)

func _load() -> void:
	var cf := ConfigFile.new()
	if cf.load(path) != OK:
		return
	preset = cf.get_value("graphics", "preset", preset)
	for key in GRAPHICS:
		var i: int = cf.get_value("graphics", key, graphics.get(key, GRAPHICS[key].preset[2]))
		graphics[key] = clampi(i, 0, GRAPHICS[key].choices.size() - 1)
	display = clampi(cf.get_value("display", "display", display), 0, DISPLAY.size() - 1)
	vsync = clampi(cf.get_value("display", "vsync", vsync), 0, VSYNC.size() - 1)
	fps_cap = clampi(cf.get_value("display", "fps_cap", fps_cap), 0, FPS.size() - 1)
	sensitivity = clampf(cf.get_value("controls", "sensitivity", sensitivity), 0.1, 10.0)
	volume = clampf(cf.get_value("audio", "volume", volume), 0.0, 1.0)
