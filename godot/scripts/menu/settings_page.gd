## The Settings page: the page title, three sections (graphics, controls, audio) and their rows
## on the left half of the grid. Every row changes the setting the moment you use it.
extends Control

const SECTIONS := [["Graphics", "画質"], ["Controls", "操作"], ["Audio", "音声"]]

var _tabs := TopTabs.new()
var _title: Label
var _scroll := ScrollContainer.new()
var _rows := VBoxContainer.new()
var _preset_row: SettingRow
var _option_rows := {}

func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_title = UiTheme.label("page", "Settings")
	Grid.place(_title, 0, 6, 184)
	add_child(_title)
	for s in SECTIONS:
		_tabs.add_tab(s[0], s[1])
	_tabs.position = Vector2(Grid.x(0), 184 + Tokens.TYPE.page.line + Tokens.SPACE_XL)
	_tabs.picked.connect(_show)
	add_child(_tabs)
	_rows.add_theme_constant_override("separation", Tokens.SPACE_XS)
	_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_scroll.add_child(_rows)
	_rows.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	add_child(_scroll)
	Settings.changed.connect(_sync)
	_show(0)

func _layout_scroll() -> void:
	var top := _tabs.position.y + _tabs.get_combined_minimum_size().y + Tokens.SPACE_XL
	_scroll.position = Vector2(Grid.x(0), top)
	_scroll.size = Vector2(Grid.w(6), Grid.bottom() - top)

func _show(section: int) -> void:
	for c in _rows.get_children():
		c.queue_free()
	_option_rows.clear()
	_preset_row = null
	match section:
		0: _graphics()
		1: _controls()
		2: _audio()
	_layout_scroll.call_deferred()
	Motion.enter_all(_rows.get_children())

func _graphics() -> void:
	_preset_row = _add(SettingRow.stepper("Preset", Settings.PRESETS, Settings.preset))
	_preset_row.changed.connect(func(i):
		# Custom is reached by changing an option, not picked directly
		if i == Settings.CUSTOM:
			i = 0
			_preset_row.set_index(0)
		Settings.set_preset(i))
	for key in Settings.GRAPHICS:
		var o: Dictionary = Settings.GRAPHICS[key]
		var shown: Array = o.choices.map(func(c): return o.fmt % c if o.has("fmt") else str(c))
		var row := _add(SettingRow.stepper(o.title, shown, Settings.graphics[key]))
		row.changed.connect(func(i): Settings.set_graphics(key, i))
		_option_rows[key] = row
	_add(SettingRow.stepper("Display", Settings.DISPLAY, Settings.display)).changed.connect(
		func(i): Settings.set_value("display", i))
	_add(SettingRow.stepper("Vertical sync", Settings.VSYNC, Settings.vsync)).changed.connect(
		func(i): Settings.set_value("vsync", i))
	var caps: Array = Settings.FPS.map(func(f): return "No cap" if f == 0 else "%d fps" % f)
	_add(SettingRow.stepper("Frame cap", caps, Settings.fps_cap)).changed.connect(
		func(i): Settings.set_value("fps_cap", i))

func _controls() -> void:
	_add(SettingRow.slider("Mouse sensitivity", 0.1, 10.0, 0.05, Settings.sensitivity, "%.2f")).changed.connect(
		func(v): Settings.set_value("sensitivity", v))

func _audio() -> void:
	_add(SettingRow.slider("Volume", 0, 100, 1, Settings.volume * 100, "%d%%")).changed.connect(
		func(v): Settings.set_value("volume", v / 100.0))

func _add(row: SettingRow) -> SettingRow:
	_rows.add_child(row)
	return row

## A preset moves every option, and an option can turn the preset into Custom.
func _sync() -> void:
	if _preset_row:
		_preset_row.set_index(Settings.preset)
	for key in _option_rows:
		_option_rows[key].set_index(Settings.graphics[key])

func enter() -> void:
	Motion.enter_all([_title, _tabs])
	Motion.enter_all(_rows.get_children(), Tokens.STAGGER * 2)
