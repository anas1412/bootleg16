## The kill feed, top right: killer, the weapon's icon, the headshot icon, victim, newest at
## the bottom, each row for a few seconds. A row's outer edge is the killer's side; names in
## their side's colour, yours in amber, and a row you're in is lit amber. Rows tune in.
extends TiltPanel

const SIZE := Vector2(560, 240)
const ROWS := 5
const SECONDS := 6.0
const ICON := 28.0
const ROW := 36.0

var _list := VBoxContainer.new()

func _init() -> void:
	super(SIZE, -1.0)
	_list.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	_list.alignment = BoxContainer.ALIGNMENT_BEGIN
	_list.add_theme_constant_override("separation", Tokens.SPACE_XS)
	content.add_child(_list)

func add(e: Dictionary) -> void:
	var mine: bool = e.by_you or e.of_you
	var row := PanelContainer.new()
	row.size_flags_horizontal = Control.SIZE_SHRINK_END
	row.custom_minimum_size.y = ROW
	var box := StyleBoxFlat.new()
	box.bg_color = Color(Tokens.YOU, 0.18) if mine else Color(Tokens.BG, 0.66)
	box.border_width_right = Tokens.STROKE_FRAME
	box.border_color = _side(e.killer_team if e.killer != "" else e.victim_team)
	box.content_margin_left = Tokens.SPACE_M
	box.content_margin_right = Tokens.SPACE_M
	row.add_theme_stylebox_override("panel", box)
	var line := HBoxContainer.new()
	line.add_theme_constant_override("separation", Tokens.SPACE_S)
	if e.killer != "":
		line.add_child(_name(e.killer, e.killer_team, e.by_you))
	line.add_child(HudText.icon(Weapons.icon(e.weapon), ICON))
	if e.headshot:
		line.add_child(HudText.icon("d_headshot", ICON))
	line.add_child(_name(e.victim, e.victim_team, e.of_you))
	for c in line.get_children():
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(line)
	_list.add_child(row)
	Motion.glitch.call_deferred(row)
	while _list.get_child_count() > ROWS:
		var old := _list.get_child(0)
		_list.remove_child(old)
		old.queue_free()
	get_tree().create_timer(SECONDS).timeout.connect(_expire.bind(row))

func clear() -> void:
	for c in _list.get_children():
		c.queue_free()

func _expire(row) -> void:  # (untyped: the row may be gone by now)
	if is_instance_valid(row) and not row.is_queued_for_deletion():
		Motion.exit(row).tween_callback(row.queue_free)

func _name(n: String, team: String, you: bool) -> Label:
	return HudText.label("hud_name", n, Tokens.YOU if you else _side(team))

static func _side(team: String) -> Color:
	return Tokens.BUREAU if team == "ct" else Tokens.STRAYS
