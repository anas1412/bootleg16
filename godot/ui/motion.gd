## The motion rules from Tokens: things arrive by sliding up and fading in (staggered), leave
## faster than they came, ease out, and glitch only for a moment.
class_name Motion
extends RefCounted

const GLITCH := preload("res://ui/glitch.gdshader")

static func tween(node: Node) -> Tween:
	return node.create_tween().set_trans(Tokens.EASE_TRANS).set_ease(Tokens.EASE_TYPE)

## Slides a control up from SLIDE below and fades it in, after `delay` seconds.
static func enter(c: Control, delay := 0.0) -> void:
	c.modulate.a = 0.0
	# a container places its children on its next sort: start from where it puts them
	if c.get_parent() is Container:
		await c.get_tree().process_frame
		if not is_instance_valid(c) or not c.is_inside_tree():
			return
	# a second enter while one is running takes over from it, from the same resting place
	var rest: Vector2 = c.get_meta("motion_rest", c.position)
	if c.has_meta("motion_tween"):
		(c.get_meta("motion_tween") as Tween).kill()
	c.set_meta("motion_rest", rest)
	c.position = rest + Vector2(0, Tokens.SLIDE)
	var t := tween(c).set_parallel()
	t.tween_property(c, "modulate:a", 1.0, Tokens.ENTER).set_delay(delay)
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
	over.material.set_shader_parameter("amount", 1.0)
	c.add_child(over)
	over.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	# a little wider than the control, so split edges aren't cut off
	over.offset_left = -Tokens.GLITCH_SPLIT
	over.offset_right = Tokens.GLITCH_SPLIT
	c.set_meta("glitching", true)
	var t := c.create_tween()
	t.tween_property(over.material, "shader_parameter/amount", 0.0, Tokens.GLITCH)
	t.tween_callback(func():
		over.queue_free()
		c.remove_meta("glitching"))
