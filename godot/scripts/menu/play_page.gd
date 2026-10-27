## The Play page: your character's title card on the left, your squad on the right, and at the
## bottom right a status line, the mode and DEPLOY. Casual queues, Custom starts your custom
## game (the leader decides both), Practice starts the game on its own. A found match takes
## the screen over until you accept or pass.
extends Control

signal deploy(mode: int)

enum Mode { CASUAL, CUSTOM, PRACTICE }
const MODES := ["Casual", "Custom", "Practice"]

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
var _squad := preload("res://scripts/menu/squad_panel.gd").new()
var _bottom := VBoxContainer.new()
var _status: Label
var _modes := HBoxContainer.new()
var _deploy := FrameButton.new("Deploy  ›", true)
var _takeover := Takeover.new()
var _notice_left := 0.0

func _ready() -> void:
	set_anchors_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_build_title()
	Grid.place(_squad, 9, 3, 312)
	add_child(_squad)
	_build_bottom()
	add_child(_takeover)
	_apply_side()
	LobbyLink.state_changed.connect(func(_s): _refresh())
	LobbyLink.notice.connect(_show_notice)
	var unread := LobbyLink.take_unread()
	if unread != "":
		_show_notice(unread)
	_refresh()

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

func _build_bottom() -> void:
	_status = UiTheme.label("label", "", Tokens.TEXT_DIM)
	_status.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_modes.add_theme_constant_override("separation", Tokens.SPACE_S)
	for i in MODES.size():
		var b := FrameButton.new(MODES[i])
		b.compact = true
		b.toggle_mode = true
		b.size_flags_horizontal = Control.SIZE_EXPAND_FILL
		b.pressed.connect(_pick_mode.bind(i))
		_modes.add_child(b)
	_deploy.custom_minimum_size = Vector2(Grid.w(3), Tokens.SPACE_MAX)
	_deploy.pressed.connect(_on_deploy)
	_bottom.add_theme_constant_override("separation", Tokens.SPACE_M)
	for c in [_status, _modes, _deploy]:
		_bottom.add_child(c)
	add_child(_bottom)
	Grid.place(_bottom, 9, 3, 0)
	# sits on the bottom margin, growing upwards when the status line shows or wraps
	_bottom.minimum_size_changed.connect(_sit_bottom)
	_sit_bottom()

func _sit_bottom() -> void:
	_bottom.size.y = _bottom.get_combined_minimum_size().y
	_bottom.position.y = Grid.bottom() - _bottom.size.y

## The leader's pick sets the party's mode at the lobby; Practice is yours alone.
func _pick_mode(i: int) -> void:
	mode = i
	if i != Mode.PRACTICE and LobbyLink.leading() and LobbyLink.state.mode != i:
		LobbyLink.lobby.set_mode(i)
	_refresh()

func _on_deploy() -> void:
	var s: Dictionary = LobbyLink.state
	if mode == Mode.PRACTICE:
		deploy.emit(Mode.PRACTICE)
	elif s.get("can_rejoin", false):
		LobbyLink.lobby.rejoin()
	elif s.get("searching", -1.0) >= 0.0:
		LobbyLink.lobby.cancel()
	else:
		LobbyLink.lobby.play()

## Everything that follows the lobby: which modes you can pick, what DEPLOY does, the status
## line and the found-match takeover.
func _refresh() -> void:
	var s: Dictionary = LobbyLink.state
	var online := not s.is_empty()
	var leading := LobbyLink.leading()
	# the party's mode is the leader's; members follow it (Practice stays theirs to pick)
	if online and not leading and mode != Mode.PRACTICE:
		mode = s.mode
	for i in _modes.get_child_count():
		var b: FrameButton = _modes.get_child(i)
		b.disabled = i != Mode.PRACTICE and not (online and leading)
		b.button_pressed = i == mode

	var searching: float = s.get("searching", -1.0)
	var cooldown: float = s.get("cooldown", -1.0)
	var label := "Deploy  ›"
	var can := true
	if mode != Mode.PRACTICE:
		if s.get("can_rejoin", false):
			label = "Rejoin match  ›"
		elif searching >= 0.0:
			label = "Cancel"
			can = leading
		elif not online or not leading or cooldown > 0.0:
			can = false
	_deploy.text = label.to_upper()
	_deploy.disabled = not can
	_deploy.queue_redraw()

	if _notice_left <= 0.0:
		_set_status(_status_text(s, online, leading, searching, cooldown))
	_ready_check(s.get("ready", {}))
	if stage:
		stage.set_squad(s.get("members", [1]).size())

func _status_text(s: Dictionary, online: bool, leading: bool, searching: float, cooldown: float) -> Array:
	if mode == Mode.PRACTICE:
		return ["", Tokens.TEXT_DIM]
	if not online:
		return ["Can't reach the lobby. Practice still works.", Tokens.TEXT_DIM]
	if searching >= 0.0:
		return ["Searching · %s" % _clock(searching), Tokens.YOU]
	if cooldown > 0.0:
		return ["Queue locked · %s" % _clock(cooldown), Tokens.ALERT]
	if not leading:
		return ["Your leader starts the match", Tokens.TEXT_DIM]
	return ["", Tokens.TEXT_DIM]

func _set_status(spec: Array) -> void:
	_status.text = spec[0]
	_status.add_theme_color_override("font_color", spec[1])
	_status.visible = spec[0] != ""

## The lobby's messages show in the status line for a few seconds.
func _show_notice(text: String) -> void:
	_notice_left = 4.0
	_set_status([text, Tokens.TEXT])
	Motion.glitch(_status)

func _process(delta: float) -> void:
	if _notice_left > 0.0:
		_notice_left -= delta
		if _notice_left <= 0.0:
			_refresh()

## A match found: the takeover with Accept and Decline, then who's in while you wait.
func _ready_check(r: Dictionary) -> void:
	if r.is_empty():
		_takeover.close()
		return
	var line := "Confirm in %d · %d / %d in" % [ceili(r.left), r.accepted, r.total]
	if r.you:
		line = "You're in · waiting for %d · %d" % [r.total - r.accepted, ceili(r.left)]
	if not _takeover.visible:
		_takeover.open("Operation ready", line, [
			{"text": "Accept", "primary": true, "call": func(): LobbyLink.lobby.accept_match()},
			{"text": "Decline", "call": func(): LobbyLink.lobby.decline_match()},
		])
	else:
		_takeover.set_line(line)
	_takeover.set_buttons_visible(not r.you)

func _clock(seconds: float) -> String:
	var t := int(seconds)
	return "%d:%02d" % [t / 60, t % 60]

func _apply_side() -> void:
	var s: Dictionary = SIDES[side]
	_side_label.text = s.label
	_side_label.add_theme_color_override("font_color", s.color)
	_rule.color = s.color
	_line.text = s.line
	_squad.side_color = s.color
	_squad.refresh()
	if stage:
		stage.set_side(s.color)

func _unhandled_key_input(event: InputEvent) -> void:
	if visible and event.pressed and not event.echo and event.physical_keycode == KEY_F:
		side = "bureau" if side == "strays" else "strays"
		_apply_side()
		Motion.glitch(_title)

## Arrival: the title card, then the squad, then the bottom, one stagger apart.
func enter() -> void:
	Motion.enter_all([_title, _squad, _bottom])
