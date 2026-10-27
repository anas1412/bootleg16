## A framed button with square corners, never a solid fill. `primary` is the screen's one main
## action (amber, action type, 4 frame); otherwise a white label on a resting 2 frame, which
## turns amber while it's the picked one of a toggle group.
## The hover fill eases in and out, and the label glitches for a moment as the pointer arrives.
## Frame and fill are the button's own style box, so they're always drawn under the label.
class_name FrameButton
extends Button

@export var primary := false
## Tighter sides (8 instead of 24), for a row of buttons that share a width.
@export var compact := false

var _hover := 0.0  # 0 at rest, 1 hovered; eased
var _press := 0.0
var _box := StyleBoxFlat.new()

func _init(label := "", is_primary := false) -> void:
	text = label
	primary = is_primary

func _ready() -> void:
	focus_mode = Control.FOCUS_ALL
	mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	var role := "action" if primary else "tab"
	if Tokens.TYPE[role].upper:
		text = text.to_upper()
	add_theme_font_override("font", UiTheme.font(role))
	add_theme_font_size_override("font_size", UiTheme.size(role))
	_ink()
	add_theme_color_override("font_disabled_color", Tokens.TEXT_OFF)
	toggled.connect(func(_on): _ink(); _restyle())
	_box.set_border_width_all(Tokens.STROKE_FRAME if primary else Tokens.STROKE_HAIR)
	_box.content_margin_left = Tokens.SPACE_XL if primary else (Tokens.SPACE_S if compact else Tokens.SPACE_L)
	_box.content_margin_right = _box.content_margin_left
	_box.content_margin_top = Tokens.SPACE_M if primary else Tokens.SPACE_S
	_box.content_margin_bottom = _box.content_margin_top
	for s in ["normal", "hover", "pressed", "disabled", "focus", "hover_pressed"]:
		add_theme_stylebox_override(s, _box)
	_restyle()
	mouse_entered.connect(_on_enter)
	mouse_exited.connect(func(): _ease("_hover", 0.0))
	button_down.connect(func(): _ease("_press", 1.0))
	button_up.connect(func(): _ease("_press", 0.0))

func _picked() -> bool:
	return toggle_mode and button_pressed

func _ink() -> void:
	var ink := Tokens.YOU if primary or _picked() else Tokens.TEXT
	for c in ["font_color", "font_hover_color", "font_pressed_color", "font_focus_color", "font_hover_pressed_color"]:
		add_theme_color_override(c, ink)

func _on_enter() -> void:
	if disabled:
		return
	_ease("_hover", 1.0)
	Motion.glitch(self)

func _ease(prop: String, to: float) -> void:
	var t := Motion.tween(self)
	t.tween_method(func(v): set(prop, v); _restyle(), get(prop), to, Tokens.HOVER)

## The frame and fill for the current state (disabled, picked, hovered, held).
func _restyle() -> void:
	var frame: Color
	var fill: Color
	if disabled:
		frame = Tokens.LINE
		fill = Color.TRANSPARENT
	elif primary or _picked():
		frame = Tokens.YOU
		fill = Color(Tokens.YOU, 0.12 * _hover + 0.08 * _press)
	else:
		frame = Tokens.LINE.lerp(Tokens.TEXT_DIM, _hover)
		fill = Color(Tokens.PANEL_HI, _hover)
	if _box.border_color != frame or _box.bg_color != fill:
		_box.border_color = frame
		_box.bg_color = fill

func _draw() -> void:
	# `disabled` has no signal: catch its changes when the button redraws
	_restyle()
	if has_focus() and not disabled:
		draw_rect(Rect2(Vector2.ZERO, size).grow(Tokens.SPACE_XS), Tokens.TEXT, false, Tokens.STROKE_HAIR)
