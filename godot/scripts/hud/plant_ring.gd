## While you plant or defuse: a ring of ticks round the crosshair fills in amber (it's your
## action), and under it what you're doing and the seconds you still have to hold.
extends Control

const TICKS := 40
const INNER := 44.0
const OUTER := 54.0
## CS: the bomb takes 3 seconds to plant (`sim::weapons::C4_PLANT_TIME`).
const PLANT_SECONDS := 3.0

var _value := -1.0
var _label: Label
var _left: Label
var _row := HBoxContainer.new()

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_row.add_theme_constant_override("separation", Tokens.SPACE_M)
	_label = HudText.label("label", "", Tokens.YOU)
	_left = HudText.label("hud_small", "")
	_row.add_child(_label)
	_row.add_child(_left)
	add_child(_row)
	visible = false

## From this frame's `Session.hud`.
func refresh(hud: Dictionary) -> void:
	var planting: float = hud.get("planting", -1.0)
	var was := visible
	if planting >= 0.0:
		_show("Planting", planting, (1.0 - planting) * PLANT_SECONDS)
	elif hud.get("you_defuse", false) and hud.get("defuse_total", 0.0) > 0.0:
		_show("Defusing", 1.0 - hud.defuse_left / hud.defuse_total, hud.defuse_left)
	else:
		visible = false
	if visible and not was:
		Motion.glitch.call_deferred(_row)

func _show(what: String, value: float, seconds: float) -> void:
	visible = true
	_value = clampf(value, 0.0, 1.0)
	_label.text = what
	_left.text = "%.1f s" % maxf(seconds, 0.0)
	_row.reset_size()
	_row.position = Vector2((size.x - _row.size.x) / 2.0, size.y / 2.0 + OUTER + Tokens.SPACE_L)
	queue_redraw()

func _draw() -> void:
	var c := size / 2.0
	for i in TICKS:
		var a := -PI / 2.0 + TAU * i / TICKS
		var d := Vector2(cos(a), sin(a))
		var on := float(i) / TICKS < _value
		draw_line(c + d * INNER, c + d * OUTER, Color(Tokens.BG, 0.6), 5.0)
		draw_line(c + d * INNER, c + d * OUTER, Tokens.YOU if on else Color(Tokens.TEXT, 0.25), 3.0)
