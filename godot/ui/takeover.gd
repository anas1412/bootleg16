## The takeover: for the few moments that matter (match found, bomb planted). It takes the whole
## screen: the game blurs behind, hazard bands slide in from both sides and a framed title
## arrives with a glitch, with a line under it and, if given, buttons. Then it leaves.
class_name Takeover
extends Control

const BAND := 160

var _blur := ColorRect.new()
var _top := ColorRect.new()
var _bottom := ColorRect.new()
var _frame := PanelContainer.new()
var _title: Label
var _line: Label
var _buttons := HBoxContainer.new()
var _leaving: Tween

func _init() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	visible = false
	mouse_filter = Control.MOUSE_FILTER_STOP
	_blur.material = ShaderMaterial.new()
	_blur.material.shader = preload("res://ui/blur.gdshader")
	_blur.material.set_shader_parameter("amount", 0.0)
	_blur.set_anchors_preset(Control.PRESET_FULL_RECT)
	_blur.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_blur)
	for band in [_top, _bottom]:
		band.material = ShaderMaterial.new()
		band.material.shader = preload("res://ui/stripes.gdshader")
		band.material.set_shader_parameter("color", Tokens.YOU)
		band.material.set_shader_parameter("size", Vector2(Tokens.CANVAS.x, BAND))
		band.size = Vector2(Tokens.CANVAS.x, BAND)
		band.mouse_filter = Control.MOUSE_FILTER_IGNORE
		add_child(band)
	# the bands frame the middle: a margin above the top one, the same below the bottom one
	_top.position.y = Tokens.MARGIN_Y + Tokens.SPACE_MAX
	_bottom.position.y = Tokens.CANVAS.y - Tokens.MARGIN_Y - Tokens.SPACE_MAX - BAND

	var box := StyleBoxFlat.new()
	box.bg_color = Tokens.BG
	box.border_color = Tokens.YOU
	box.set_border_width_all(Tokens.STROKE_FRAME)
	box.content_margin_left = Tokens.SPACE_XL
	box.content_margin_right = Tokens.SPACE_XL
	box.content_margin_top = Tokens.SPACE_S
	box.content_margin_bottom = Tokens.SPACE_S
	_frame.add_theme_stylebox_override("panel", box)
	_title = UiTheme.label("takeover", "", Tokens.YOU)
	_frame.add_child(_title)
	_line = UiTheme.label("label", "", Tokens.TEXT)
	_buttons.add_theme_constant_override("separation", Tokens.SPACE_M)
	var middle := VBoxContainer.new()
	middle.alignment = BoxContainer.ALIGNMENT_CENTER
	middle.add_theme_constant_override("separation", Tokens.SPACE_L)
	middle.set_anchors_preset(Control.PRESET_FULL_RECT)
	for c in [_frame, _line, _buttons]:
		c.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
		middle.add_child(c)
	add_child(middle)

## Shows it with a title, a line under it and buttons ([{text, primary, call}]).
func open(title: String, line := "", buttons: Array = []) -> void:
	_title.text = title
	_line.text = line
	for b in _buttons.get_children():
		b.queue_free()
	for spec in buttons:
		var b := FrameButton.new(spec.text, spec.get("primary", false))
		b.pressed.connect(spec.call)
		_buttons.add_child(b)
	# opened again while still fading out: stay, don't let the fade hide it
	if _leaving and _leaving.is_valid():
		_leaving.kill()
		modulate.a = 1.0
	if visible:
		return
	visible = true
	modulate.a = 1.0
	var t := Motion.tween(self).set_parallel()
	t.tween_property(_blur.material, "shader_parameter/amount", 1.0, Tokens.ENTER)
	_top.position.x = -Tokens.CANVAS.x * 0.3
	_bottom.position.x = Tokens.CANVAS.x * 0.3
	for band in [_top, _bottom]:
		band.modulate.a = 0.0
		t.tween_property(band, "position:x", 0.0, Tokens.TAKEOVER)
		t.tween_property(band, "modulate:a", 1.0, Tokens.TAKEOVER)
	_frame.pivot_offset = _frame.size / 2.0
	_frame.scale = Vector2(1.08, 1.08)
	_frame.modulate.a = 0.0
	t.tween_property(_frame, "scale", Vector2.ONE, Tokens.TAKEOVER)
	t.tween_property(_frame, "modulate:a", 1.0, Tokens.TAKEOVER)
	get_tree().create_timer(Tokens.HOVER).timeout.connect(func(): Motion.glitch(_frame))
	Motion.enter_all([_line, _buttons], Tokens.TAKEOVER / 2.0)

## Changes the line under the title (a countdown, who's accepted) without arriving again.
func set_line(line: String) -> void:
	_line.text = line

## Shows or hides the buttons (once you've answered, they go).
func set_buttons_visible(on: bool) -> void:
	_buttons.visible = on

func close() -> void:
	if not visible:
		return
	_leaving = Motion.exit(self)
	_leaving.tween_callback(func(): visible = false; modulate.a = 1.0)
