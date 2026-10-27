## The motion rules from Tokens: things arrive by sliding up and fading in (staggered), leave
## faster than they came, ease out, and glitch only for a moment.
class_name Motion
extends RefCounted

const GLITCH := preload("res://ui/glitch.gdshader")
## How far past a control's sides a glitch reaches (its tears move up to 10).
const REACH := 16

static func tween(node: Node) -> Tween:
	return node.create_tween().set_trans(Tokens.EASE_TRANS).set_ease(Tokens.EASE_TYPE)

## Slides a control up from SLIDE below and fades it in, after `delay` seconds. Inside a
## container (rows of a list) it only fades: the container owns where its children sit, and
## moving them fights its layout (a list built while hidden hasn't placed its rows yet).
static func enter(c: Control, delay := 0.0) -> void:
	if c.has_meta("motion_tween"):
		(c.get_meta("motion_tween") as Tween).kill()
	c.modulate.a = 0.0
	var t := tween(c).set_parallel()
	t.tween_property(c, "modulate:a", 1.0, Tokens.ENTER).set_delay(delay)
	if not c.get_parent() is Container:
		# a second enter while one is running takes over from it, from the same resting place
		var rest: Vector2 = c.get_meta("motion_rest", c.position)
		c.set_meta("motion_rest", rest)
		c.position = rest + Vector2(0, Tokens.SLIDE)
		t.tween_property(c, "position", rest, Tokens.ENTER).set_delay(delay)
	c.set_meta("motion_tween", t)
	t.finished.connect(func():
		c.remove_meta("motion_rest")
		c.remove_meta("motion_tween"))

## Enters each of `items` one STAGGER after the other.
static func enter_all(items: Array, delay := 0.0) -> void:
	for i in items.size():
		enter(items[i], delay + i * Tokens.STAGGER)

## Fades a control out, faster than it arrived.
static func exit(c: Control) -> Tween:
	var t := tween(c)
	t.tween_property(c, "modulate:a", 0.0, Tokens.EXIT)
	return t

## A moment of channel split over a control, then clean.
static func glitch(c: Control) -> void:
	if not c.is_inside_tree() or c.has_meta("glitching"):
		return
	var over := ColorRect.new()
	over.mouse_filter = Control.MOUSE_FILTER_IGNORE
	over.material = ShaderMaterial.new()
	over.material.shader = GLITCH
	over.material.set_shader_parameter("split_px", float(Tokens.GLITCH_SPLIT))
	over.material.set_shader_parameter("left_color", Tokens.BUREAU)
	over.material.set_shader_parameter("right_color", Tokens.STRAYS)
	over.material.set_shader_parameter("seed", randf() * 100.0)  # a different tear each time
	over.material.set_shader_parameter("amount", 1.0)
	# a fresh copy of the screen under it, taken right here in draw order: Godot otherwise
	# copies once for the first reader in a frame, and a second glitch would read a copy
	# taken before its own control was drawn (and paint it out)
	var copy := BackBufferCopy.new()
	copy.copy_mode = BackBufferCopy.COPY_MODE_VIEWPORT
	c.add_child(copy)
	c.add_child(over)
	over.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	# wider than the control by the furthest a tear moves, so ghosts and tears aren't cut off
	over.offset_left = -REACH
	over.offset_right = REACH
	c.set_meta("glitching", true)
	var t := c.create_tween()
	t.tween_property(over.material, "shader_parameter/amount", 0.0, Tokens.GLITCH)
	t.tween_callback(func():
		over.queue_free()
		copy.queue_free()
		c.remove_meta("glitching"))
