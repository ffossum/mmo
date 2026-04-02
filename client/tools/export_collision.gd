@tool
extends EditorScript

## Run from the editor: open this script, then File > Run (or Ctrl+Shift+X).

func _run():
	var scene = EditorInterface.get_edited_scene_root()
	if not scene:
		printerr("No scene open in the editor")
		return
	CollisionExporter.export_scene(scene)
