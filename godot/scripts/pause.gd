## Esc in a game: frees the mouse, blurs the game behind and offers Resume or Leave (back to
## the menu). Esc again or Resume puts you back in.
extends CanvasLayer

signal leave

var _root := Control.new()
var _blur := ColorRect.new()
var _buttons := VBoxContainer.new()

func _ready() -> void:
	layer = 10
	_root.theme = UiTheme.theme()
	_root.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.visible = false
	add_child(_root)
	_blur.material = ShaderMaterial.new()
	_blur.material.shader = preload("res://ui/blur.gdshader")
	_blur.set_anchors_preset(Control.PRESET_FULL_RECT)
	_root.add_child(_blur)
	_buttons.add_theme_constant_override("separation", Tokens.SPACE_M)
	var resume := FrameButton.new("Resume", true)
	resume.pressed.connect(close)
	var quit := FrameButton.new("Leave")
	quit.pressed.connect(func(): leave.emit())
	_buttons.add_child(UiTheme.label("page", "Paused"))
	_buttons.add_child(resume)
	_buttons.add_child(quit)
	Grid.place(_buttons, 0, 3, 0)
	_root.add_child(_buttons)
	_buttons.position.y = Grid.bottom() - _buttons.get_combined_minimum_size().y
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED

func is_open() -> bool:
	return _root.visible

func open() -> void:
	_root.visible = true
	Input.mouse_mode = Input.MOUSE_MODE_VISIBLE
	_blur.material.set_shader_parameter("amount", 0.0)
	Motion.tween(_blur).tween_property(_blur.material, "shader_parameter/amount", 1.0, Tokens.ENTER)
	Motion.enter_all(_buttons.get_children())

func close() -> void:
	_root.visible = false
	Input.mouse_mode = Input.MOUSE_MODE_CAPTURED

func _unhandled_input(event: InputEvent) -> void:
	if event.is_action_pressed("ui_cancel"):
		close() if is_open() else open()
		get_viewport().set_input_as_handled()
