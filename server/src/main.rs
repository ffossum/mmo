extern crate enet;

use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use anyhow::Context;
use enet::*;

fn main() -> anyhow::Result<()> {
    let enet = Enet::new().context("could not initialize ENet")?;

    let local_addr = Address::new(Ipv4Addr::LOCALHOST, 9001);

    let mut host = enet
        .create_host::<()>(
            Some(&local_addr),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .context("could not create host")?;

    println!("Server listening on 127.0.0.1:9001");

    let mut last_broadcast = Instant::now();
    let broadcast_interval = Duration::from_secs(1);
    let mut tick_count: u64 = 0;

    loop {
        // Service with a shorter timeout to allow for regular broadcasts
        match host.service(100).context("service failed")? {
            Some(Event::Connect(ref mut peer)) => {
                println!("New connection from: {:?}", peer.address());

                // Send welcome message to the new client
                let welcome = format!("Welcome to the server! You are connected.");
                peer.send_packet(
                    Packet::new(welcome.as_bytes(), PacketMode::ReliableSequenced)
                        .context("failed to create packet")?,
                    0,
                )
                .context("failed to send welcome packet")?;
            }
            Some(Event::Disconnect(ref peer, _)) => {
                println!("Disconnected: {:?}", peer.address());
            }
            Some(Event::Receive {
                ref mut sender,
                channel_id,
                ref packet,
                ..
            }) => {
                let message = std::str::from_utf8(packet.data()).unwrap_or("<invalid utf8>");
                println!(
                    "Received from {:?} on channel {}: '{}'",
                    sender.address(),
                    channel_id,
                    message
                );

                // Echo the message back with a prefix
                let response = format!("Server received: {}", message);
                sender
                    .send_packet(
                        Packet::new(response.as_bytes(), PacketMode::ReliableSequenced)
                            .context("failed to create packet")?,
                        0,
                    )
                    .context("failed to send response packet")?;
            }
            _ => (),
        }

        // Send periodic broadcast to all connected clients
        if last_broadcast.elapsed() >= broadcast_interval {
            last_broadcast = Instant::now();
            tick_count += 1;

            let broadcast_msg = format!("Server tick #{}", tick_count);

            for mut peer in host.peers() {
                if peer.state() == PeerState::Connected {
                    if let Err(e) = peer.send_packet(
                        Packet::new(broadcast_msg.as_bytes(), PacketMode::ReliableSequenced)
                            .context("failed to create broadcast packet")?,
                        0,
                    ) {
                        eprintln!("Failed to send broadcast to {:?}: {}", peer.address(), e);
                    }
                }
            }

            if tick_count % 10 == 0 {
                let connected_count = host
                    .peers()
                    .filter(|p| p.state() == PeerState::Connected)
                    .count();
                println!("Tick #{} - {} clients connected", tick_count, connected_count);
            }
        }
    }
}
