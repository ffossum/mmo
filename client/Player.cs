using Godot;

public partial class Player : CharacterBody3D
{
	private const float Speed = 5.0f;
	private const float TurnSpeed = 15.0f;
	private const float JumpVelocity = 4.5f;

	private Node3D _cameraPivot;
	private SpringArm3D _cameraArm;
	private AnimationPlayer _animationPlayer;
	private Node3D _rig;

	[Export(PropertyHint.Range, "0.0,1.0")]
	public float MouseSensitivity { get; set; } = 0.005f;

	[Export]
	public float TiltLimit { get; set; } = Mathf.DegToRad(75);

	private Vector2 _capturedMousePosition = Vector2.Zero;

	public override void _Ready()
	{
		_cameraPivot = GetNode<Node3D>("%CameraPivot");
		_cameraArm = GetNode<SpringArm3D>("%CameraArm");
		_animationPlayer = GetNode<AnimationPlayer>("AnimationLibrary_Godot_Standard/AnimationPlayer");
		_rig = GetNode<Node3D>("AnimationLibrary_Godot_Standard/Rig");
	}

	public override void _PhysicsProcess(double delta)
	{
		Vector3 velocity = Velocity;

		if (!IsOnFloor())
		{
			velocity += GetGravity() * (float)delta;
		}

		if (Input.IsActionJustPressed("ui_accept") && IsOnFloor())
		{
			velocity.Y = JumpVelocity;
		}

		bool leftHeld = Input.IsMouseButtonPressed(MouseButton.Left);
		bool rightHeld = Input.IsMouseButtonPressed(MouseButton.Right);

		Vector2 inputDir = Input.GetVector("strafe_left", "strafe_right", "move_forward", "move_backward");

		if (leftHeld && rightHeld)
		{
			inputDir.Y -= 1;
		}

		if (inputDir != Vector2.Zero)
		{
			Vector3 direction = (_cameraPivot.Basis * new Vector3(inputDir.X, 0, inputDir.Y)).Normalized();
			velocity.X = direction.X * Speed;
			velocity.Z = direction.Z * Speed;

			if (!rightHeld)
			{
				float targetYaw = Mathf.Atan2(direction.X, direction.Z);
				Vector3 rot = Rotation;
				rot.Y = Mathf.RotateToward(rot.Y, targetYaw, TurnSpeed * (float)delta);
				Rotation = rot;
			}
		}
		else if (IsOnFloor())
		{
			velocity.X = 0;
			velocity.Z = 0;
		}

		Velocity = velocity;
		MoveAndSlide();
	}

	public override void _Process(double delta)
	{
		if (!IsOnFloor())
		{
			_animationPlayer.Play("Jump");
		}
		else if (Velocity != Vector3.Zero)
		{
			_animationPlayer.Play("Jog_Fwd");
		}
		else
		{
			_animationPlayer.Play("Idle");
		}
	}

	public override void _UnhandledInput(InputEvent @event)
	{
		bool leftHeld = Input.IsMouseButtonPressed(MouseButton.Left);
		bool rightHeld = Input.IsMouseButtonPressed(MouseButton.Right);

		if (leftHeld || rightHeld)
		{
			if (_capturedMousePosition == Vector2.Zero)
			{
				_capturedMousePosition = GetViewport().GetMousePosition();
			}
			Input.MouseMode = Input.MouseModeEnum.Captured;
		}
		else
		{
			Input.MouseMode = Input.MouseModeEnum.Visible;
			if (_capturedMousePosition != Vector2.Zero)
			{
				Input.WarpMouse(_capturedMousePosition);
				_capturedMousePosition = Vector2.Zero;
			}
		}

		if (@event is InputEventMouseMotion mouseMotion)
		{
			if (leftHeld || rightHeld)
			{
				Vector3 armRot = _cameraArm.Rotation;
				armRot.X -= mouseMotion.Relative.Y * MouseSensitivity;
				armRot.X = Mathf.Clamp(armRot.X, -TiltLimit, TiltLimit);
				_cameraArm.Rotation = armRot;

				Vector3 pivotRot = _cameraPivot.Rotation;
				pivotRot.Y += -mouseMotion.Relative.X * MouseSensitivity;
				_cameraPivot.Rotation = pivotRot;
			}

			if (rightHeld)
			{
				Vector3 rot = Rotation;
				rot.Y = _cameraPivot.Rotation.Y + Mathf.Pi;
				Rotation = rot;
			}
		}
		else if (@event is InputEventMouseButton mouseButton)
		{
			if (mouseButton.ButtonIndex == MouseButton.WheelUp && mouseButton.Pressed)
			{
				_cameraArm.SpringLength -= 1.0f;
				_cameraArm.SpringLength = Mathf.Max(_cameraArm.SpringLength, 1.0f);
			}
			else if (mouseButton.ButtonIndex == MouseButton.WheelDown && mouseButton.Pressed)
			{
				_cameraArm.SpringLength += 1.0f;
			}
			else if (mouseButton.ButtonIndex == MouseButton.Right && mouseButton.Pressed)
			{
				Vector3 rot = Rotation;
				rot.Y = _cameraPivot.Rotation.Y + Mathf.Pi;
				Rotation = rot;
			}
		}
	}
}
