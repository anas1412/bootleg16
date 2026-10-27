## A player's picture over the game: a person's is a tile in their own colour with their
## initial (their Steam picture once Steam is in this client), a bot's is the B16 hexagon
## marked BOT. Greyed when they're down, framed in amber when it's you. Under it, a strip:
## a teammate's health, or for an enemy only whether they're alive.
class_name HudAvatar
extends Control

const STRIP := 4.0
const STRIP_GAP := 3.0

var player_name := ""
var bot := false
var alive := true
var you := false
## 0 to 100 for you and teammates; -1 for an enemy (only alive or not shows).
var health := 100
var side := Tokens.STRAYS
## No strip (the voice rows use the picture alone).
var bare := false

func _init(px: float, no_strip := false) -> void:
	bare = no_strip
	custom_minimum_size = Vector2(px, px if bare else px + STRIP_GAP + STRIP)
	mouse_filter = Control.MOUSE_FILTER_IGNORE

## Sets everything at once and redraws only if something changed.
func show_player(p: Dictionary, side_color: Color) -> void:
	var h: int = p.get("health", -1)
	if p.name == player_name and p.bot == bot and p.alive == alive and p.you == you and h == health and side_color == side:
		return
	player_name = p.name
	bot = p.bot
	alive = p.alive
	you = p.you
	health = h
	side = side_color
	queue_redraw()

func _draw() -> void:
	var s := custom_minimum_size.x
	var pic := Rect2(0, 0, s, s)
	draw_rect(Rect2(pic.position + Vector2(0, 1), pic.size), Color(Tokens.BG, 0.7))
	if bot:
		draw_rect(pic, Tokens.PANEL)
		var c := pic.get_center()
		var r := s * 0.3
		var pts := PackedVector2Array()
		for k in 7:
			var a := PI / 6.0 + k * PI / 3.0
			pts.append(c + Vector2(cos(a), sin(a)) * r)
		draw_polyline(pts, Tokens.TEXT_OFF, 2.0)
		_text("BOT", UiTheme.font("label"), roundi(s * 0.2), Tokens.TEXT_DIM, c)
	else:
		# their own colour, from their name, so the same person always looks the same
		var hue := float(abs(player_name.hash()) % 360) / 360.0
		draw_rect(pic, Color.from_hsv(hue, 0.45, 0.32))
		draw_rect(Rect2(0, s * 0.55, s, s * 0.45), Color.from_hsv(hue, 0.5, 0.22))
		_text(player_name.substr(0, 1).to_upper(), UiTheme.font("tab"), roundi(s * 0.5), Color.from_hsv(hue, 0.25, 0.92), pic.get_center())
	if not alive:
		draw_rect(pic, Color(Tokens.BG, 0.62))
	if you:
		draw_rect(pic.grow(-1.0), Tokens.YOU, false, 2.0)
	if bare:
		return
	var strip := Rect2(0, s + STRIP_GAP, s, STRIP)
	draw_rect(strip, Color(Tokens.LINE, 0.9))
	if alive:
		var w := s if health < 0 else s * clampf(health / 100.0, 0.0, 1.0)
		draw_rect(Rect2(strip.position, Vector2(w, STRIP)), Tokens.ALERT if health >= 0 and health <= 25 else side)

func _text(t: String, font: Font, px: int, color: Color, centre: Vector2) -> void:
	var w := font.get_string_size(t, HORIZONTAL_ALIGNMENT_LEFT, -1, px).x
	var base := centre.y + (font.get_ascent(px) - font.get_descent(px)) / 2.0
	draw_string(font, Vector2(centre.x - w / 2.0, base), t, HORIZONTAL_ALIGNMENT_LEFT, -1, px, color)
