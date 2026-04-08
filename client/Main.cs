using Godot;
using System.Collections.Generic;
using System.Text;
using System.Text.Json;

public readonly struct ServerSnapshot
{
	[System.Text.Json.Serialization.JsonPropertyName("x")] public float X { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("y")] public float Y { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("z")] public float Z { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("velocity_x")] public float VelocityX { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("velocity_y")] public float VelocityY { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("velocity_z")] public float VelocityZ { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("server_tick")] public int ServerTick { get; init; }
	[System.Text.Json.Serialization.JsonPropertyName("last_client_tick")] public int LastClientTick { get; init; }
}

public partial class Main : Node3D
{
	private ENetConnection _client;
	private ENetPacketPeer _serverPeer;
	private bool _connected = false;
	private Player _player;
	private const string ServerHost = "172.18.186.168";
	private const int ServerPort = 9001;
	private const int RedundantInputCount = 3;
	private readonly Queue<PlayerIntent> _recentIntents = new();

	public override void _Ready()
	{
		_client = new ENetConnection();
		var error = _client.CreateHost(1, 0, 0, 0);
		if (error != Error.Ok)
		{
			GD.PrintErr($"Failed to create ENet host: {error}");
			return;
		}

		GD.Print($"Connecting to server at {ServerHost}:{ServerPort}...");
		_serverPeer = _client.ConnectToHost(ServerHost, ServerPort, 2, 0);
		if (_serverPeer == null)
		{
			GD.PrintErr("Failed to initiate connection");
			return;
		}
		GD.Print($"Connection initiated, peer state: {_serverPeer.GetState()}");

		_player = GetNode<Player>("Player");
		_player.IntentEmitted += OnPlayerIntent;
	}

	public override void _PhysicsProcess(double delta)
	{
		ServiceENet();
	}

	private void ServiceENet()
	{
		if (_client == null) return;
		var events = _client.Service(0);
		while (events[0].AsInt32() > 0)
		{
			var eventType = (ENetConnection.EventType)events[0].AsInt32();
			var peer = events[1].As<ENetPacketPeer>();
			var data = events[2].AsInt32();
			var channel = events[3].AsInt32();

			switch (eventType)
			{
				case ENetConnection.EventType.Connect:
					GD.Print("Connected to server!");
					_connected = true;
					SendMessage("Hello from Godot client!");
					break;

				case ENetConnection.EventType.Disconnect:
					GD.Print($"Disconnected from server (data: {data})");
					_connected = false;
					_serverPeer = null;
					break;

				case ENetConnection.EventType.Receive:
					var packet = peer.GetPacket();
					if (channel == 1)
					{
						string json = Encoding.UTF8.GetString(packet);
						var snapshot = JsonSerializer.Deserialize<ServerSnapshot>(json);
						_player.ApplyServerCorrection(
							new Vector3(snapshot.X, snapshot.Y, snapshot.Z),
							new Vector3(snapshot.VelocityX, snapshot.VelocityY, snapshot.VelocityZ),
							snapshot.LastClientTick);
					}
					break;
			}

			events = _client.Service(0);
		}
	}

	private void OnPlayerIntent(PlayerIntent intent)
	{
		if (!_connected || _serverPeer == null) return;

		_recentIntents.Enqueue(intent);
		while (_recentIntents.Count > RedundantInputCount)
			_recentIntents.Dequeue();

		string json = JsonSerializer.Serialize(_recentIntents.ToArray());
		byte[] data = Encoding.UTF8.GetBytes(json);
		_serverPeer.Send(1, data, 0);
		ServiceENet();
	}

	private void SendMessage(string message)
	{
		if (!_connected || _serverPeer == null) return;

		byte[] data = Encoding.UTF8.GetBytes(message);
		_serverPeer.Send(0, data, (int)ENetPacketPeer.FlagReliable);
		GD.Print($"Sent to server: {message}");
	}

	public override void _ExitTree()
	{
		if (_serverPeer != null && _connected)
		{
			_serverPeer.PeerDisconnect(0);
		}
		_client?.Destroy();
	}
}
