## The top bar: both teams' pictures (a teammate's health under theirs; an enemy's only
## alive or not), each side's score with its emblem, and the timer between them, always in the
## same place and style: buy time (grey), the round (red in the last 10 s), the bomb's fuse
## once it's planted (red, with the bomb icon). Under the timer, who's left: 4 v 3.
extends TiltPanel

const SIZE := Vector2(1000, 112)
const PIC := 44.0

var _sides := {"t": [], "ct": []}
var _score := {}
var _timer: Label
var _bomb: TextureRect
var _under: Label
var _planted := false

func _init() -> void:
	# wide, so a little tilt goes a long way at its ends
	super(SIZE, 0.0, 0.25)
	var row := HBoxContainer.new()
	row.alignment = BoxContainer.ALIGNMENT_CENTER
	row.add_theme_constant_override("separation", Tokens.SPACE_M)
	row.set_anchors_and_offsets_preset(Control.PRESET_TOP_WIDE)
	row.offset_top = Tokens.SPACE_S
	content.add_child(row)
	for team in ["t", "ct"]:
		var pics := HBoxContainer.new()
		pics.add_theme_constant_override("separation", Tokens.SPACE_XS)
		for i in 5:
			var a := HudAvatar.new(PIC)
			pics.add_child(a)
			_sides[team].append(a)
		var color := Tokens.STRAYS if team == "t" else Tokens.BUREAU
		var score := HudText.label("hud_value", "0", color)
		score.add_theme_font_size_override("font_size", 40)
		_score[team] = score
		var mark := TextureRect.new()
		mark.texture = preload("res://ui/marks/strays.svg") if team == "t" else preload("res://ui/marks/bureau.svg")
		mark.expand_mode = TextureRect.EXPAND_IGNORE_SIZE
		mark.stretch_mode = TextureRect.STRETCH_KEEP_ASPECT_CENTERED
		mark.custom_minimum_size = Vector2(22, PIC)
		var block := HBoxContainer.new()
		block.add_theme_constant_override("separation", Tokens.SPACE_S)
		for c in ([score, mark] if team == "t" else [mark, score]):
			c.size_flags_vertical = Control.SIZE_SHRINK_BEGIN
			block.add_child(c)
		score.custom_minimum_size.y = PIC
		score.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
		if team == "t":
			row.add_child(pics)
			row.add_child(block)
			row.add_child(_clock())
		else:
			row.add_child(block)
			row.add_child(pics)

func _clock() -> Control:
	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", 0)
	col.custom_minimum_size.x = 120
	var top := HBoxContainer.new()
	top.alignment = BoxContainer.ALIGNMENT_CENTER
	top.add_theme_constant_override("separation", Tokens.SPACE_S)
	top.custom_minimum_size.y = PIC
	_bomb = HudText.icon("c4", 24, Tokens.ALERT)
	_bomb.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_bomb.visible = false
	_timer = HudText.label("hud_timer", "0:00")
	_timer.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	top.add_child(_bomb)
	top.add_child(_timer)
	_under = HudText.label("label", "", Tokens.TEXT_DIM)
	_under.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	col.add_child(top)
	col.add_child(_under)
	return col

## Everyone (`Session.players`) and this frame's `Session.hud`.
func refresh(players: Array, hud: Dictionary) -> void:
	for team in ["t", "ct"]:
		var mine: Array = players.filter(func(p): return p.team == team)
		var color := Tokens.STRAYS if team == "t" else Tokens.BUREAU
		for i in 5:
			var a: HudAvatar = _sides[team][i]
			a.visible = i < mine.size()
			if a.visible:
				a.show_player(mine[i], color)
		var score := str(hud.get("score_" + team, 0))
		if _score[team].text != score:
			_score[team].text = score
			Motion.glitch(_score[team])
	var planted: bool = hud.get("bomb", "") == "planted"
	if planted != _planted:
		_planted = planted
		_bomb.visible = planted
		Motion.glitch(_timer.get_parent())
	var left: float = maxf(hud.bomb_left if planted else hud.get("left", 0.0), 0.0)
	_timer.text = "%d:%02d" % [int(left) / 60, int(left) % 60]
	var phase: String = hud.get("phase", "")
	var color := Tokens.TEXT
	if planted or (phase == "live" and left <= 10.0):
		color = Tokens.ALERT
	elif phase in ["freeze", "over", "warmup", "gameover"]:
		color = Tokens.TEXT_DIM
	_timer.add_theme_color_override("font_color", color)
	var mine_t: bool = hud.get("team", "t") == "t"
	var us: int = hud.get("alive_t" if mine_t else "alive_ct", 0)
	var them: int = hud.get("alive_ct" if mine_t else "alive_t", 0)
	match phase:
		"warmup":
			_under.text = "Warmup"
		"freeze":
			_under.text = "Buy time"
		"over":
			_under.text = "Round over"
		"gameover":
			_under.text = "Match over"
		_:
			_under.text = "%d v %d" % [us, them]
