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

	var input_dir := Input.get_vector("strafe_left", "strafe_right", "move_forward", "move_backward")
	var direction := (transform.basis * Vector3(input_dir.x, 0, input_dir.y)).normalized()
	if direction:
		velocity.x = direction.x * SPEED
		velocity.z = direction.z * SPEED
	else:
		velocity.x = move_toward(velocity.x, 0, SPEED)
		velocity.z = move_toward(velocity.z, 0, SPEED)

	move_and_slide()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventMouseMotion:
		var left_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT)
		var right_held := Input.is_mouse_button_pressed(MOUSE_BUTTON_RIGHT)

		# Vertical rotation (pitch)
		if left_held or right_held:
			_camera_pivot.rotation.x -= event.relative.y * mouse_sensitivity
			_camera_pivot.rotation.x = clampf(_camera_pivot.rotation.x, -tilt_limit, tilt_limit)
		
		# Horizontal rotation (yaw)
		if left_held and not right_held:
			_camera_pivot.rotation.y += -event.relative.x * mouse_sensitivity
		elif right_held:
			global_rotation.y = _camera_pivot.global_rotation.y
			_camera_pivot.rotation.y = 0.0
			rotation.y += -event.relative.x * mouse_sensitivity
		
		# Hide/show cursor
		if left_held or right_held:
			Input.set_mouse_mode(Input.MOUSE_MODE_CAPTURED)
		else:
			Input.set_mouse_mode(Input.MOUSE_MODE_VISIBLE)
	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_spring_arm_3d.spring_length -= 1.0
			_spring_arm_3d.spring_length = max(_spring_arm_3d.spring_length, 1.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_spring_arm_3d.spring_length += 1.0
