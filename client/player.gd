extends CharacterBody3D

const SPEED = 5.0
const JUMP_VELOCITY = 4.5

@onready var _camera: Camera3D = %Camera3D
@onready var _camera_pivot: Node3D = %CameraPivot
@onready var _spring_arm_3d: SpringArm3D = %SpringArm3D

@export_range(0.0, 1.0) var mouse_sensitivity = 0.005
@export var tilt_limit = deg_to_rad(75)

func _physics_process(delta: float) -> void:
	if not is_on_floor():
		velocity += get_gravity() * delta

	if Input.is_action_just_pressed("ui_accept") and is_on_floor():
		velocity.y = JUMP_VELOCITY

	var left_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)
	var right_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT)
	
	var input_dir := Input.get_vector("strafe_left", "strafe_right", "move_forward", "move_backward")
	
	if left_held and right_held:
		input_dir.y -= 1
	
	var direction := (transform.basis * Vector3(input_dir.x, 0, input_dir.y)).normalized()
	if direction:
		velocity.x = direction.x * SPEED
		velocity.z = direction.z * SPEED
		rig.look_at(rig.global_position + direction, Vector3.UP, true)
	else:
		velocity.x = move_toward(velocity.x, 0, SPEED)
		velocity.z = move_toward(velocity.z, 0, SPEED)
		rig.rotation = Vector3.ZERO

	move_and_slide()
	
@onready var animation_player: AnimationPlayer = $AnimationLibrary_Godot_Standard/AnimationPlayer
@onready var rig: Node3D = $AnimationLibrary_Godot_Standard/Rig

func _process(_delta: float) -> void:
	if not is_on_floor():
		animation_player.play("Jump")
	elif velocity:
		animation_player.play("Jog_Fwd")
	else:
		animation_player.play("Idle")

var captured_mouse_position: Vector2 = Vector2.ZERO

func _unhandled_input(event: InputEvent) -> void:
	var left_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)
	var right_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT)
	
	# Hide/show cursor
	if left_held or right_held:
		if not captured_mouse_position:
			captured_mouse_position = get_viewport().get_mouse_position()
		Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
	else:
		Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
		if captured_mouse_position:
			Input.warp_mouse(captured_mouse_position)
			captured_mouse_position = Vector2.ZERO
			

	if event is InputEventMouseMotion:
		# Vertical rotation (pitch)
		if left_held or right_held:
			_camera_pivot.rotation.x -= event.relative.y * mouse_sensitivity
			_camera_pivot.rotation.x = clampf(_camera_pivot.rotation.x, -tilt_limit, tilt_limit)
		
		# Horizontal rotation (yaw)
		if left_held and not right_held:
			_camera_pivot.rotation.y += -event.relative.x * mouse_sensitivity
		elif right_held:
			rotation.y += -event.relative.x * mouse_sensitivity

	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_spring_arm_3d.spring_length -= 1.0
			_spring_arm_3d.spring_length = max(_spring_arm_3d.spring_length, 1.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_spring_arm_3d.spring_length += 1.0
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			global_rotation.y = _camera_pivot.global_rotation.y
			_camera_pivot.rotation.y = 0.0
		
