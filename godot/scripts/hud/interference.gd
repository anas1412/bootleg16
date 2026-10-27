## The HUD's interference, only ever as a signal: a hit tears your health (the identity's
## glitch, harder), and while your health is 25 or less the whole HUD drops out like a failing
## signal every few seconds, until you heal or die. (Things arriving tune in on their own.)
extends Node

## Seconds between dropouts while you're low.
const WEAK_EVERY := 2.6

## The controls a dropout tears (the HUD's groups), and the one a hit tears (your health).
var groups: Array[Control] = []
var health: Control
var _weak := false
var _since := 0.0

func hurt() -> void:
	if health:
		Motion.glitch(health, true)

## Low or not, from this frame's `Session.hud`.
func refresh(hud: Dictionary) -> void:
	_weak = hud.get("alive", false) and hud.get("health", 100) <= 25

func _process(delta: float) -> void:
	if not _weak:
		_since = 0.0
		return
	_since += delta
	if _since >= WEAK_EVERY:
		_since = 0.0
		for g in groups:
			Motion.glitch(g, true)
