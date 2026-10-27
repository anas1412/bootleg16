## One settings row: the setting's name on the left, its control on the right. Two kinds:
## a stepper (‹ value ›, click the arrows or the value to cycle) and a slider with its value.
## Hovering a row lights its panel; every change is reported through `changed`.
class_name SettingRow
extends PanelContainer

signal changed(value)

var _value_label: Label
var _choices: Array = []
var _index := 0
var _slider: HSlider
var _format := "%d"
var _hover := 0.0

static func stepper(title: String, choices: Array, index: int) -> SettingRow:
	var r := SettingRow.new(title)
	r._choices = choices
	r._index = index
	var box := HBoxContainer.new()
	box.add_theme_constant_override("separation", Tokens.SPACE_M)
	var prev := r._arrow("‹", -1)
	r._value_label = UiTheme.label("body", "", Tokens.TEXT)
	r._value_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	r._value_label.custom_minimum_size.x = Tokens.COLUMN * 2 + Tokens.GUTTER - 2 * (Tokens.SPACE_XL + Tokens.SPACE_M)
	var next := r._arrow("›", 1)
	for c in [prev, r._value_label, next]:
		box.add_child(c)
	r._control(box)
	r._show()
	return r

static func slider(title: String, lo: float, hi: float, step: float, value: float, format := "%d") -> SettingRow:
	var r := SettingRow.new(title)
	r._format = format
	var box := HBoxContainer.new()
	box.add_theme_constant_override("separation", Tokens.SPACE_M)
	r._slider = HSlider.new()
	r._slider.min_value = lo
	r._slider.max_value = hi
	r._slider.step = step
	r._slider.value = value
	r._slider.custom_minimum_size.x = Tokens.COLUMN * 2 + Tokens.GUTTER - Tokens.COLUMN / 2 - Tokens.SPACE_M
	r._slider.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	r._value_label = UiTheme.label("label", "", Tokens.TEXT)
	r._value_label.custom_minimum_size.x = Tokens.COLUMN / 2
	r._value_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
	r._slider.value_changed.connect(func(v):
		r._value_label.text = r._format % v
		r.changed.emit(v))
	r._value_label.text = format % value
	box.add_child(r._slider)
	box.add_child(r._value_label)
	r._control(box)
	return r

func _init(title := "") -> void:
	var box := StyleBoxFlat.new()
	box.bg_color = Color.TRANSPARENT
	box.content_margin_left = Tokens.SPACE_M
	box.content_margin_right = Tokens.SPACE_M
	box.content_margin_top = Tokens.SPACE_S
	box.content_margin_bottom = Tokens.SPACE_S
	add_theme_stylebox_override("panel", box)
	var row := HBoxContainer.new()
	row.name = "Row"
	row.add_theme_constant_override("separation", Tokens.SPACE_L)
	var name_label := UiTheme.label("body", title, Tokens.TEXT)
	name_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	name_label.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(name_label)
	add_child(row)
	mouse_filter = Control.MOUSE_FILTER_PASS
	mouse_entered.connect(func(): _light(1.0))
	mouse_exited.connect(func(): _light(0.0))

## Sets a stepper's choice without reporting it (used when a preset changes many rows).
func set_index(i: int) -> void:
	_index = i
	_show()

func set_value(v: float) -> void:
	_slider.set_value_no_signal(v)
	_value_label.text = _format % v

func _control(c: Control) -> void:
	c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	get_node("Row").add_child(c)

func _arrow(glyph: String, dir: int) -> Button:
	var b := Button.new()
	b.text = glyph
	b.flat = true
	b.focus_mode = Control.FOCUS_ALL
	b.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	b.custom_minimum_size = Vector2(Tokens.SPACE_XL, Tokens.SPACE_XL)
	b.add_theme_font_override("font", UiTheme.font("tab"))
	b.add_theme_font_size_override("font_size", UiTheme.size("tab"))
	b.add_theme_color_override("font_color", Tokens.TEXT_DIM)
	b.add_theme_color_override("font_hover_color", Tokens.YOU)
	b.pressed.connect(func(): _step(dir))
	return b

func _step(dir: int) -> void:
	_index = posmod(_index + dir, _choices.size())
	_show()
	Motion.glitch(_value_label)
	changed.emit(_index)

func _show() -> void:
	_value_label.text = str(_choices[_index])

func _light(to: float) -> void:
	var t := Motion.tween(self)
	t.tween_method(func(v):
		_hover = v
		(get_theme_stylebox("panel") as StyleBoxFlat).bg_color = Color(Tokens.PANEL_HI, v), _hover, to, Tokens.HOVER)
