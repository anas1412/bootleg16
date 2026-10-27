## You: reads the mouse and keys, runs one simulation step per physics tick (at the
## server's rate) and puts the camera where the simulation says your eye is.
extends Camera3D

## CS's m_yaw: degrees per mouse count at sensitivity 1.
const M_YAW := 0.022
## The vertical field of view of CS's normal 90 degrees (at 4:3): zooming in slows the mouse.
const NORMAL_FOV := 73.74
## The buttons `Session.tick` takes, in bit order.
const BUTTONS := ["jump", "crouch", "walk", "use", "attack", "attack2", "reload", "drop"]

var session: Session
var yaw := 0.0
var pitch := 0.0

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion and Input.mouse_mode == Input.MOUSE_MODE_CAPTURED:
		# zoomed scopes slow the mouse in proportion, like CS
		var k := deg_to_rad(Settings.sensitivity * M_YAW) * fov / NORMAL_FOV
		yaw -= event.relative.x * k
		pitch = clamp(pitch - event.relative.y * k, deg_to_rad(-89), deg_to_rad(89))

func _physics_process(_delta: float) -> void:
	if session == null:
		return
	var forward := Input.get_action_strength("forward") - Input.get_action_strength("back")
	var side := Input.get_action_strength("right") - Input.get_action_strength("left")
	var buttons := 0
	for i in BUTTONS.size():
		if Input.is_action_pressed(BUTTONS[i]):
			buttons |= 1 << i
	var slot := 0
	for s in range(1, 6):
		if Input.is_action_just_pressed("slot%d" % s):
			slot = s
	# the game keeps running under the pause overlay, but your hands are off the keys
	if Input.mouse_mode != Input.MOUSE_MODE_CAPTURED:
		forward = 0.0
		side = 0.0
		buttons = 0
		slot = 0
	session.tick(forward, side, yaw, pitch, buttons, slot)

func _process(delta: float) -> void:
	if session == null:
		return
	var frac := Engine.get_physics_interpolation_fraction()
	position = session.eye(frac, delta)
	var punch := session.punch(frac)
	rotation = Vector3(pitch + deg_to_rad(punch.x), yaw + deg_to_rad(punch.y), deg_to_rad(punch.z))
	fov = session.fov()
