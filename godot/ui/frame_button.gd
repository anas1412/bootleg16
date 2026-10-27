## A framed button with square corners, never a solid fill. `primary` is the screen's one main
## action (amber, action type, 4 frame); otherwise a white label on a resting 2 frame, which
## turns amber while it's the picked one of a toggle group.
## The hover fill eases in and out, and the label glitches for a moment as the pointer arrives.
class_name FrameButton
extends Button

@export var primary := false
## Tighter sides (8 instead of 24), for a row of buttons that share a width.
@export var compact := false

var _hover := 0.0  # 0 at rest, 1 hovered; eased
var _press := 0.0

func _init(label := "", is_primary := false) -> void:
	text = label
	primary = is_primary

func _ready() -> void:
	flat = true
	focus_mode = Control.FOCUS_ALL
	mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	var role := "action" if primary else "tab"
	if Tokens.TYPE[role].upper:
		text = text.to_upper()
	add_theme_font_override("font", UiTheme.font(role))
	add_theme_font_size_override("font_size", UiTheme.size(role))
	_ink()
	add_theme_color_override("font_disabled_color", Tokens.TEXT_OFF)
	toggled.connect(func(_on): _ink(); queue_redraw())
	var pad := StyleBoxEmpty.new()
	pad.content_margin_left = Tokens.SPACE_XL if primary else (Tokens.SPACE_S if compact else Tokens.SPACE_L)
	pad.content_margin_right = pad.content_margin_left
	pad.content_margin_top = Tokens.SPACE_M if primary else Tokens.SPACE_S
	pad.content_margin_bottom = pad.content_margin_top
	for s in ["normal", "hover", "pressed", "disabled", "focus"]:
		add_theme_stylebox_override(s, pad)
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
	t.tween_method(func(v): set(prop, v); queue_redraw(), get(prop), to, Tokens.HOVER)

func _draw() -> void:
	var r := Rect2(Vector2.ZERO, size)
	var w := Tokens.STROKE_FRAME if primary else Tokens.STROKE_HAIR
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
	if fill.a > 0.0:
		draw_rect(r, fill)
	# the frame sits inside the button's box, so its outer edge is the box edge
	draw_rect(r.grow(-w / 2.0), frame, false, w)
	if has_focus() and not disabled:
		draw_rect(r.grow(Tokens.SPACE_XS), Tokens.TEXT, false, Tokens.STROKE_HAIR)
