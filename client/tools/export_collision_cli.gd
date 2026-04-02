extends SceneTree

## Usage: godot --headless --script tools/export_collision_cli.gd -- main.tscn

func _init():
	var scene_path = _get_scene_path()

	var packed = load(scene_path) as PackedScene
	if not packed:
		printerr("Failed to load scene: ", scene_path)
		quit(1)
		return

	var scene = packed.instantiate()
	root.add_child(scene)

	# Wait one frame for CSG shapes to process their meshes
	await process_frame

	CollisionExporter.export_scene(scene)
	quit(0)


func _get_scene_path() -> String:
	var args = OS.get_cmdline_user_args()
	if args.size() > 0:
		var path = args[0]
		if not path.begins_with("res://"):
			path = "res://" + path
		return path

	return ProjectSettings.get_setting("application/run/main_scene")
