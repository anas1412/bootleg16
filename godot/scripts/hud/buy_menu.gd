## The buy menu (B), the Armory: your side's guns and gear in four columns of framed cards,
## each with the game's icon, its key and its price now. The card you point at (or pick by
## keys, CS 1.6 style: the column, then the item) is amber, and the panel under the columns
## says what buying it leaves you with and what it replaces. What you carry is listed, and
## Undo buys (Backspace) sells back this round's buys you haven't used. It opens only in your
## buy zone during buy time (a notice says why not), closes itself when buy time ends, and you
## keep walking while it's open, as in CS2. B or Esc closes it.
extends CanvasLayer

signal opened(on: bool)
signal notice(text: String)

const CATEGORIES := ["Pistols", "SMGs", "Rifles", "Gear"]
const KINDS := {"Pistols": "Pistol", "SMGs": "SMG", "Rifles": "Rifle", "Gear": "Gear"}
const GEAR_ICONS := {"kevlar": "suit_full", "kevlar_helmet": "suithelmet_full", "defuse_kit": "defuser"}
const CARD_H := 56.0
const ICON_W := 84.0

var session: Session
## This frame's `Session.hud`, from main.gd.
var hud := {}
var _root := Control.new()
var _blur := ColorRect.new()
var _eyebrow: Label
var _money: Label
var _time: Label
var _columns: Array[VBoxContainer] = []
var _cards := {}  # key -> {button, price, item}
var _detail := VBoxContainer.new()
var _carry := HBoxContainer.new()
var _undo: FrameButton
var _undo_amount: Label
var _team := ""
var _picked := ""
var _column := -1
var _spent := 0  # bought this round through this menu (what undo can give back, at most)
var _round := -1
var _leaving: Tween

func _ready() -> void:
	layer = 5
	_root.theme = UiTheme.theme()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.visible = false
	add_child(_root)
	_blur.material = ShaderMaterial.new()
	_blur.material.shader = preload("res://ui/blur.gdshader")
	_blur.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.add_child(_blur)
	_build_head()
	for i in CATEGORIES.size():
		var col := VBoxContainer.new()
		col.add_theme_constant_override("separation", Tokens.SPACE_S)
		var head := HBoxContainer.new()
		head.add_theme_constant_override("separation", Tokens.SPACE_M)
		head.add_child(_key(str(i + 1)))
		head.add_child(UiTheme.label("label", CATEGORIES[i], Tokens.TEXT_DIM))
		col.add_child(head)
		Grid.place(col, i * 3, 3, 236)
		_root.add_child(col)
		_columns.append(col)
	_detail.add_theme_constant_override("separation", Tokens.SPACE_S)
	Grid.place(_detail, 0, 6, 640)
	_root.add_child(_detail)
	_build_right()
	var hint := UiTheme.label("label", "Number keys: the column, then the item · Backspace undoes · Click works too · B or Esc closes · You keep walking", Tokens.TEXT_DIM)
	Grid.place(hint, 0, 12, Grid.bottom() - Tokens.TYPE.label.line)
	_root.add_child(hint)

func _build_head() -> void:
	var left := VBoxContainer.new()
	left.add_theme_constant_override("separation", 0)
	_eyebrow = UiTheme.label("label")
	left.add_child(_eyebrow)
	left.add_child(UiTheme.label("page", "Armory"))
	left.add_child(UiTheme.label("tab_kana", "アーマリー", Tokens.TEXT_DIM))
	Grid.place(left, 0, 6, Tokens.MARGIN_Y)
	_root.add_child(left)
	var right := HBoxContainer.new()
	right.alignment = BoxContainer.ALIGNMENT_END
	right.add_theme_constant_override("separation", Tokens.SPACE_XXL)
	_money = UiTheme.label("hud_timer")
	_time = UiTheme.label("hud_timer")
	for pair in [["Money", _money], ["Buy time", _time]]:
		var v := VBoxContainer.new()
		v.add_theme_constant_override("separation", Tokens.SPACE_XS)
		var l := UiTheme.label("label", pair[0], Tokens.TEXT_DIM)
		l.horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		pair[1].horizontal_alignment = HORIZONTAL_ALIGNMENT_RIGHT
		v.add_child(l)
		v.add_child(pair[1])
		right.add_child(v)
	Grid.place(right, 6, 6, Tokens.MARGIN_Y + Tokens.SPACE_L)
	_root.add_child(right)

func _build_right() -> void:
	var col := VBoxContainer.new()
	col.add_theme_constant_override("separation", Tokens.SPACE_M)
	col.add_child(UiTheme.label("label", "Carrying", Tokens.TEXT_DIM))
	_carry.add_theme_constant_override("separation", Tokens.SPACE_S)
	col.add_child(_carry)
	var gap := Control.new()
	gap.custom_minimum_size.y = Tokens.SPACE_M
	col.add_child(gap)
	_undo = FrameButton.new("Undo buys")
	_undo.compact = true
	_undo.focus_mode = Control.FOCUS_NONE
	_undo.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_undo.pressed.connect(_undo_buys)
	var row := HBoxContainer.new()
	row.add_theme_constant_override("separation", Tokens.SPACE_M)
	_undo_amount = UiTheme.label("hud_small", "", Tokens.YOU)
	for c in [_key("Bksp"), _undo, _undo_amount]:
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		row.add_child(c)
	col.add_child(row)
	col.add_child(UiTheme.label("body", "Sells back what you bought this round and haven't used yet, while buy time lasts.", Tokens.TEXT_DIM))
	Grid.place(col, 6, 6, 640)
	_root.add_child(col)

func is_open() -> bool:
	return _root.visible

func _unhandled_input(event: InputEvent) -> void:
	if session == null:
		return
	if is_open():
		if event.is_action_pressed("ui_cancel") or event.is_action_pressed("buy"):
			close()
		elif event.is_action_pressed("undo_buys"):
			_undo_buys()
		elif event is InputEventKey and event.pressed and not event.echo and event.keycode >= KEY_1 and event.keycode <= KEY_5:
			_number(event.keycode - KEY_1)
		else:
			return
		get_viewport().set_input_as_handled()
	elif event.is_action_pressed("buy") and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		if hud.get("can_buy", false):
			open()
		elif hud.get("alive", false):
			notice.emit("Buy time is over" if hud.get("in_zone", false) else "Not in a buy zone")
		get_viewport().set_input_as_handled()

## A number key: the first picks a column, the second buys that column's item.
func _number(n: int) -> void:
	if _column < 0:
		if n < CATEGORIES.size():
			_column = n
			Motion.glitch(_columns[n].get_child(0))
		return
	var cards := _columns[_column].get_children().slice(1)
	_column = -1
	if n < cards.size():
		var key: String = cards[n].get_meta("key")
		_pick(key)
		_buy(key)

func open() -> void:
	if _leaving and _leaving.is_valid():
		_leaving.kill()
	_build()
	_root.visible = true
	_root.modulate.a = 1.0
	_column = -1
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	opened.emit(true)
	_blur.material.set_shader_parameter("amount", 0.0)
	Motion.tween(_blur).tween_property(_blur.material, "shader_parameter/amount", 0.7, Tokens.ENTER)
	Motion.enter_all(_columns)
	_refresh()

func close() -> void:
	if not is_open():
		return
	opened.emit(false)
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED
	_leaving = Motion.exit(_root)
	_leaving.tween_callback(func(): _root.visible = false; _root.modulate.a = 1.0)

## This frame's `Session.hud`: money spent through the menu, a new round, and the open
## menu's prices.
func on_hud(h: Dictionary) -> void:
	hud = h
	var round_n: int = h.get("round", 0)
	if round_n != _round:
		_round = round_n
		_spent = 0
	if is_open():
		_refresh()

## Your side's cards (the lists differ by side).
func _build() -> void:
	var team: String = hud.get("team", "")
	if team == _team and not _cards.is_empty():
		return
	_team = team
	_cards.clear()
	for col in _columns:
		for c in col.get_children().slice(1):
			c.free()
	var counts := [0, 0, 0, 0]
	for item in session.shop():
		var c := CATEGORIES.find(item.category)
		counts[c] += 1
		_columns[c].add_child(_card(item, counts[c]))

func _card(item: Dictionary, n: int) -> FrameButton:
	var card := FrameButton.new()
	card.compact = true
	card.toggle_mode = true
	card.focus_mode = Control.FOCUS_NONE
	card.custom_minimum_size = Vector2(Grid.w(3), CARD_H)
	card.set_meta("key", item.key)
	var row := HBoxContainer.new()
	row.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT, Control.PRESET_MODE_KEEP_SIZE, Tokens.SPACE_M)
	row.add_theme_constant_override("separation", Tokens.SPACE_M)
	row.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var icon_box := CenterContainer.new()
	icon_box.custom_minimum_size.x = ICON_W
	var icon_name: String = GEAR_ICONS.get(item.key, Weapons.icon(item.key))
	var icon := HudText.icon(icon_name, 26, Tokens.TEXT, ICON_W)
	icon_box.add_child(icon)
	var title := UiTheme.label("body", item.name)
	title.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	var price := UiTheme.label("hud_small", "")
	for c in [_key(str(n)), icon_box, title, price]:
		c.mouse_filter = Control.MOUSE_FILTER_IGNORE
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		row.add_child(c)
	card.add_child(row)
	var key: String = item.key
	card.pressed.connect(func(): _pick(key); _buy(key))
	card.mouse_entered.connect(func(): _pick(key))
	_cards[key] = {"button": card, "price": price, "title": title, "icon": icon, "item": item}
	return card

func _key(text: String) -> Label:
	var k := UiTheme.label("hud_tag", text)
	var frame := StyleBoxFlat.new()
	frame.draw_center = false
	frame.set_border_width_all(Tokens.STROKE_HAIR)
	frame.border_color = Tokens.LINE
	frame.content_margin_left = Tokens.SPACE_S
	frame.content_margin_right = Tokens.SPACE_S
	frame.content_margin_top = Tokens.SPACE_XS
	frame.content_margin_bottom = Tokens.SPACE_XS
	k.add_theme_stylebox_override("normal", frame)
	return k

func _buy(key: String) -> void:
	var c: Dictionary = _cards.get(key, {})
	if c.is_empty() or c.button.disabled:
		return
	_spent += int(c.item.cost)
	session.buy(key)
	Motion.glitch(c.button)

func _undo_buys() -> void:
	if _spent <= 0:
		return
	session.undo_buys()
	_spent = 0
	Motion.glitch(_undo)

## The card you point at, amber, and the panel saying what buying it does.
func _pick(key: String) -> void:
	_picked = key
	for k in _cards:
		_cards[k].button.set_pressed_no_signal(k == key)
		_cards[k].button.queue_redraw()
	_show_detail()

func _show_detail() -> void:
	for c in _detail.get_children():
		c.queue_free()
	var c: Dictionary = _cards.get(_picked, {})
	if c.is_empty():
		return
	var item: Dictionary = (session.shop().filter(func(i): return i.key == _picked) + [c.item])[0]
	var col := CATEGORIES.find(item.category)
	var n: int = _columns[col].get_children().slice(1).find(c.button) + 1
	_detail.add_child(UiTheme.label("label", "%s · keys %d then %d" % [KINDS[item.category], col + 1, n], Tokens.YOU))
	var title := HBoxContainer.new()
	title.add_theme_constant_override("separation", Tokens.SPACE_XL)
	var icon_name: String = GEAR_ICONS.get(item.key, Weapons.icon(item.key))
	title.add_child(HudText.icon(icon_name, 72, Tokens.YOU, 260))
	title.add_child(UiTheme.label("alert", item.name, Tokens.TEXT))
	for t in title.get_children():
		t.size_flags_vertical = Control.SIZE_SHRINK_CENTER
	_detail.add_child(title)
	var rule := ColorRect.new()
	rule.color = Tokens.YOU
	rule.custom_minimum_size = Vector2(200, Tokens.STROKE_FRAME)
	rule.size_flags_horizontal = Control.SIZE_SHRINK_BEGIN
	_detail.add_child(rule)
	var rows := GridContainer.new()
	rows.columns = 2
	rows.add_theme_constant_override("h_separation", Tokens.SPACE_XL)
	rows.add_theme_constant_override("v_separation", Tokens.SPACE_S)
	var money: int = hud.get("money", 0)
	var lines := [["Price", HudText.money(item.price)]]
	if item.cost > money:
		lines.append(["Short by", HudText.money(item.cost - money)])
	elif item.cost >= 0:
		lines.append(["After buying", HudText.money(money - item.cost)])
	if item.clip > 0:
		lines.append(["Magazine", "%d / %d" % [item.clip, item.reserve]])
	if item.category != "Gear":
		var slot: String = hud.get("secondary" if item.category == "Pistols" else "primary", "")
		lines.append(["Replaces", ("Your %s, dropped" % Weapons.shown(slot)) if slot != "" else "Nothing, that slot is empty"])
	for l in lines:
		rows.add_child(UiTheme.label("label", l[0], Tokens.TEXT_DIM))
		rows.add_child(UiTheme.label("hud_small" if l[0] != "Replaces" else "body", l[1], Tokens.ALERT if l[0] == "Short by" else Tokens.TEXT))
	_detail.add_child(rows)

## Prices, what you can afford, money, buy time, what you carry and undo; closes when you
## can't buy any more.
func _refresh() -> void:
	if not hud.get("can_buy", false):
		close()
		return
	var money: int = hud.get("money", 0)
	_eyebrow.text = "%s · round %d" % ["Bureau" if _team == "ct" else "Strays", hud.get("round", 0)]
	_eyebrow.add_theme_color_override("font_color", Tokens.BUREAU if _team == "ct" else Tokens.STRAYS)
	_money.text = HudText.money(money)
	var left: float = hud.get("buy_left", -1.0)
	_time.text = ("0:%02d" % ceili(left)) if left >= 0.0 else ""
	for item in session.shop():
		if not _cards.has(item.key):
			continue
		var c: Dictionary = _cards[item.key]
		c.item = item
		var cost: int = item.cost
		var have := cost < 0
		var cant := not have and cost > money
		c.button.disabled = have or cant
		c.price.text = "HAVE IT" if have else HudText.money(cost)
		c.price.add_theme_color_override("font_color", Tokens.TEXT_OFF if have else (Tokens.ALERT if cant else Tokens.TEXT))
		c.title.add_theme_color_override("font_color", Tokens.TEXT_OFF if have else (Tokens.TEXT_DIM if cant else Tokens.TEXT))
		c.icon.modulate = Tokens.TEXT_OFF if have or cant else Tokens.TEXT
	_carrying()
	_undo.disabled = _spent <= 0
	_undo_amount.text = ("up to +" + HudText.money(_spent)) if _spent > 0 else ""

func _carrying() -> void:
	var names := ["knife"]
	for slot in ["secondary", "primary"]:
		if hud.get(slot, "") != "":
			names.append(hud[slot])
	var g: PackedInt32Array = hud.get("grenades", PackedInt32Array([0, 0, 0]))
	for i in 3:
		for k in g[i]:
			names.append(["hegrenade", "flashbang", "smokegrenade"][i])
	if hud.get("c4", false):
		names.append("c4")
	var key := ",".join(names)
	if _carry.get_meta("shown", "") == key:
		return
	_carry.set_meta("shown", key)
	for c in _carry.get_children():
		c.queue_free()
	for n in names:
		var chip := PanelContainer.new()
		var frame := StyleBoxFlat.new()
		frame.draw_center = false
		frame.set_border_width_all(Tokens.STROKE_HAIR)
		frame.border_color = Tokens.LINE
		frame.content_margin_left = Tokens.SPACE_M
		frame.content_margin_right = Tokens.SPACE_M
		frame.content_margin_top = Tokens.SPACE_XS
		frame.content_margin_bottom = Tokens.SPACE_XS
		chip.add_theme_stylebox_override("panel", frame)
		var row := HBoxContainer.new()
		row.add_theme_constant_override("separation", Tokens.SPACE_S)
		row.add_child(HudText.icon(Weapons.icon(n), 18, Tokens.TEXT, 60))
		row.add_child(UiTheme.label("body", Weapons.shown(n)))
		for r in row.get_children():
			r.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		chip.add_child(row)
		_carry.add_child(chip)
