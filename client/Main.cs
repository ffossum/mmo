using Godot;
using System.Text;

public partial class Main : Node3D
{
	private ENetConnection _client;
	private ENetPacketPeer _serverPeer;
	private bool _connected = false;
	private double _timeSinceLastMessage = 0;
	private const double MessageInterval = 1.0;
	private const string ServerHost = "172.18.186.168";
	private const int ServerPort = 9001;

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
	}

	public override void _Process(double delta)
	{
		if (_client == null) return;

		// Service the ENet host to process events
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
					string message = Encoding.UTF8.GetString(packet);
					GD.Print($"Received from server: {message}");
					break;
			}

			events = _client.Service(0);
		}

		// Send periodic messages when connected
		if (_connected)
		{
			_timeSinceLastMessage += delta;
			if (_timeSinceLastMessage >= MessageInterval)
			{
				_timeSinceLastMessage = 0;
				SendMessage($"Client tick at {Time.GetTicksMsec()}ms");
			}
		}
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
