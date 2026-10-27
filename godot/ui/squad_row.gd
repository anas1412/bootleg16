## One squad member: name left, status right, on a panel with a 4 wide bar in their side's
## colour. Your own name is amber. An empty slot is a resting frame that says what it's for.
class_name SquadRow
extends PanelContainer

func _init(member := "", status := "", side_color := Tokens.STRAYS, is_you := false, empty := false) -> void:
	var box := StyleBoxFlat.new()
	box.bg_color = Color.TRANSPARENT if empty else Tokens.PANEL
	box.border_color = Tokens.LINE if empty else side_color
	box.border_width_left = Tokens.STROKE_FRAME
	if empty:
		box.border_width_top = Tokens.STROKE_HAIR
		box.border_width_right = Tokens.STROKE_HAIR
		box.border_width_bottom = Tokens.STROKE_HAIR
	box.content_margin_left = Tokens.SPACE_M
	box.content_margin_right = Tokens.SPACE_M
	box.content_margin_top = Tokens.SPACE_S
	box.content_margin_bottom = Tokens.SPACE_S
	add_theme_stylebox_override("panel", box)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", Tokens.SPACE_M)
	var who := UiTheme.label("body", member, Tokens.TEXT_OFF if empty else (Tokens.YOU if is_you else Tokens.TEXT))
	who.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var st := UiTheme.label("label", status, Tokens.TEXT_DIM)
	st.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	row.add_child(who)
	row.add_child(st)
	add_child(row)
