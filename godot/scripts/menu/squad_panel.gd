## Your squad, as the lobby says it is: everyone in your party (in a custom game with their
## team; the leader clicks someone to move them), invites to other parties, your party's code
## to give a friend (click to copy) and a field to type a friend's code. Offline: just you.
extends VBoxContainer

const MAX := [5, 10]  # casual, custom

var side_color := Tokens.STRAYS

var _code_field := LineEdit.new()
var _shown = null

func _ready() -> void:
	add_theme_constant_override("separation", Tokens.SPACE_S)
	LobbyLink.state_changed.connect(func(_s): refresh())
	refresh()

## Rebuilds only when something shown here changed: the queue timer ticks the lobby state
## every second, and a rebuild would wipe a code you're typing.
func refresh(force := false) -> void:
	var s: Dictionary = LobbyLink.state
	var shown := [] if s.is_empty() else [s.members, s.invites.map(func(i): return i.code), s.code, s.mode, s.leader, s.you, side_color]
	if shown == _shown and not force:
		return
	_shown = shown
	for c in get_children():
		c.queue_free()
	if s.is_empty():
		add_child(UiTheme.label("label", "Your squad · 1 / 5", Tokens.TEXT_DIM))
		add_child(SquadRow.new(LobbyLink.player_name(), "You", side_color, true))
		add_child(SquadRow.new("Can't reach the lobby", "Offline", side_color, false, true))
		return
	var members: Array = s.members
	var most: int = MAX[s.mode]
	add_child(UiTheme.label("label", "Your squad · %d / %d" % [members.size(), most], Tokens.TEXT_DIM))
	for m in members:
		add_child(_member_row(m, s))
	for inv in s.invites:
		add_child(_invite_row(inv))
	if members.size() < most:
		add_child(_code_row(s.code))
		add_child(_join_row())

func _member_row(m: Dictionary, s: Dictionary) -> Control:
	var you: bool = m.id == s.you
	var status := "You" if you else ("Leader" if m.id == s.leader else "")
	if s.mode == 1:
		status = ("Team %s" % ["A", "B"][m.group]) + (" · you" if you else "")
	var row := SquadRow.new(m.name, status, side_color, you)
	if s.mode == 1 and LobbyLink.leading():
		_clickable(row, func():
			LobbyLink.lobby.move_member(m.id, 1 - m.group)
			Motion.glitch(row))
	return row

func _invite_row(inv: Dictionary) -> Control:
	var row := SquadRow.new("%s invites you" % inv.from, "Join", Tokens.YOU)
	_clickable(row, func(): LobbyLink.lobby.join_party(inv.code))
	return row

## Your party's code: a friend types it to join you. Click to copy it.
func _code_row(code: String) -> Control:
	var row := SquadRow.new("Your code  %s" % code, "Copy", side_color, false, true)
	_clickable(row, func():
		DisplayServer.clipboard_set(code)
		LobbyLink.tell("Code %s copied. Send it to a friend." % code))
	return row

func _join_row() -> Control:
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", Tokens.SPACE_S)
	_code_field = LineEdit.new()
	_code_field.placeholder_text = "Friend's code"
	_code_field.max_length = 12
	_code_field.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var join := FrameButton.new("Join")
	join.compact = true
	var go := func():
		if _code_field.text.strip_edges() != "":
			LobbyLink.lobby.join_party(_code_field.text)
	join.pressed.connect(go)
	_code_field.text_submitted.connect(func(_t): go.call())
	row.add_child(_code_field)
	row.add_child(join)
	return row

## A row you can click: the pointer turns to a hand and the row lights under it.
func _clickable(row: Control, on_click: Callable) -> void:
	row.mouse_filter = Control.MOUSE_FILTER_STOP
	row.mouse_default_cursor_shape = Control.CURSOR_POINTING_HAND
	row.gui_input.connect(func(e):
		if e is InputEventMouseButton and e.pressed and e.button_index == MOUSE_BUTTON_LEFT:
			on_click.call())
	row.mouse_entered.connect(func(): Motion.tween(row).tween_property(row, "modulate", Color(1.15, 1.15, 1.15), Tokens.HOVER))
	row.mouse_exited.connect(func(): Motion.tween(row).tween_property(row, "modulate", Color.WHITE, Tokens.HOVER))
