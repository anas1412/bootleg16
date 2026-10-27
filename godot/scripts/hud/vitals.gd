## You, bottom left: your money (what you earn tunes in beside it), the buy key while you may
## buy (in your buy zone, in buy time), your health as a number and as cells that show a
## hit's loss, and your armor with the suit icon (with a helmet when you have one). Red when
## your health is 25 or less.
extends TiltPanel

const SIZE := Vector2(360, 190)
## Seconds the reward stays beside your money.
const REWARD := 2.5

var health_row: Control
var _money: Label
var _reward: Label
var _buy := HBoxContainer.new()
var _buy_left: Label
var _health: Label
var _suit: TextureRect
var _armor: Label
var _cells := Cells.new(10)
var _armor_cells := Cells.new(10, Color(Tokens.TEXT, 0.6))
var _shown_money := -1
var _helmet := false
var _reward_tween: Tween

func _init() -> void:
	super(SIZE, 1.0)
	var col := VBoxContainer.new()
	col.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	col.alignment = BoxContainer.ALIGNMENT_END
	col.add_theme_constant_override("separation", Tokens.SPACE_S)
	content.add_child(col)

	var money := HBoxContainer.new()
	money.add_theme_constant_override("separation", Tokens.SPACE_M)
	_money = HudText.label("hud_small", "$0")
	_reward = HudText.label("hud_small", "", Tokens.YOU)
	_reward.modulate.a = 0.0
	money.add_child(_money)
	money.add_child(_reward)
	col.add_child(money)

	_buy.add_theme_constant_override("separation", Tokens.SPACE_S)
	var key := HudText.label("label", "B")
	var frame := StyleBoxFlat.new()
	frame.draw_center = false
	frame.set_border_width_all(Tokens.STROKE_HAIR)
	frame.border_color = Color(Tokens.TEXT, 0.75)
	frame.content_margin_left = Tokens.SPACE_S
	frame.content_margin_right = Tokens.SPACE_S
	key.add_theme_stylebox_override("normal", frame)
	_buy_left = HudText.label("label", "")
	for c in [HudText.icon("buyzone", 22, Tokens.YOU), key, HudText.label("tab", "Buy"), _buy_left]:
		c.size_flags_vertical = Control.SIZE_SHRINK_CENTER
		_buy.add_child(c)
	col.add_child(_buy)

	health_row = HBoxContainer.new()
	health_row.add_theme_constant_override("separation", Tokens.SPACE_S)
	_health = HudText.label("hud_value", "100")
	_suit = HudText.icon("suit_full", 24, Color(Tokens.TEXT, 0.75))
	_armor = HudText.label("hud_value", "0", Color(Tokens.TEXT, 0.75))
	_armor.add_theme_font_size_override("font_size", 26)
	var gap := Control.new()
	gap.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	for c in [_health, gap, _suit, _armor]:
		c.size_flags_vertical = Control.SIZE_SHRINK_END
		health_row.add_child(c)
	col.add_child(health_row)
	_cells.custom_minimum_size.y = 12
	_armor_cells.custom_minimum_size.y = 4
	col.add_child(_cells)
	col.add_child(_armor_cells)

func refresh(hud: Dictionary) -> void:
	var money: int = hud.get("money", 0)
	if money != _shown_money:
		if _shown_money >= 0 and money > _shown_money:
			_earned(money - _shown_money)
		_shown_money = money
		_money.text = HudText.money(money)
	_buy.visible = hud.get("can_buy", false)
	if _buy.visible:
		var left: float = hud.get("buy_left", -1.0)
		_buy_left.text = ("%d s left" % ceili(left)) if left >= 0.0 else ""
	var hp: int = hud.get("health", 0)
	_health.text = str(hp)
	_health.add_theme_color_override("font_color", Tokens.ALERT if hp <= 25 else Tokens.TEXT)
	_cells.low = hp <= 25
	_cells.value = hp
	var armor: int = hud.get("armor", 0)
	_armor.text = str(armor)
	_armor_cells.value = armor
	var helmet: bool = hud.get("helmet", false)
	if helmet != _helmet:
		_helmet = helmet
		HudText.set_icon(_suit, "suithelmet_full" if helmet else "suit_full")
	for c in [_suit, _armor]:
		c.modulate.a = 1.0 if armor > 0 else 0.4

## What you just earned, beside your money for a moment.
func _earned(amount: int) -> void:
	_reward.text = "+" + HudText.money(amount)
	if _reward_tween:
		_reward_tween.kill()
	_reward.modulate.a = 1.0
	Motion.glitch(_reward)
	_reward_tween = create_tween()
	_reward_tween.tween_interval(REWARD)
	_reward_tween.tween_property(_reward, "modulate:a", 0.0, Tokens.EXIT)
