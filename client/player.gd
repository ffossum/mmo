extends CharacterBody3D

const SPEED = 5.0
const TURN_SPEED = 15.0
const JUMP_VELOCITY = 4.5

@onready var _camera_pivot: Node3D = %CameraPivot
@onready var _camera_arm: SpringArm3D = %CameraArm

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
	
	if input_dir:
		var direction := (_camera_pivot.basis * Vector3(input_dir.x, 0, input_dir.y)).normalized()
		velocity.x = direction.x * SPEED
		velocity.z = direction.z * SPEED
		if not right_held:
			var target_yaw := atan2(direction.x, direction.z)
			rotation.y = rotate_toward(rotation.y, target_yaw, TURN_SPEED * delta)

	elif is_on_floor():
		velocity.x = 0
		velocity.z = 0
	
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
		if left_held or right_held:
			# Vertical rotation by tilting arm up/down
			_camera_arm.rotation.x -= event.relative.y * mouse_sensitivity
			_camera_arm.rotation.x = clampf(_camera_arm.rotation.x, -tilt_limit, tilt_limit)
			
			# Horizontal rotation by rotating the pivot
			_camera_pivot.rotation.y += -event.relative.x * mouse_sensitivity
			
		if right_held:
			# Rotate character to always keep back to camera
			rotation.y = _camera_pivot.rotation.y + PI
		
	elif event is InputEventMouseButton:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP and event.pressed:
			_camera_arm.spring_length -= 1.0
			_camera_arm.spring_length = max(_camera_arm.spring_length, 1.0)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN and event.pressed:
			_camera_arm.spring_length += 1.0
		elif event.button_index == MOUSE_BUTTON_RIGHT and event.pressed:
			rotation.y = _camera_pivot.rotation.y + PI
