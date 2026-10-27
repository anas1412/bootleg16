## The alert: something just changed everything. One component in two colours, the same
## motion and timing: the hex field pulses once out from behind the score, and a card (a small
## line over the word) tunes in under it; both are gone after a couple of seconds.
## Amber: you're the last one alive. Red: the bomb is down.
extends Control

## Seconds the pulse runs and the card stays.
const SECONDS := 2.2
const HEX := 22.0
## The field's centre (behind the score) and how far it reaches.
const CENTRE := Vector2(960, 70)
const REACH := 520.0

var _t := -1.0
var _color := Tokens.YOU
var _card := VBoxContainer.new()
var _line: Label
var _word: Label

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_card.add_theme_constant_override("separation", Tokens.SPACE_S)
	_line = HudText.label("label")
	_word = HudText.label("alert")
	for l in [_line, _word]:
		l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		_card.add_child(l)
	add_child(_card)
	_card.visible = false

func last_alive(enemies: int) -> void:
	play(Tokens.YOU, "1 v %d" % enemies, "Last alive")

## The bomb is down: what your side does now.
func bomb_planted(your_side: String) -> void:
	play(Tokens.ALERT, "Defend it" if your_side == "t" else "Defuse it", "Bomb planted")

func play(color: Color, line: String, word: String) -> void:
	_color = color
	_line.text = line
	_word.text = word
	for l in [_line, _word]:
		l.add_theme_color_override("font_color", color)
	_card.visible = true
	_card.modulate.a = 1.0
	_card.reset_size()
	_card.position = Vector2((size.x - _card.size.x) / 2.0, 150)
	Motion.glitch.call_deferred(_card)
	_t = 0.0

func _process(delta: float) -> void:
	if _t < 0.0:
		return
	_t += delta
	if _t >= SECONDS:
		_t = -1.0
		Motion.exit(_card).tween_callback(func(): _card.visible = false)
	queue_redraw()

func _draw() -> void:
	if _t < 0.0:
		return
	var k := _t / SECONDS
	var w := sqrt(3.0) * HEX
	var rows := ceili(260.0 / (HEX * 1.5))
	var cols := ceili(REACH * 2.0 / w)
	for row in rows:
		for col in cols:
			var p := Vector2(CENTRE.x - REACH + col * w + (w / 2.0 if row % 2 else 0.0), row * HEX * 1.5)
			var d := p.distance_to(CENTRE) / REACH
			var fade := maxf(0.0, 1.0 - d) * (1.0 - k)
			if fade <= 0.0:
				continue
			# a ring of lit cells running outwards
			var lit := maxf(0.0, 1.0 - absf(d - k) * 5.0) * 0.5 * fade
			var pts := PackedVector2Array()
			for i in 7:
				var a := PI / 6.0 + i * PI / 3.0
				pts.append(p + Vector2(cos(a), sin(a)) * HEX * 0.9)
			if lit > 0.01:
				draw_colored_polygon(pts.slice(0, 6), Color(_color, lit))
			draw_polyline(pts, Color(_color, 0.22 * fade), 1.5)
