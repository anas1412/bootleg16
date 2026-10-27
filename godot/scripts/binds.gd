## CS's default keys as Godot input actions, by key position (physical keys), so AZERTY and
## other layouts get the same places as QWERTY.
class_name Binds
extends RefCounted

const KEYS := {
	"forward": KEY_W, "back": KEY_S, "left": KEY_A, "right": KEY_D,
	"jump": KEY_SPACE, "crouch": KEY_CTRL, "walk": KEY_SHIFT, "use": KEY_E,
	"reload": KEY_R, "drop": KEY_G,
	"slot1": KEY_1, "slot2": KEY_2, "slot3": KEY_3, "slot4": KEY_4, "slot5": KEY_5,
}
const MOUSE := {"attack": MOUSE_BUTTON_LEFT, "attack2": MOUSE_BUTTON_RIGHT}

static func install() -> void:
	for action in KEYS:
		var e := InputEventKey.new()
		e.physical_keycode = KEYS[action]
		_add(action, e)
	for action in MOUSE:
		var e := InputEventMouseButton.new()
		e.button_index = MOUSE[action]
		_add(action, e)

static func _add(action: String, e: InputEvent) -> void:
	if not InputMap.has_action(action):
		InputMap.add_action(action)
	InputMap.action_add_event(action, e)
