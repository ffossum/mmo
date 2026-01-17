using Godot;
using System;
using System.Text;
using ENet;

public partial class Main : Node3D
{
	private Host _client;
	private Peer _serverPeer;
	private bool _connected = false;
	private double _timeSinceLastMessage = 0;
	private const double MessageInterval = 1.0; // Send a message every second

	public override void _Ready()
	{
		Library.Initialize();

		_client = new Host();
		_client.Create();

		Address address = new Address();
		address.SetHost("127.0.0.1");
		address.Port = 9001;

		GD.Print("Connecting to server at 127.0.0.1:9001...");
		_serverPeer = _client.Connect(address, 2, 0);
	}

	public override void _Process(double delta)
	{
		if (_client == null) return;

		// Service the ENet host to process events
		Event enetEvent;
		while (_client.Service(0, out enetEvent) > 0)
		{
			switch (enetEvent.Type)
			{
				case EventType.Connect:
					GD.Print("Connected to server!");
					_connected = true;
					SendMessage("Hello from Godot client!");
					break;

				case EventType.Disconnect:
					GD.Print("Disconnected from server.");
					_connected = false;
					_serverPeer = default;
					break;

				case EventType.Receive:
					byte[] data = new byte[enetEvent.Packet.Length];
					enetEvent.Packet.CopyTo(data);
					string message = Encoding.UTF8.GetString(data);
					GD.Print($"Received from server: {message}");
					enetEvent.Packet.Dispose();
					break;
			}
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
		if (!_connected) return;

		byte[] data = Encoding.UTF8.GetBytes(message);
		Packet packet = new Packet();
		packet.Create(data, PacketFlags.Reliable);
		_serverPeer.Send(0, ref packet);
		GD.Print($"Sent to server: {message}");
	}

	public override void _ExitTree()
	{
		if (_connected)
		{
			_serverPeer.Disconnect(0);
		}
		_client?.Dispose();
		Library.Deinitialize();
	}
}
