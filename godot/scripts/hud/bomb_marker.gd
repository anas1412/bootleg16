## Where the bomb is, drawn over the world where CS's radar would show it: once it's planted,
## to everyone; dropped, only to the Strays. A red diamond, BOMB and how far it is.
extends Control

var camera: Camera3D
var _at := Vector3.ZERO
var _on := false
var _name: Label
var _far: Label
var _labels := VBoxContainer.new()

func _ready() -> void:
	set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_labels.add_theme_constant_override("separation", 0)
	_name = HudText.label("tab", "Bomb", Tokens.ALERT)
	_far = HudText.label("hud_tag", "")
	for l in [_name, _far]:
		l.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
		_labels.add_child(l)
	add_child(_labels)

## From this frame's `Session.hud`.
func refresh(hud: Dictionary) -> void:
	var state: String = hud.get("bomb", "none")
	_on = hud.has("bomb_at") and (state == "planted" or (state == "dropped" and hud.get("team", "") == "t"))
	if _on:
		_at = hud.bomb_at
	queue_redraw()

func _draw() -> void:
	_labels.visible = false
	if not _on or camera == null or camera.is_position_behind(_at):
		return
	var p := camera.unproject_position(_at)
	var r := 18.0
	var pts := PackedVector2Array([p + Vector2(0, -r), p + Vector2(r, 0), p + Vector2(0, r), p + Vector2(-r, 0), p + Vector2(0, -r)])
	draw_colored_polygon(pts.slice(0, 4), Color(0.12, 0.02, 0.02, 0.5))
	draw_polyline(pts, Tokens.ALERT, 3.0)
	draw_rect(Rect2(p - Vector2(8, 4), Vector2(16, 9)), Tokens.ALERT)
	_far.text = "%d m" % roundi(camera.global_position.distance_to(_at))
	_labels.visible = true
	_labels.reset_size()
	_labels.position = p + Vector2(-_labels.size.x / 2.0, r + Tokens.SPACE_XS)
