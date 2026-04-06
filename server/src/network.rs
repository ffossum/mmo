use std::net::Ipv4Addr;

use anyhow::Context;
use enet::*;
use serde::{Deserialize, Serialize};

pub type PlayerId = u32;

#[derive(Debug, Deserialize)]
pub struct PlayerIntent {
    pub tick: i32,
    pub move_x: f32,
    pub move_z: f32,
    pub yaw: f32,
    pub jump: bool,
}

pub enum ServerEvent {
    PlayerConnected(PlayerId),
    PlayerDisconnected(PlayerId),
    PlayerInput(PlayerId, Vec<PlayerIntent>),
    None,
}

#[derive(Serialize)]
pub struct PlayerPosition {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub last_tick: i32,
}

pub struct Network {
    host: Host<PlayerId>,
    next_id: PlayerId,
}

impl Network {
    pub fn new(enet: &Enet, port: u16) -> anyhow::Result<Self> {
        let host_addr = Address::new(Ipv4Addr::UNSPECIFIED, port);

        let host = enet
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

        Ok(Self { host, next_id: 1 })
    }

    pub fn poll(&mut self, timeout_ms: u32) -> anyhow::Result<ServerEvent> {
        match self.host.service(timeout_ms).context("service failed")? {
            Some(Event::Connect(ref mut peer)) => {
                let id = self.next_id;
                self.next_id += 1;
                peer.set_data(Some(id));
                println!("Player {} connected from {:?}", id, peer.address());
                Ok(ServerEvent::PlayerConnected(id))
            }
            Some(Event::Disconnect(ref peer, _)) => {
                if let Some(&id) = peer.data() {
                    println!("Player {} disconnected", id);
                    Ok(ServerEvent::PlayerDisconnected(id))
                } else {
                    Ok(ServerEvent::None)
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
                    None => return Ok(ServerEvent::None),
                };
                let data = packet.data();

                if channel_id == 1 {
                    match serde_json::from_slice::<Vec<PlayerIntent>>(data) {
                        Ok(intents) => Ok(ServerEvent::PlayerInput(id, intents)),
                        Err(e) => {
                            eprintln!("Failed to parse PlayerIntent from player {}: {}", id, e);
                            Ok(ServerEvent::None)
                        }
                    }
                } else {
                    let message = std::str::from_utf8(data).unwrap_or("<invalid utf8>");
                    println!(
                        "Received from player {} on channel {}: '{}'",
                        id, channel_id, message
                    );
                    Ok(ServerEvent::None)
                }
            }
            _ => Ok(ServerEvent::None),
        }
    }

    pub fn send_position(&mut self, player_id: PlayerId, pos: &PlayerPosition) {
        let json = serde_json::to_string(pos).unwrap();
        let data = json.as_bytes();

        for mut peer in self.host.peers() {
            if peer.state() == PeerState::Connected {
                if let Some(&id) = peer.data() {
                    if id == player_id {
                        let _ = peer.send_packet(
                            Packet::new(data, PacketMode::UnreliableUnsequenced).unwrap(),
                            1,
                        );
                        break;
                    }
                }
            }
        }
    }
}
