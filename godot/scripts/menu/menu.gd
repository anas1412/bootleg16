## The main menu: the lobby stage behind, the wordmark and tabs along the top, and one page
## at a time under them. Settings blur the stage; Esc goes back to Play.
## BOOTLEG16_SHOT=path.png saves a frame after a moment and quits (BOOTLEG16_PAGE=1 for
## Settings), for checking the look.
extends Control

const PAGES := [["Play", "出撃"], ["Settings", "設定"]]

@onready var stage := $"../../Stage"

var _blur := ColorRect.new()
var _tabs := TopTabs.new()
var _mark: TextureRect
var _pages: Array[Control] = []
var _current := 0

func _ready() -> void:
	theme = UiTheme.theme()
	set_anchors_preset(Control.PRESET_FULL_RECT)
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE

	_blur.material = ShaderMaterial.new()
	_blur.material.shader = preload("res://ui/blur.gdshader")
	_blur.material.set_shader_parameter("amount", 0.0)  # a parameter can't be tweened until it's set
	_blur.mouse_filter = Control.MOUSE_FILTER_IGNORE
	_blur.set_anchors_preset(Control.PRESET_FULL_RECT)
	_blur.visible = false
	add_child(_blur)

	var mark := TextureRect.new()
	mark.texture = preload("res://ui/marks/wordmark.svg")
	mark.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
	mark.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT
	add_child(mark)

	for p in PAGES:
		_tabs.add_tab(p[0], p[1])
	_tabs.picked.connect(_open)
	add_child(_tabs)
	_mark = mark
	_place_top()
	_tabs.minimum_size_changed.connect(_place_top)

	var play := preload("res://scripts/menu/play_page.gd").new()
	play.stage = stage
	play.deploy.connect(_deploy)
	var settings := preload("res://scripts/menu/settings_page.gd").new()
	settings.back.connect(func(): _tabs.pick(0))
	for page in [play, settings]:
		page.visible = false
		add_child(page)
		_pages.append(page)
	_pages[0].visible = true
	_pages[0].enter()
	Motion.enter_all([mark, _tabs])

	var start_page := int(OS.get_environment("BOOTLEG16_PAGE"))
	if start_page:
		_tabs.pick(start_page)
	var shot := OS.get_environment("BOOTLEG16_SHOT")
	if shot != "":
		await get_tree().create_timer(2.0).timeout
		get_viewport().get_texture().get_image().save_png(shot)
		get_tree().quit()

## The wordmark is as tall as the tabs, both on the top margin; the tabs are centred.
## Text measures itself late, so this runs again whenever the tabs' size changes.
func _place_top() -> void:
	var bar := _tabs.get_combined_minimum_size()
	var aspect := float(_mark.texture.get_width()) / _mark.texture.get_height()
	_mark.position = Vector2(Grid.x(0), Tokens.MARGIN_Y)
	_mark.size = Vector2(bar.y * aspect, bar.y)
	_tabs.position = Vector2(roundf((Tokens.CANVAS.x - bar.x) / 2.0), Tokens.MARGIN_Y)

func _open(i: int) -> void:
	if i == _current:
		return
	var old := _pages[_current]
	Motion.exit(old).tween_callback(func(): old.visible = false; old.modulate.a = 1.0)
	_current = i
	_pages[i].visible = true
	_pages[i].modulate.a = 1.0
	_pages[i].enter()
	_set_blur(i != 0)

func _set_blur(on: bool) -> void:
	_blur.visible = true
	var t := Motion.tween(_blur)
	t.tween_property(_blur.material, "shader_parameter/amount", 1.0 if on else 0.0, Tokens.ENTER if on else Tokens.EXIT)
	if not on:
		t.tween_callback(func(): _blur.visible = false)

func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_cancel") and _current != 0:
		_tabs.pick(0)

func _deploy(mode: int) -> void:
	if mode == 2:  # practice
		get_tree().change_scene_to_file("res://main.tscn")
