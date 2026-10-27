## Saves one frame of a scene at the window's size, for checking a look:
##   godot --path godot --display-driver wayland --resolution 1920x1080 --script res://tools/shot.gd -- <scene> <out.png> [frames] [property=value ...]
## Waits `frames` (240) so lights, fog and arrival motion settle. Properties are set on the
## scene's root before it enters the tree (members=3 framing=1 on a base). A base also prints
## where its members' names go (name_points), for laying a mockup over the frame.
extends SceneTree

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	var frames := int(args[2]) if args.size() > 2 else 240
	var scene: Node = load(args[0]).instantiate()
	for a in args.slice(3):
		var kv := a.split("=")
		scene.set(kv[0], str_to_var(kv[1]))
	root.add_child.call_deferred(scene)
	for i in frames:
		await process_frame
	await RenderingServer.frame_post_draw
	root.get_texture().get_image().save_png(args[1])
	if scene.has_method("name_points"):
		print("NAME_POINTS ", JSON.stringify(scene.name_points().map(func(p): return [roundi(p.x), roundi(p.y)])))
	quit()
