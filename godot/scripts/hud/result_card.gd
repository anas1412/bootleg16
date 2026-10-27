## The round's result over the game, the menu's title card: a line in the winner's colour
## (the round and why it ended), the winner in Anton, its katakana, a rule in its colour and
## when the next round starts. Halftime, and the match's end (which takes you back to the
## menu after a while), use the same card.
extends Control

signal leave

const SIDES := {
	"t": {"name": "Strays win", "kana": "ストレイズ勝利", "color": Tokens.STRAYS},
	"ct": {"name": "Bureau wins", "kana": "ビューロー勝利", "color": Tokens.BUREAU},
}
const REASONS := {"eliminated": "The other side is down", "bomb": "The bomb went off", "defused": "The bomb was defused", "time": "Time ran out"}
## Seconds before the match's end takes you back to the menu.
const BACK_AFTER := 12.0

var _card := VBoxContainer.new()
var _line: Label
var _word: Label
var _kana: Label
var _rule := ColorRect.new()
var _next: Label
var _round_over := false
## The round's "over" phase has been seen since the card opened (the round's end can arrive a
## frame before the phase does, so the card waits for it before closing on the next round).
var _seen_over := false

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_card.add_theme_constant_override("separation", Tokens.SPACE_S)
	_line = HudText.label("label")
	_word = HudText.label("moment")
	_kana = HudText.label("kana", "", Tokens.TEXT_DIM)
	_rule.custom_minimum_size = Vector2(240, Tokens.STROKE_FRAME)
	_rule.size_flags_horizontal = Control.SIZE_SHRINK_CENTER
	_next = HudText.label("body")
	for c in [_line, _word, _kana, _rule, _next]:
		if c is Label:
			c.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		_card.add_child(c)
	add_child(_card)
	_card.visible = false

## A round ended: who won and why.
func round_won(winner: String, reason: String, n: int) -> void:
	var s: Dictionary = SIDES.get(winner, {"name": "Round over", "kana": "", "color": Tokens.TEXT})
	_open("Round %d · %s" % [n, REASONS.get(reason, "")], s.name, s.kana, s.color)
	_round_over = true

func halftime() -> void:
	_open("Halftime", "Sides swap", "ハーフタイム", Tokens.TEXT)
	_round_over = true

## The match ended: won, lost or a draw, with the score; back to the menu after a while.
func match_over(won: Variant, line: String) -> void:
	var word := "Draw" if won == null else ("Match won" if won else "Match lost")
	_open(line, word, "", Tokens.YOU if won == true else Tokens.TEXT)
	_next.text = ""
	_round_over = false
	get_tree().create_timer(BACK_AFTER).timeout.connect(func(): leave.emit())

## From this frame's `Session.hud`: the countdown to the next round, and away once it starts.
func refresh(hud: Dictionary) -> void:
	if not _round_over:
		return
	if hud.get("phase", "") == "over":
		_seen_over = true
		_next.text = "Next round in %d s" % ceili(maxf(hud.get("left", 0.0), 0.0))
	elif _seen_over:
		_round_over = false
		Motion.exit(_card).tween_callback(func(): _card.visible = false)

func _open(line: String, word: String, kana: String, color: Color) -> void:
	_line.text = line
	_word.text = word
	_kana.text = kana
	_kana.visible = kana != ""
	_line.add_theme_color_override("font_color", color)
	# the word white, as the menu's title card; the line and the rule carry the colour
	_word.add_theme_color_override("font_color", Tokens.TEXT)
	_rule.color = color
	_next.text = ""
	_seen_over = false
	_card.visible = true
	_card.modulate.a = 1.0
	_card.reset_size()
	_card.position = Vector2((size.x - _card.size.x) / 2.0, 190)
	Motion.enter(_card)
	Motion.glitch.call_deferred(_word)
