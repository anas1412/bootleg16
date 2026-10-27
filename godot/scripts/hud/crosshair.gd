## The crosshair and the hit marker. The gap is the real spread of your next shot
## (`Session.spread`), seen at this screen's size, as CS 1.6 draws it; none with the bomb
## out, nor with a sniper rifle unscoped; down a scope, the scope's black ring and lines. A hit
## shows a short X, red when it killed.
extends Control

const LENGTH := 10.0
const THICK := 2.0
const SNIPERS := ["scout", "awp"]
## A scope zooms below this vertical field of view (degrees).
const ZOOMED_FOV := 60.0
const HIT_SECONDS := 0.3

var spread := 0.0
var weapon := ""
var alive := false
var zoomed := false
var _hit := 0.0
var _hit_color := Tokens.TEXT
var _hit_size := 1.0

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func on_event(e: Dictionary) -> void:
	match e.type:
		"hit":
			_mark(Tokens.TEXT, e.head)
		"kill":
			if e.by_you:
				_mark(Tokens.ALERT, e.headshot)

func _mark(color: Color, head: bool) -> void:
	_hit = 1.0
	_hit_color = color
	_hit_size = 1.3 if head else 1.0

func _process(delta: float) -> void:
	_hit = maxf(_hit - delta / HIT_SECONDS, 0.0)
	queue_redraw()

func _draw() -> void:
	if not alive:
		return
	var c := (size / 2.0).round()
	if zoomed and weapon in SNIPERS:
		_scope(c)
	elif weapon != "c4" and not weapon in SNIPERS:
		# spread is the tangent of the cone; the view is 90 wide at 4:3 (half-height 0.75)
		var gap := clampf(spread * (size.y / 2.0) / 0.75, 3.0, size.y / 4.0)
		for d in [Vector2.RIGHT, Vector2.LEFT, Vector2.DOWN, Vector2.UP]:
			var from: Vector2 = c + d * gap
			var to: Vector2 = c + d * (gap + LENGTH)
			# a dark edge so it reads on bright walls
			draw_line(from - d, to + d, Color(Tokens.BG, 0.7), THICK + 2.0)
			draw_line(from, to, Tokens.TEXT, THICK)
	if _hit > 0.0:
		var col := Color(_hit_color, _hit)
		for d in [Vector2(1, 1), Vector2(1, -1), Vector2(-1, 1), Vector2(-1, -1)]:
			draw_line(c + d * 9.0 * _hit_size, c + d * 15.0 * _hit_size, col, THICK)

## The scope: black outside a circle, fine lines across.
func _scope(c: Vector2) -> void:
	var r := size.y * 0.46
	var ring := maxf(size.x, size.y)
	draw_arc(c, r + ring / 2.0, 0.0, TAU, 96, Color.BLACK, ring)
	draw_line(Vector2(c.x - r, c.y), Vector2(c.x + r, c.y), Color.BLACK, 1.0)
	draw_line(Vector2(c.x, c.y - r), Vector2(c.x, c.y + r), Color.BLACK, 1.0)
