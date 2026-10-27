## Who's talking, top left: a row per voice you hear right now. Team radio shows in the
## speaker's side colour; proximity voice (anyone alive near you, enemies too) is marked NEAR,
## and MUFFLED when a wall is between you. Each row has the speaker's picture and their
## voice's waveform.
extends TiltPanel

const SIZE := Vector2(420, 200)
const PIC := 34.0

var _list := VBoxContainer.new()
var _rows := {}  # id -> [row, wave]

func _init() -> void:
	super(SIZE, 1.0)
	_list.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_list.add_theme_constant_override("separation", Tokens.SPACE_XS)
	content.add_child(_list)

## `Session.voices()`.
func refresh(voices: Array) -> void:
	var now := {}
	for v in voices:
		now[v.id] = v
		if not _rows.has(v.id):
			_rows[v.id] = _row(v)
		_rows[v.id][1].level = v.level
	for id in _rows.keys():
		if not now.has(id):
			_rows[id][0].queue_free()
			_rows.erase(id)

func _row(v: Dictionary) -> Array:
	var side := Tokens.BUREAU if v.team == "ct" else Tokens.STRAYS
	var near: bool = v.channel == "near"
	var row := PanelContainer.new()
	row.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	var box := StyleBoxFlat.new()
	box.bg_color = Color(Tokens.BG, 0.62)
	box.content_margin_right = Tokens.SPACE_L
	row.add_theme_stylebox_override("panel", box)
	var line := HBoxContainer.new()
	line.add_theme_constant_override("separation", Tokens.SPACE_S)
	var pic := HudAvatar.new(PIC, true)
	pic.show_player({"name": v.name, "bot": false, "alive": true, "you": false, "health": -1}, side)
	var tag := "Near" if near else "Team"
	if v.muffled:
		tag += " · Muffled"
	var wave := HudWave.new(10, Tokens.TEXT if near else side)
	for c in [pic, HudText.label("hud_name", v.name, side), HudText.label("hud_tag", tag, Tokens.TEXT_DIM), wave]:
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		line.add_child(c)
	row.add_child(line)
	_list.add_child(row)
	Motion.glitch.call_deferred(row)
	return [row, wave]
