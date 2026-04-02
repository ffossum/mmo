GODOT ?= godot

.PHONY: server export-collision

shared/collision.glb: client/main.tscn client/tools/export_collision_cli.gd
	cd client && $(GODOT) --headless --script tools/export_collision_cli.gd -- main.tscn

export-collision: shared/collision.glb

server: shared/collision.glb
	cd server && cargo run
