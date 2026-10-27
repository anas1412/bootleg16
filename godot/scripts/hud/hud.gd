## The HUD over the game. Places its pieces (each its own script in scripts/hud) and hands
## them what they show: main.gd reads `Session.hud` once a frame and passes it here and to
## the other parts (`on_hud`), and passes the game's events (`on_event`); the HUD reads the
## players, the voices and the spread itself. The groups by the screen's edges sit on the
## visor (`TiltPanel`): the top bar, the voices, the kill feed, you and your gun. Hidden
## until you're in the match.
extends CanvasLayer

signal leave

const EDGE := Tokens.SPACE_XL
const NOTICE_SECONDS := 1.6

var session: Session
var camera: Camera3D:
	set(c):
		camera = c
		_marker.camera = c

var _root := Control.new()
var _top := preload("res://scripts/hud/top_bar.gd").new()
var _voices := preload("res://scripts/hud/voice_rows.gd").new()
var _feed := preload("res://scripts/hud/kill_feed.gd").new()
var _vitals := preload("res://scripts/hud/vitals.gd").new()
var _gun := preload("res://scripts/hud/weapon.gd").new()
var _crosshair := preload("res://scripts/hud/crosshair.gd").new()
var _ring := preload("res://scripts/hud/plant_ring.gd").new()
var _marker := preload("res://scripts/hud/bomb_marker.gd").new()
var _alert := preload("res://scripts/hud/alert.gd").new()
var _result := preload("res://scripts/hud/result_card.gd").new()
var _uplink := preload("res://scripts/hud/uplink.gd").new()
var _interference := preload("res://scripts/hud/interference.gd").new()
var _notice: Label
var _notice_tween: Tween
var _last := {}
var _alive_us := 5

func _ready() -> void:
	layer = 1
	_root.theme = UiTheme.theme()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.mouse_filter = Control.MOUSE_FILTER_IGNORE
	add_child(_root)
	for c in [_marker, _crosshair, _ring, _alert, _result, _uplink]:
		_root.add_child(c)
	var w := Tokens.CANVAS.x
	var h := Tokens.CANVAS.y
	_place(_top, Vector2((w - _top.SIZE.x) / 2.0, 0))
	_place(_voices, Vector2(EDGE, EDGE))
	_place(_feed, Vector2(w - EDGE - _feed.SIZE.x, EDGE))
	_place(_vitals, Vector2(EDGE, h - EDGE - _vitals.SIZE.y))
	_place(_gun, Vector2(w - EDGE - _gun.SIZE.x, h - EDGE - _gun.SIZE.y))
	_result.leave.connect(func(): leave.emit())
	_notice = HudText.label("label")
	_notice.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	_notice.position = Vector2(0, h * 0.62)
	_notice.size = Vector2(w, Tokens.TYPE.label.line)
	_notice.modulate.a = 0.0
	_root.add_child(_notice)
	add_child(_interference)
	_interference.groups = [_top.content, _voices.content, _feed.content, _vitals.content, _gun.content]
	_interference.health = _vitals.health_row
	_root.visible = false

func _place(panel: Control, at: Vector2) -> void:
	_root.add_child(panel)
	panel.position = at

## This frame's `Session.hud`.
func on_hud(hud: Dictionary) -> void:
	var playing: bool = session != null and session.has_snapshot() and not hud.is_empty() and hud.get("team", "") != ""
	_root.visible = playing
	if not playing:
		return
	_last = hud
	_top.refresh(session.players(), hud)
	_voices.refresh(session.voices())
	_vitals.refresh(hud)
	_gun.refresh(hud)
	_ring.refresh(hud)
	_marker.refresh(hud)
	_result.refresh(hud)
	_interference.refresh(hud)
	_crosshair.alive = hud.get("alive", false)
	_crosshair.weapon = hud.get("weapon", "")
	_crosshair.spread = session.spread()
	_crosshair.zoomed = camera != null and camera.fov < _crosshair.ZOOMED_FOV
	_crosshair.visible = hud.get("planting", -1.0) < 0.0
	# you just became the last one alive on your side, with someone left to fight
	var mine_t: bool = hud.get("team", "") == "t"
	var us: int = hud.get("alive_t" if mine_t else "alive_ct", 0)
	var them: int = hud.get("alive_ct" if mine_t else "alive_t", 0)
	if us == 1 and _alive_us > 1 and them > 0 and hud.get("alive", false) and hud.get("phase", "") == "live":
		_alert.last_alive(them)
	_alive_us = us

func on_event(e: Dictionary) -> void:
	_crosshair.on_event(e)
	match e.type:
		"kill":
			_feed.add(e)
		"hurt":
			_interference.hurt()
		"round_start":
			_feed.clear()
			_alive_us = 5
		"round_end":
			_result.round_won(e.winner, e.reason, _last.get("round", 0))
		"swap":
			_result.halftime()
		"planted":
			_alert.bomb_planted(_last.get("team", ""))
		"match_over":
			var line := "Strays %d · Bureau %d" % [_last.get("score_t", 0), _last.get("score_ct", 0)]
			_result.match_over(null if e.winner == "" else e.winner == _last.get("team", ""), line)

## A short line over the game (the buy menu's "Not in a buy zone").
func notice(text: String) -> void:
	_notice.text = text
	if _notice_tween:
		_notice_tween.kill()
	_notice.modulate.a = 1.0
	_notice_tween = create_tween()
	_notice_tween.tween_interval(NOTICE_SECONDS)
	_notice_tween.tween_property(_notice, "modulate:a", 0.0, Tokens.EXIT)

## The assistant says a line (the brain lands with the voice and assistant port).
func say(who: int, mood: int, text: String) -> void:
	_uplink.say(who, mood, text)
