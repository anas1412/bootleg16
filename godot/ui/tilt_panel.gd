## A HUD group drawn on the visor (`tilt.gdshader`): its children live in `content`, a
## control the size of the panel in its own viewport, and the panel draws that turned in
## towards you. Glitches inside it read its own viewport.
class_name TiltPanel
extends SubViewportContainer

## How much smaller the far edge is (the visor's curve at the screen's sides and top).
const TILT := 0.12

var content := Control.new()
var _view := SubViewport.new()

## `x`: 1 on the left of the screen, -1 on the right, 0 neither; `y`: 1 at the top centre.
func _init(panel_size: Vector2, x := 0.0, y := 0.0) -> void:
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	stretch = true
	size = panel_size
	custom_minimum_size = panel_size
	_view.transparent_bg = true
	_view.disable_3d = true
	_view.handle_input_locally = false
	add_child(_view)
	content.set_anchors_preset(Control.PRESET_FULL_RECT)
	content.mouse_filter = Control.MOUSE_FILTER_IGNORE
	content.theme = UiTheme.theme()
	_view.add_child(content)
	material = ShaderMaterial.new()
	material.shader = preload("res://ui/tilt.gdshader")
	material.set_shader_parameter("tilt_x", x * TILT)
	material.set_shader_parameter("tilt_y", y * TILT)
