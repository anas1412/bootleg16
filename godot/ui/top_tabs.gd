## The top tabs: each name with its katakana under it. The page you're on is white with an amber
## underline that slides to a newly picked tab; the others are dim and brighten on hover.
class_name TopTabs
extends Control

signal picked(index: int)

var current := 0
var _row := HBoxContainer.new()
var _line := ColorRect.new()
var _slide: Tween

func _init() -> void:
	_row.add_theme_constant_override("separation", Tokens.SPACE_XL)
	add_child(_row)
	_line.color = Tokens.YOU
	_line.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_line)
	_row.sort_children.connect(_place_line.bind(false))
	_row.minimum_size_changed.connect(update_minimum_size)

func _get_minimum_size() -> Vector2:
	return _row.get_combined_minimum_size()

func add_tab(name_text: String, kana: String) -> void:
	var b := Button.new()
	b.flat = true
	b.focus_mode = Control.FOCUS_ALL
	b.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	for s in ["normal", "hover", "pressed", "focus"]:
		b.add_theme_stylebox_override(s, StyleBoxEmpty.new())
	var box := VBoxContainer.new()
	box.mouse_filter = Control.MOUSE_FILTER_IGNORE
	box.add_theme_constant_override("separation", Tokens.SPACE_XS)
	var title := UiTheme.label("tab", name_text, Tokens.TEXT_DIM)
	var sub := UiTheme.label("tab_kana", kana, Tokens.TEXT_OFF)
	for l: Label in [title, sub]:
		l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		l.mouse_filter = Control.MOUSE_FILTER_IGNORE
		box.add_child(l)
	b.add_child(box)
	# the tab is as wide as its name, with room under the katakana for the 4 tall underline,
	# 8 clear of it; labels measure themselves late, so follow their size
	var fit := func():
		b.custom_minimum_size = box.get_combined_minimum_size() + Vector2(0, Tokens.SPACE_S + Tokens.STROKE_FRAME)
		box.size = box.get_combined_minimum_size()
		box.position.x = 0
	box.minimum_size_changed.connect(fit)
	fit.call()
	var i := _row.get_child_count()
	b.pressed.connect(func(): pick(i))
	b.mouse_entered.connect(func():
		if i != current:
			_tint(b, Tokens.TEXT)
			Motion.glitch(b))
	b.mouse_exited.connect(func(): if i != current: _tint(b, Tokens.TEXT_DIM))
	b.set_meta("title", title)
	b.set_meta("sub", sub)
	_row.add_child(b)
	_restyle()

func pick(i: int) -> void:
	if i == current:
		return
	current = i
	_restyle()
	_place_line(true)
	Motion.glitch(_row.get_child(i))
	picked.emit(i)

func _restyle() -> void:
	for i in _row.get_child_count():
		var b: Button = _row.get_child(i)
		var on := i == current
		_tint(b, Tokens.TEXT if on else Tokens.TEXT_DIM)
		(b.get_meta("sub") as Label).add_theme_color_override("font_color", Tokens.YOU if on else Tokens.TEXT_OFF)

func _tint(b: Button, c: Color) -> void:
	var title: Label = b.get_meta("title")
	if not title.is_inside_tree():
		title.add_theme_color_override("font_color", c)
		return
	var t := Motion.tween(title)
	t.tween_method(func(v): title.add_theme_color_override("font_color", v),
		title.get_theme_color("font_color"), c, Tokens.HOVER)

## The underline sits under the current tab: 4 tall, flush with the tab's bottom.
func _place_line(animate: bool) -> void:
	if _row.get_child_count() == 0:
		return
	var tab: Button = _row.get_child(current)
	var pos := Vector2(tab.position.x, tab.position.y + tab.size.y - Tokens.STROKE_FRAME)
	var sz := Vector2(tab.size.x, Tokens.STROKE_FRAME)
	# a slide still running would drag the line back to where the tab was before layout
	if _slide and _slide.is_valid():
		_slide.kill()
	if animate and is_visible_in_tree():
		_slide = Motion.tween(_line).set_parallel()
		_slide.tween_property(_line, "position", pos, Tokens.ENTER)
		_slide.tween_property(_line, "size", sz, Tokens.ENTER)
	else:
		_line.position = pos
		_line.size = sz
