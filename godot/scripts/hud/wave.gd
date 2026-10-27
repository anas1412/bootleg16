## A voice's waveform: bars that move with how loud it is (`level`, 0 to 1). Silent, they
## rest low.
class_name HudWave
extends Control

const BAR := 3.0
const GAP := 2.0

var level := 0.0
var color := Tokens.TEXT
var _bars := 10
var _seed := randf() * 10.0

func _init(bars := 10, bar_color := Tokens.TEXT, height := 20.0) -> void:
	_bars = bars
	color = bar_color
	custom_minimum_size = Vector2(bars * (BAR + GAP) - GAP, height)
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func _process(_delta: float) -> void:
	queue_redraw()

func _draw() -> void:
	var t := Time.get_ticks_msec() / 1000.0
	var h := size.y
	for i in _bars:
		# each bar its own wobble, scaled by the voice's level
		var wob := 0.5 + 0.5 * sin(t * 11.0 + i * 1.7 + _seed) * sin(t * 4.3 + i * 0.9)
		var bh := maxf(h * 0.15, h * clampf(level, 0.0, 1.0) * (0.35 + 0.65 * wob))
		var r := Rect2(i * (BAR + GAP), (h - bh) / 2.0, BAR, bh)
		draw_rect(Rect2(r.position + Vector2(0, 1), r.size), Color(Tokens.BG, 0.6))
		draw_rect(r, color)
