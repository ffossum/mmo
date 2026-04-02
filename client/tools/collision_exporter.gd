class_name CollisionExporter

## Shared logic for exporting collision geometry to .glb.
## Used by both the editor script and CLI script.

static func export_scene(scene: Node) -> void:
	var root = Node3D.new()
	root.name = "CollisionExport"

	var count = _collect_collision_shapes(scene, root)

	if count == 0:
		printerr("No collision shapes found in the scene")
		root.free()
		return

	var doc = GLTFDocument.new()
	var state = GLTFState.new()
	var err = doc.append_from_scene(root, state)
	if err != OK:
		printerr("Failed to create glTF from scene: ", err)
		root.free()
		return

	var project_dir = ProjectSettings.globalize_path("res://").rstrip("/")
	var repo_root = project_dir.get_base_dir()
	var shared_dir = repo_root + "/shared"
	DirAccess.make_dir_absolute(shared_dir)
	var export_path = shared_dir + "/collision.glb"

	err = doc.write_to_filesystem(state, export_path)
	if err != OK:
		printerr("Failed to write glb: ", err)
	else:
		print("Exported ", count, " collision shape(s) to ", export_path)

	root.free()


static func _collect_collision_shapes(node: Node, export_root: Node3D) -> int:
	var count = 0

	if node is CSGShape3D and node.use_collision and node.is_root_shape():
		var meshes = node.get_meshes()
		for i in range(0, meshes.size(), 2):
			var xform: Transform3D = meshes[i]
			var mesh: Mesh = meshes[i + 1]
			var mi = MeshInstance3D.new()
			mi.mesh = mesh
			mi.transform = node.global_transform * xform
			export_root.add_child(mi)
			mi.owner = export_root
			count += 1

	if node is CollisionShape3D and node.shape and not node.get_parent() is CharacterBody3D:
		var mi = _shape_to_mesh_instance(node)
		if mi:
			mi.transform = node.global_transform
			export_root.add_child(mi)
			mi.owner = export_root
			count += 1

	for child in node.get_children():
		count += _collect_collision_shapes(child, export_root)

	return count


static func _shape_to_mesh_instance(node: CollisionShape3D) -> MeshInstance3D:
	var shape = node.shape
	var mi = MeshInstance3D.new()

	if shape is BoxShape3D:
		var m = BoxMesh.new()
		m.size = shape.size
		mi.mesh = m
	elif shape is SphereShape3D:
		var m = SphereMesh.new()
		m.radius = shape.radius
		m.height = shape.radius * 2
		mi.mesh = m
	elif shape is CapsuleShape3D:
		var m = CapsuleMesh.new()
		m.radius = shape.radius
		m.height = shape.height
		mi.mesh = m
	elif shape is CylinderShape3D:
		var m = CylinderMesh.new()
		m.top_radius = shape.radius
		m.bottom_radius = shape.radius
		m.height = shape.height
		mi.mesh = m
	elif shape is ConcavePolygonShape3D:
		var arr_mesh = ArrayMesh.new()
		var arrays = []
		arrays.resize(Mesh.ARRAY_MAX)
		arrays[Mesh.ARRAY_VERTEX] = shape.get_faces()
		arr_mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, arrays)
		mi.mesh = arr_mesh
	else:
		mi.free()
		return null

	return mi
