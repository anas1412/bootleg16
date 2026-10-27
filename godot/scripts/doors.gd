## The map's doors where the server says they are: each door node slides or swings from
## where it was placed (closed).
extends Node

var session: Session
## Door nodes, in the map file's order (the order the server sends positions in), and where
## each was placed.
var _nodes: Array[Node3D] = []
var _rest: Array[Transform3D] = []

func setup(map_scene: Node, map: Dictionary) -> void:
	for d in map.get("doors", []):
		var node := map_scene.get_node_or_null(NodePath(d.node)) as Node3D
		_nodes.append(node)
		_rest.append(node.transform if node else Transform3D())

func _process(_delta: float) -> void:
	if session == null or _nodes.is_empty():
		return
	var offsets: PackedVector3Array = session.door_offsets()
	var turns: PackedFloat32Array = session.door_turns()
	for i in min(_nodes.size(), offsets.size()):
		if _nodes[i]:
			_nodes[i].transform = _rest[i].rotated_local(Vector3.UP, turns[i]).translated(offsets[i])
