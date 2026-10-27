## The map's breakable pieces as the server says they are: hidden while broken (the server
## puts them back at the start of each round). Sounds and debris come with the effects.
extends Node

var session: Session
## Piece number -> its node in the map scene.
var _nodes := {}

func setup(map_scene: Node, map: Dictionary) -> void:
	for b in map.get("breakables", []):
		var node := map_scene.get_node_or_null(NodePath(b.node))
		if node:
			_nodes[int(b.model)] = node

func _process(_delta: float) -> void:
	if session == null or _nodes.is_empty():
		return
	var broken: PackedInt32Array = session.broken_pieces()
	for model in _nodes:
		_nodes[model].visible = not broken.has(model)
