## Everyone else (for now the practice dummies) as plain capsules, where the simulation says.
extends Node3D

var session: Session

func _process(_delta: float) -> void:
	if session == null:
		return
	var frac := Engine.get_physics_interpolation_fraction()
	var feet := session.others_feet(frac)
	var yaws := session.others_yaw(frac)
	while get_child_count() < feet.size():
		var body := MeshInstance3D.new()
		var capsule := CapsuleMesh.new()
		capsule.radius = 0.4
		capsule.height = 1.83
		body.mesh = capsule
		add_child(body)
	for i in get_child_count():
		var body := get_child(i) as Node3D
		body.visible = i < feet.size()
		if body.visible:
			body.position = feet[i] + Vector3(0, 0.915, 0)
			body.rotation.y = yaws[i]
