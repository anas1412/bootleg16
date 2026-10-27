## The Play page: your character's title card on the left, your squad on the right, and at the
## bottom right the mode and DEPLOY. Practice starts the game; the queue and custom games
## arrive with the lobby connection, so they show as not ready yet.
extends Control

signal deploy(mode: int)

enum Mode { CASUAL, CUSTOM, PRACTICE }
const MODES := ["Casual", "Custom", "Practice"]
const READY := [false, false, true]  # which modes can start today

## Placeholder character until characters come (last). F swaps the side to judge both looks.
const SIDES := {
	"strays": {"label": "Strays · Attack", "line": "Plants the bomb. Every round.", "color": Tokens.STRAYS},
	"bureau": {"label": "Bureau · Defend", "line": "Stops the bomb. Every round.", "color": Tokens.BUREAU},
}

var side := "strays"
var mode := Mode.PRACTICE
var stage: Node

var _title := VBoxContainer.new()
var _side_label: Label
var _rule := ColorRect.new()
var _line: Label
var _squad := VBoxContainer.new()
var _modes := HBoxContainer.new()
var _deploy := FrameButton.new("Deploy  ›", true)

func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_build_title()
	_build_squad()
	_build_bottom()
	_apply_side()

func _build_title() -> void:
	_title.add_theme_constant_override("separation", Tokens.SPACE_M)
	_side_label = UiTheme.label("label", "")
	var name_label := UiTheme.label("name", "Vera")
	var kana := UiTheme.label("kana", "ヴェラ", Tokens.TEXT_DIM)
	_rule.custom_minimum_size = Vector2(Grid.w(2), Tokens.STROKE_FRAME)
	_rule.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_line = UiTheme.label("body", "", Tokens.TEXT_DIM)
	for c in [_side_label, name_label, kana, _rule, _line]:
		_title.add_child(c)
	Grid.place(_title, 0, 4, 312)
	add_child(_title)

func _build_squad() -> void:
	_squad.add_theme_constant_override("separation", Tokens.SPACE_S)
	Grid.place(_squad, 9, 3, 312)
	add_child(_squad)
	_fill_squad()

## You, and a slot to invite someone (invites arrive with the lobby connection).
func _fill_squad() -> void:
	for c in _squad.get_children():
		c.queue_free()
	var color: Color = SIDES[side].color
	_squad.add_child(UiTheme.label("label", "Your squad · 1 / 5", Tokens.TEXT_DIM))
	_squad.add_child(SquadRow.new("Vera", "You", color, true))
	_squad.add_child(SquadRow.new("Invite", "Soon", color, false, true))
	if stage:
		stage.set_squad(1)

func _build_bottom() -> void:
	_modes.add_theme_constant_override("separation", Tokens.SPACE_S)
	for i in MODES.size():
		var b := FrameButton.new(MODES[i])
		b.compact = true
		b.toggle_mode = true
		b.button_pressed = i == mode
		b.disabled = not READY[i]
		b.tooltip_text = "" if READY[i] else "Arrives with the lobby connection"
		b.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		b.pressed.connect(_pick_mode.bind(i))
		_modes.add_child(b)
	_deploy.custom_minimum_size = Vector2(Grid.w(3), Tokens.SPACE_MAX)
	_deploy.pressed.connect(func(): deploy.emit(mode))
	var bottom := VBoxContainer.new()
	bottom.add_theme_constant_override("separation", Tokens.SPACE_M)
	bottom.add_child(_modes)
	bottom.add_child(_deploy)
	add_child(bottom)
	Grid.place(bottom, 9, 3, 0)
	# sits on the bottom margin
	bottom.position.y = Grid.bottom() - bottom.get_combined_minimum_size().y

func _pick_mode(i: int) -> void:
	mode = i
	for j in _modes.get_child_count():
		(_modes.get_child(j) as Button).button_pressed = j == i

func _apply_side() -> void:
	var s: Dictionary = SIDES[side]
	_side_label.text = s.label
	_side_label.add_theme_color_override("font_color", s.color)
	_rule.color = s.color
	_line.text = s.line
	_fill_squad()
	if stage:
		stage.set_side(s.color)

func _unhandled_key_input(event: InputEvent) -> void:
	if visible and event.pressed and not event.echo and event.physical_keycode == KEY_F:
		side = "bureau" if side == "strays" else "strays"
		_apply_side()
		Motion.glitch(_title)

## Arrival: the title card, then the squad, then the bottom, one stagger apart.
func enter() -> void:
	Motion.enter_all([_title, _squad, _modes.get_parent()])
