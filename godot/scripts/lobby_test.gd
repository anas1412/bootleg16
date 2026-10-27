## Lobby stage test: your character under one hard light on the hex platform, a party member
## beside you, the title card, squad list and Deploy on the grid. F swaps the faction (Strays
## with a pink rim, Bureau with a cyan rim) to judge both. The characters are the clay
## mannequin placeholder until the real ones come, last.
## BOOTLEG16_SHOT=path.png saves one frame after a moment and quits (for checking the look).
extends Node3D

const STRAYS := {"name": "VERA", "kana": "ヴェラ", "side": "STRAYS · ATTACK", "rim": Color(1.0, 0.31, 0.55), "line": "Plants the bomb. Every round."}
const BUREAU := {"name": "MEI", "kana": "メイ", "side": "BUREAU · DEFEND", "rim": Color(0.29, 0.78, 1.0), "line": "Stops the bomb. Every round."}

var strays := true

func _ready() -> void:
	# a long lens a little below eye level, on the character's chest
	$Camera.position = Vector3(0.25, 1.45, 6.6)
	$Camera.look_at(Vector3(0, 0.95, 0))
	$Lights/Face.look_at(Vector3(0, 1.45, 0))  # soft key on the face, from the front left
	if OS.get_environment("BOOTLEG16_CLOSE") != "":  # face close-up for judging the character
		$Camera.position = Vector3(0.1, 1.62, 1.6)
		$Camera.look_at(Vector3(0, 1.55, 0))
	for p in find_children("*", "AnimationPlayer", true, false):
		var anim := "Idle" if p.has_animation("Idle") else ("Stand" if p.has_animation("Stand") else "")
		if anim != "":
			p.get_animation(anim).loop_mode = Animation.LOOP_LINEAR
			p.play(anim)
	_apply()
	var shot := OS.get_environment("BOOTLEG16_SHOT")
	if shot != "":
		await get_tree().create_timer(3.0).timeout
		get_viewport().get_texture().get_image().save_png(shot)
		get_tree().quit()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and event.physical_keycode == KEY_F:
		strays = not strays
		_apply()

func _apply() -> void:
	var f := STRAYS if strays else BUREAU
	$Lights/Rim.light_color = f.rim
	$UI/Title/Side.text = f.side
	$UI/Title/Side.add_theme_color_override("font_color", f.rim)
	$UI/Title/Name.text = f.name
	$UI/Title/Kana.text = f.kana
	$UI/Title/Rule.color = f.rim
	$UI/Title/Line.text = f.line
