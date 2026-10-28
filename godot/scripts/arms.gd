## Your own body in first person: your character (a Quaternius base body, `menu/figure.gd`)
## standing where you stand and turned the way you look, its head hidden so the camera sits in
## it. Its arms reach the gun in your hands (`viewmodel.gd`'s grip points) by IK, so you see
## your hands holding it; look down and you see yourself. It's the rig everyone has (the
## hitboxes' to come), so what you see is what others see. Hidden when you're dead.
extends Node3D

## What the eyes sit in front of (metres): the camera is the eyes, a little ahead of the neck.
const EYES_FORWARD := 0.08
## Above this speed (units a second) the body jogs.
const JOG := 60.0
## Where the elbows point (in the body's space, from the shoulder): down and out.
const ELBOW := Vector3(0.45, -0.6, -0.1)

var session: Session
## This frame's `Session.hud`, from main.gd.
var hud := {}
## The camera you see through, and the gun in your hands.
var camera: Camera3D
var gun: Node3D
var _figure: Node3D
var _skeleton: Skeleton3D
var _anim: AnimationPlayer
var _ik := TwoBoneIK3D.new()
var _hide_head := _HideHead.new()
var _targets: Array[Marker3D] = []
var _poles: Array[Marker3D] = []
var _head := -1

func _ready() -> void:
	top_level = true
	_figure = preload("res://scripts/menu/figure.gd").new()
	_figure.look = {"body": "Female", "skin": "Light", "hair": [], "pose": "Idle"}
	add_child(_figure)
	_skeleton = _figure.skeleton
	_anim = _figure.find_children("*", "AnimationPlayer", true, false)[0]
	_head = _skeleton.find_bone("Head")
	_hide_head.bone = _head
	_skeleton.add_child(_hide_head)
	# right arm first, then left; each reaches its marker with the elbow towards its pole
	_ik.set_setting_count(2)
	for i in 2:
		var s := "r" if i == 0 else "l"
		_ik.set_root_bone_name(i, "upperarm_" + s)
		_ik.set_middle_bone_name(i, "lowerarm_" + s)
		_ik.set_end_bone_name(i, "hand_" + s)
		var target := Marker3D.new()
		var pole := Marker3D.new()
		add_child(target)
		add_child(pole)
		target.top_level = true
		pole.top_level = true
		_targets.append(target)
		_poles.append(pole)
	_skeleton.add_child(_ik)
	for i in 2:
		_ik.set_target_node(i, _ik.get_path_to(_targets[i]))
		_ik.set_pole_node(i, _ik.get_path_to(_poles[i]))
	for m in _figure.find_children("*", "MeshInstance3D", true, false):
		(m as MeshInstance3D).cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF

func _process(_delta: float) -> void:
	visible = camera != null and hud.get("alive", false)
	if not visible:
		return
	# the pose for how you move; the arms are the IK's
	var pose := "Crouch_Idle" if hud.get("crouched", false) else ("Jog_Fwd" if session and session.speed() > JOG else "Idle")
	if _anim.current_animation != pose:
		_anim.play(pose, 0.2)
	# turned the way you look (not tilted with it), the eyes at the camera
	var yaw := camera.global_rotation.y
	var turn := Basis(Vector3.UP, yaw)
	var ahead := -turn.z
	global_transform = Transform3D(turn.rotated(Vector3.UP, PI), camera.global_position - ahead * EYES_FORWARD - Vector3.UP * _eye_height())
	# each hand to its grip on the gun; a free hand hangs (no IK)
	var grips := [gun.get("grip_right"), gun.get("grip_left")] if gun else [null, null]
	for i in 2:
		var g = grips[i]
		var on: bool = g != null and is_instance_valid(g)
		if on:
			_targets[i].global_transform = (g as Node3D).global_transform
			var side := 1.0 if i == 0 else -1.0
			var shoulder := _bone_world("upperarm_" + ("r" if i == 0 else "l"))
			_poles[i].global_position = shoulder + turn * Vector3(ELBOW.x * side, ELBOW.y, ELBOW.z)
		else:
			# a free hand: its target where it rests, beside the hip
			_targets[i].global_position = _bone_world("thigh_" + ("r" if i == 0 else "l")) + turn * Vector3(0.12 * (1.0 if i == 0 else -1.0), -0.1, 0)

## How high the eyes are above the feet in the body's pose now (crouched is lower): the head
## bone's height, and the eyes a little above it.
func _eye_height() -> float:
	var head := _skeleton.global_transform * _skeleton.get_bone_global_pose(_head).origin
	return (global_transform.affine_inverse() * head).y + 0.09

func _bone_world(name: String) -> Vector3:
	return _skeleton.global_transform * _skeleton.get_bone_global_pose(_skeleton.find_bone(name)).origin

## Shrinks the head to nothing after the animation, so the camera can sit where it is.
class _HideHead extends SkeletonModifier3D:
	var bone := -1

	func _process_modification() -> void:
		if bone >= 0:
			get_skeleton().set_bone_pose_scale(bone, Vector3.ONE * 0.001)
