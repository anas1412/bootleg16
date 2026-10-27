## The lobby stage behind the menu: your character under one hard light on the hex platform,
## a squadmate beside you when you have one, and a rim light in your side's colour.
extends Node3D

func _ready() -> void:
	# a long lens a little below eye level, on the character's chest
	$Camera.position = Vector3(0.25, 1.45, 6.6)
	$Camera.look_at(Vector3(0, 0.95, 0))
	$Lights/Face.look_at(Vector3(0, 1.45, 0))  # soft key on the face, from the front left
	for p in find_children("*", "AnimationPlayer", true, false):
		if p.has_animation("Stand"):
			p.play("Stand")

func set_side(color: Color) -> void:
	$Lights/Rim.light_color = color

func set_squad(count: int) -> void:
	$Stage/Mate.visible = count > 1
