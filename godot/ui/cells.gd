## A value out of 100 as a row of cells (health, armor): a cell per 100 / `count`. A hit's
## cells stay lit a moment as a lighter trail, then drain, so a hit reads as a loss you see.
## Red when `low` says so.
class_name Cells
extends Control

const GAP := 3.0
## Seconds the trail waits before draining, and how fast it drains (points a second).
const HOLD := 0.35
const DRAIN := 120.0

var count := 10
var value := 100.0:
	set(v):
		if v < value:
			_trail = maxf(_trail, value)
			_since = 0.0
		value = v
		queue_redraw()
var low := false
## Lit cells' colour (dim for armor).
var color := Tokens.TEXT
var _trail := 0.0
var _since := 0.0

func _init(cells := 10, cell_color := Tokens.TEXT) -> void:
	count = cells
	color = cell_color
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func _process(delta: float) -> void:
	if _trail <= value:
		return
	_since += delta
	if _since > HOLD:
		_trail = maxf(_trail - DRAIN * delta, value)
	queue_redraw()

func _draw() -> void:
	var w := (size.x - GAP * (count - 1)) / count
	var step := 100.0 / count
	var lit := Tokens.ALERT if low else color
	for i in count:
		var r := Rect2(i * (w + GAP), 0, w, size.y)
		var lo := i * step
		# a dark edge under each cell so the row reads on a bright wall
		draw_rect(Rect2(r.position + Vector2(0, 1), r.size), Color(Tokens.BG, 0.6))
		draw_rect(r, Color(Tokens.TEXT, 0.16))
		if _trail > lo:
			draw_rect(Rect2(r.position, Vector2(w * clampf((_trail - lo) / step, 0, 1), size.y)), Color(lit, 0.42))
		if value > lo:
			draw_rect(Rect2(r.position, Vector2(w * clampf((value - lo) / step, 0, 1), size.y)), lit)
