extern crate enet;

use std::collections::HashMap;
use std::net::Ipv4Addr;

use anyhow::Context;
use enet::*;
use serde::Deserialize;

type PlayerId = u32;

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

    let host_addr = Address::new(Ipv4Addr::UNSPECIFIED, 9001);

    let mut host = enet
        .create_host::<PlayerId>(
            Some(&host_addr),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .context("could not create host")?;

    println!(
        "Server listening on {hostname}:{port}",
        hostname = host_addr.ip(),
        port = host_addr.port()
    );

    let mut players: HashMap<PlayerId, PlayerState> = HashMap::new();
    let mut next_id: PlayerId = 1;

    loop {
        match host.service(100).context("service failed")? {
            Some(Event::Connect(ref mut peer)) => {
                let id = next_id;
                next_id += 1;
                peer.set_data(Some(id));
                println!("Player {} connected from {:?}", id, peer.address());
                players.insert(id, PlayerState::default());
            }
            Some(Event::Disconnect(ref peer, _)) => {
                if let Some(&id) = peer.data() {
                    println!("Player {} disconnected", id);
                    players.remove(&id);
                }
            }
            Some(Event::Receive {
                ref sender,
                channel_id,
                ref packet,
                ..
            }) => {
                let id = match sender.data() {
                    Some(&id) => id,
                    None => continue,
                };
                let data = packet.data();

                if channel_id == 1 {
                    match serde_json::from_slice::<PlayerIntent>(data) {
                        Ok(intent) => {
                            if let Some(state) = players.get_mut(&id) {
                                state.velocity[0] = intent.move_x;
                                state.velocity[2] = intent.move_z;
                                state.yaw = intent.yaw;
                                println!(
                                    "Player {} tick={} move=({}, {}) yaw={:.2} jump={}",
                                    id,
                                    intent.tick,
                                    intent.move_x,
                                    intent.move_z,
                                    intent.yaw,
                                    intent.jump
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse PlayerIntent from player {}: {}", id, e);
                        }
                    }
                } else {
                    let message = std::str::from_utf8(data).unwrap_or("<invalid utf8>");
                    println!(
                        "Received from player {} on channel {}: '{}'",
                        id, channel_id, message
                    );
                }
            }
            _ => (),
        }
    }
}
