using Godot;
using System;
using System.Text.Json.Serialization;

public readonly struct PlayerIntent
{
	[JsonPropertyName("tick")] public int Tick { get; init; }
	[JsonPropertyName("move_x")] public float MoveX { get; init; }
	[JsonPropertyName("move_z")] public float MoveZ { get; init; }
	[JsonPropertyName("yaw")] public float Yaw { get; init; }
	[JsonPropertyName("jump")] public bool Jump { get; init; }
}

public partial class Player : CharacterBody3D
{
	private const float Speed = 5.0f;
	private const float TurnSpeed = 15.0f;
	private const float JumpVelocity = 4.5f;
	private const int InputBufferSize = 128;
	private const float ReconciliationThreshold = 0.02f;

	private Node3D _cameraPivot;
	private SpringArm3D _cameraArm;
	private AnimationPlayer _animationPlayer;

	[Export(PropertyHint.Range, "0.0,1.0")]
	public float MouseSensitivity { get; set; } = 0.005f;

	[Export]
	public float TiltLimit { get; set; } = Mathf.DegToRad(75);

	private Vector2 _capturedMousePosition = Vector2.Zero;

	public event Action<PlayerIntent> IntentEmitted;
	private int _tick = 1;
	private bool _serverReady;

	private PlayerIntent[] _inputBuffer = new PlayerIntent[InputBufferSize];
	private Vector3[] _positionBuffer = new Vector3[InputBufferSize];
	private Vector3[] _velocityBuffer = new Vector3[InputBufferSize];

	private Vector3? _pendingCorrectionPos;
	private Vector3 _pendingCorrectionVel;
	private int _pendingCorrectionTick;

	public override void _Ready()
	{
		_cameraPivot = GetNode<Node3D>("%CameraPivot");
		_cameraArm = GetNode<SpringArm3D>("%CameraArm");
		_animationPlayer = GetNode<AnimationPlayer>("AnimationLibrary_Godot_Standard/AnimationPlayer");
	}

	public override void _PhysicsProcess(double delta)
	{
		ProcessPendingCorrection();

		var intent = ReadInput();

		Vector3 rot = Rotation;
		rot.Y = Mathf.RotateToward(rot.Y, intent.Yaw, TurnSpeed * (float)delta);
		Rotation = rot;

		ApplyMovement(intent);

		if (_serverReady)
		{
			_inputBuffer[_tick % InputBufferSize] = intent;
			_positionBuffer[_tick % InputBufferSize] = GlobalPosition;
			_velocityBuffer[_tick % InputBufferSize] = Velocity;
			_tick++;
			IntentEmitted?.Invoke(intent);
		}
	}

	private PlayerIntent ReadInput()
	{
		bool leftHeld = Input.IsMouseButtonPressed(MouseButton.Left);
		bool rightHeld = Input.IsMouseButtonPressed(MouseButton.Right);

		Vector2 inputDir = Input.GetVector("strafe_left", "strafe_right", "move_forward", "move_backward");

		if (leftHeld && rightHeld)
		{
			inputDir.Y -= 1;
		}

		float moveX = 0f;
		float moveZ = 0f;
		float yaw = Rotation.Y;

		if (inputDir != Vector2.Zero)
		{
			Vector3 direction = (_cameraPivot.Basis * new Vector3(inputDir.X, 0, inputDir.Y)).Normalized();
			moveX = direction.X;
			moveZ = direction.Z;

			if (!rightHeld)
			{
				yaw = Mathf.Atan2(direction.X, direction.Z);
			}
		}

		if (rightHeld)
		{
			yaw = _cameraPivot.Rotation.Y + Mathf.Pi;
		}

		return new PlayerIntent
		{
			Tick = _tick,
			MoveX = moveX,
			MoveZ = moveZ,
			Yaw = yaw,
			Jump = Input.IsActionJustPressed("ui_accept") && IsOnFloor(),
		};
	}

	private void ApplyMovement(PlayerIntent intent)
	{
		Vector3 velocity = Velocity;

		if (!IsOnFloor())
		{
			velocity += GetGravity() * (1.0f / 30.0f);
		}

		if (intent.Jump && IsOnFloor())
		{
			velocity.Y = JumpVelocity;
		}

		if (intent.MoveX != 0 || intent.MoveZ != 0)
		{
			velocity.X = intent.MoveX * Speed;
			velocity.Z = intent.MoveZ * Speed;
		}
		else if (IsOnFloor())
		{
			velocity.X = 0;
			velocity.Z = 0;
		}

		Velocity = velocity;
		MoveAndSlide();
	}

	public void ApplyServerCorrection(Vector3 serverPosition, Vector3 serverVelocity, int serverTick)
	{
		if (!_serverReady)
		{
			_serverReady = true;
			GlobalPosition = serverPosition;
			Velocity = serverVelocity;
			return;
		}
		_pendingCorrectionPos = serverPosition;
		_pendingCorrectionVel = serverVelocity;
		_pendingCorrectionTick = serverTick;
	}

	private void ProcessPendingCorrection()
	{
		if (_pendingCorrectionPos is not { } serverPosition)
			return;

		int serverTick = _pendingCorrectionTick;
		_pendingCorrectionPos = null;

		Vector3 predicted = _positionBuffer[serverTick % InputBufferSize];

		float error = predicted.DistanceTo(serverPosition);
		if (error < ReconciliationThreshold)
			return;

		GD.Print($"Correction at tick {serverTick}: error={error:F3}, server={serverPosition}, predicted={predicted}");
		GlobalPosition = serverPosition;
		Velocity = _pendingCorrectionVel;

		for (int t = serverTick + 1; t < _tick; t++)
		{
			ApplyMovement(_inputBuffer[t % InputBufferSize]);
			_positionBuffer[t % InputBufferSize] = GlobalPosition;
			_velocityBuffer[t % InputBufferSize] = Velocity;
		}
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
		}
	}
}
