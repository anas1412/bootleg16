## Your gun, bottom right: what you carry as the game's icons with their keys (the one in
## your hands amber; grenades with how many), the gun's name, its magazine as the big number
## (red at 5 or fewer) and what's left in reserve.
extends TiltPanel

const SIZE := Vector2(400, 150)
const SLOT_ICON := 20.0
const SLOT_MAX_W := 56.0

var _slots := HBoxContainer.new()
var _name: Label
var _clip: Label
var _reserve: Label
var _shown := ""

func _init() -> void:
	super(SIZE, -1.0)
	var col := VBoxContainer.new()
	col.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	col.alignment = BoxContainer.ALIGNMENT_END
	col.add_theme_constant_override("separation", Tokens.SPACE_S)
	content.add_child(col)
	_slots.alignment = BoxContainer.ALIGNMENT_END
	_slots.add_theme_constant_override("separation", Tokens.SPACE_M)
	col.add_child(_slots)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_END
	row.add_theme_constant_override("separation", Tokens.SPACE_S)
	_name = HudText.label("tab", "", Tokens.TEXT_DIM)
	_clip = HudText.label("hud_value", "")
	_reserve = HudText.label("hud_small", "", Tokens.TEXT_DIM)
	for c in [_name, _clip, _reserve]:
		c.size_flags_vertical = Control.SIZE_SHRINK_END
		row.add_child(c)
	_name.custom_minimum_size.x = 0
	col.add_child(row)

func refresh(hud: Dictionary) -> void:
	var g: PackedInt32Array = hud.get("grenades", PackedInt32Array([0, 0, 0]))
	# the slots you have, as [icon, key, count]; rebuilt only when what you carry changes
	var have := []
	if hud.get("primary", "") != "":
		have.append([Weapons.icon(hud.primary), 1, 0])
	if hud.get("secondary", "") != "":
		have.append([Weapons.icon(hud.secondary), 2, 0])
	have.append(["d_knife", 3, 0])
	var nades := ["d_grenade", "d_flashbang", "d_smoke"]
	for i in 3:
		if g[i] > 0:
			have.append([nades[i], 4, g[i]])
	if hud.get("c4", false):
		have.append(["c4", 5, 0])
	var slot: int = hud.get("slot", 3)
	# the grenades share key 4: the one in your hands is the lit one
	var held := Weapons.icon(hud.get("weapon", ""))
	var key := "%s|%d|%s" % [have, slot, held]
	if key != _shown:
		_shown = key
		_build(have, slot, held)
	var gun: bool = hud.get("clip", -1) >= 0
	_name.text = Weapons.shown(hud.get("weapon", ""))
	_clip.visible = gun
	_reserve.visible = gun
	if gun:
		_clip.text = str(hud.clip)
		_clip.add_theme_color_override("font_color", Tokens.ALERT if hud.clip <= 5 else Tokens.TEXT)
		_reserve.text = "/ %d" % hud.reserve

func _build(have: Array, slot: int, held: String) -> void:
	for c in _slots.get_children():
		c.queue_free()
	for s in have:
		var on: bool = s[1] == slot and (slot != 4 or s[0] == held)
		var color := Tokens.YOU if on else Color(Tokens.TEXT, 0.55)
		var col := VBoxContainer.new()
		col.add_theme_constant_override("separation", 2)
		var pic := HBoxContainer.new()
		pic.add_theme_constant_override("separation", 2)
		pic.add_child(HudText.icon(s[0], SLOT_ICON, color, SLOT_MAX_W))
		if s[2] > 1:
			pic.add_child(HudText.label("hud_tag", "x%d" % s[2], color))
		var k := HudText.label("hud_tag", str(s[1]), color)
		k.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		col.add_child(pic)
		col.add_child(k)
		_slots.add_child(col)
