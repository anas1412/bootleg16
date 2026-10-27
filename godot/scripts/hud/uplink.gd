## The assistant, bottom centre, while it talks and a moment after: its face in its colour
## (its own rig sheet, `ui/assistant`), its voice as a waveform out to both sides, its name and
## its line. It only ever says what your team already knows (the brain decides that).
extends Control

## Claudette, ChildGBT, the Twins (her, him), Grumpk: name, face sheet, colour (as the Bevy
## client's `assistant::ASSISTANTS`).
const WHO := [
	["Claudette", "rig_claudette", Color(0.910, 0.529, 0.373)],
	["ChildGBT", "rig_childgbt", Color(0.435, 0.890, 0.757)],
	["Twins", "rig_sister", Color(0.663, 0.545, 1.0)],
	["Twins", "rig_brother", Color(0.663, 0.545, 1.0)],
	["Grumpk", "rig_grumpk", Color(0.831, 1.0, 0.227)],
]
## Face sheets: 11 layers across, 9 moods down, 256 px cells; the mouth-shut and talking layers
## are swapped in by the talk, the rest always show.
const CELL := 256
const LAYERS := 11
const SHUT := 5
const TALK := 8
const FACE := 72.0
## A finished line stays this long.
const HOLD := 4.0

var _face := Control.new()
var _layers: Array[TextureRect] = []
var _frame := Panel.new()
var _name: Label
var _line: Label
var _waves: Array[HudWave] = []
var _talk := 0.0
var _left := 0.0
var _box := VBoxContainer.new()

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_box.add_theme_constant_override("separation", Tokens.SPACE_XS)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_CENTER
	row.add_theme_constant_override("separation", Tokens.SPACE_M)
	_face.custom_minimum_size = Vector2(FACE, FACE)
	_face.clip_contents = true
	for i in LAYERS:
		var t := TextureRect.new()
		t.set_anchors_preset(Control.PRESET_FULL_RECT)
		t.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		t.texture = AtlasTexture.new()
		_face.add_child(t)
		_layers.append(t)
	_frame.set_anchors_preset(Control.PRESET_FULL_RECT)
	_face.add_child(_frame)
	for i in 2:
		_waves.append(HudWave.new(16, Tokens.TEXT, 28.0))
	row.add_child(_waves[0])
	row.add_child(_face)
	row.add_child(_waves[1])
	for c in row.get_children():
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_name = HudText.label("label")
	_line = HudText.label("body")
	_line.add_theme_font_size_override("font_size", 25)
	for c in [row, _name, _line]:
		if c is Label:
			c.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		_box.add_child(c)
	add_child(_box)
	_box.visible = false

## The assistant `who` (an index into WHO) says `text` with a mood (0 neutral to 8 annoyed,
## as `assistant::lines::Mood`).
func say(who: int, mood: int, text: String) -> void:
	var w: Array = WHO[clampi(who, 0, WHO.size() - 1)]
	var sheet: Texture2D = load("res://ui/assistant/%s.png" % w[1])
	for i in LAYERS:
		var a := _layers[i].texture as AtlasTexture
		a.atlas = sheet
		a.region = Rect2(i * CELL, mood * CELL, CELL, CELL)
		_layers[i].visible = i != SHUT and i != TALK
	var frame := StyleBoxFlat.new()
	frame.draw_center = false
	frame.set_border_width_all(Tokens.STROKE_HAIR)
	frame.border_color = w[2]
	_frame.add_theme_stylebox_override("panel", frame)
	_name.text = w[0]
	_name.add_theme_color_override("font_color", w[2])
	for wave in _waves:
		wave.color = w[2]
	_line.text = text
	# about as long as reading the line aloud (as the Bevy client's uplink)
	_talk = minf(0.6 + text.length() * 0.038, 3.2)
	_left = _talk + HOLD
	_box.visible = true
	_box.modulate.a = 1.0
	_box.reset_size()
	_box.position = Vector2((size.x - _box.size.x) / 2.0, size.y - _box.size.y - Tokens.SPACE_XL)
	Motion.glitch.call_deferred(_face)
	Motion.glitch.call_deferred(_line)

func _process(delta: float) -> void:
	if not _box.visible:
		return
	_left -= delta
	var talking := _left > HOLD
	# while it talks, the mouth moves and the voice runs out to the sides
	var open := talking and int(Time.get_ticks_msec() / 110) % 2 == 0
	_layers[TALK].visible = open
	for wave in _waves:
		wave.level = 0.9 if talking else 0.0
	if _left <= 0.0:
		_box.visible = false
