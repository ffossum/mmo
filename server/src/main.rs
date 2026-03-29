extern crate enet;

use std::collections::HashMap;
use std::net::Ipv4Addr;

use anyhow::Context;
use enet::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PlayerIntent {
    tick: i32,
    move_x: f32,
    move_z: f32,
    yaw: f32,
    jump: bool,
}

#[derive(Debug, Default)]
struct PlayerState {
    position: [f32; 3],
    velocity: [f32; 3],
    yaw: f32,
}

fn main() -> anyhow::Result<()> {
    let enet = Enet::new().context("could not initialize ENet")?;

    let local_addr = Address::new(Ipv4Addr::UNSPECIFIED, 9001);

    let mut host = enet
        .create_host::<()>(
            Some(&local_addr),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .context("could not create host")?;

    println!(
        "Server listening on {hostname}:{port}",
        hostname = local_addr.ip(),
        port = local_addr.port()
    );

    // Track connected players keyed by peer address (ip:port string).
    // We use the address string because enet-rs doesn't expose a stable peer ID.
    let mut players: HashMap<String, PlayerState> = HashMap::new();

    loop {
        match host.service(100).context("service failed")? {
            Some(Event::Connect(ref peer)) => {
                let addr = format!("{:?}", peer.address());
                println!("New connection from: {}", addr);
                players.insert(addr, PlayerState::default());
            }
            Some(Event::Disconnect(ref peer, _)) => {
                let addr = format!("{:?}", peer.address());
                println!("Disconnected: {}", addr);
                players.remove(&addr);
            }
            Some(Event::Receive {
                ref sender,
                channel_id,
                ref packet,
                ..
            }) => {
                let addr = format!("{:?}", sender.address());
                let data = packet.data();

                if channel_id == 1 {
                    // Movement channel — deserialize PlayerIntent
                    match serde_json::from_slice::<PlayerIntent>(data) {
                        Ok(intent) => {
                            if let Some(state) = players.get_mut(&addr) {
                                state.velocity[0] = intent.move_x;
                                state.velocity[2] = intent.move_z;
                                state.yaw = intent.yaw;
                                println!(
                                    "Player {} tick={} move=({}, {}) yaw={:.2} jump={}",
                                    addr, intent.tick, intent.move_x, intent.move_z, intent.yaw, intent.jump
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse PlayerIntent from {}: {}", addr, e);
                        }
                    }
                } else {
                    let message = std::str::from_utf8(data).unwrap_or("<invalid utf8>");
                    println!(
                        "Received from {} on channel {}: '{}'",
                        addr, channel_id, message
                    );
                }
            }
            _ => (),
        }
    }
}
