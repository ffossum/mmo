use std::net::Ipv4Addr;

use anyhow::Context;
use bevy::prelude::*;
use enet::{
    Address, BandwidthLimit, ChannelLimit, Enet, Event as EnetEvent, Host, Packet, PacketMode,
    PeerState,
};
use serde::{Deserialize, Serialize};

pub type PlayerId = u32;

const SERVER_PORT: u16 = 9001;
const MAX_PEERS: usize = 10;
const INPUT_CHANNEL: u8 = 1;

#[derive(Debug, Deserialize, Clone)]
pub struct PlayerIntent {
    pub tick: i32,
    pub move_x: f32,
    pub move_z: f32,
    pub yaw: f32,
    pub jump: bool,
}

#[derive(Serialize)]
pub struct PlayerSnapshot {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub velocity_z: f32,
    pub server_tick: i32,
    pub last_client_tick: i32,
}

#[derive(Message)]
pub struct PlayerConnected(pub PlayerId);

#[derive(Message)]
pub struct PlayerDisconnected(pub PlayerId);

#[derive(Message)]
pub struct PlayerInputReceived {
    pub id: PlayerId,
    pub intents: Vec<PlayerIntent>,
}

/// Outgoing snapshot to a single client. Emitted from the physics writeback stage,
/// drained by the network plugin's send system.
#[derive(Message)]
pub struct SendSnapshot {
    pub id: PlayerId,
    pub snapshot: PlayerSnapshot,
}

/// Owns the ENet library handle and host. `!Send` because the underlying FFI
/// pointers are not thread-safe, so it lives as a `NonSend` resource.
pub struct EnetState {
    _enet: Enet,
    host: Host<PlayerId>,
    next_id: PlayerId,
}

impl EnetState {
    fn new(port: u16) -> anyhow::Result<Self> {
        let enet = Enet::new().map_err(|e| anyhow::anyhow!("{}", e))?;
        let host_addr = Address::new(Ipv4Addr::UNSPECIFIED, port);
        let host = enet
            .create_host::<PlayerId>(
                Some(&host_addr),
                MAX_PEERS,
                ChannelLimit::Maximum,
                BandwidthLimit::Unlimited,
                BandwidthLimit::Unlimited,
            )
            .context("could not create host")?;

        info!(
            "Server listening on {hostname}:{port}",
            hostname = host_addr.ip(),
            port = host_addr.port()
        );

        Ok(Self {
            _enet: enet,
            host,
            next_id: 1,
        })
    }
}

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlayerConnected>()
            .add_message::<PlayerDisconnected>()
            .add_message::<PlayerInputReceived>()
            .add_message::<SendSnapshot>()
            .add_systems(Startup, init_network)
            // Poll and send on the fixed schedule so messages don't expire between
            // FixedUpdate runs (Bevy clears messages every ~2 Update frames).
            .add_systems(FixedFirst, poll_network)
            .add_systems(FixedLast, drain_outgoing);
    }
}

fn init_network(world: &mut World) {
    let state = EnetState::new(SERVER_PORT).expect("failed to initialize ENet");
    world.insert_non_send_resource(state);
}

fn poll_network(
    mut state: NonSendMut<EnetState>,
    mut connected: MessageWriter<PlayerConnected>,
    mut disconnected: MessageWriter<PlayerDisconnected>,
    mut inputs: MessageWriter<PlayerInputReceived>,
) {
    // Split-borrow the host and next_id so we can mutate next_id while iterating
    // events whose lifetime is tied to host.
    let EnetState {
        ref mut host,
        ref mut next_id,
        ..
    } = *state;

    loop {
        let mut event = match host.service(0) {
            Ok(Some(event)) => event,
            Ok(None) => return,
            Err(err) => {
                error!("enet service failed: {}", err);
                return;
            }
        };

        match event {
            EnetEvent::Connect(ref mut peer) => {
                let id = *next_id;
                *next_id += 1;
                peer.set_data(Some(id));
                info!("Player {} connected from {:?}", id, peer.address());
                connected.write(PlayerConnected(id));
            }
            EnetEvent::Disconnect(ref peer, _) => {
                if let Some(&id) = peer.data() {
                    info!("Player {} disconnected", id);
                    disconnected.write(PlayerDisconnected(id));
                }
            }
            EnetEvent::Receive {
                ref sender,
                channel_id,
                ref packet,
                ..
            } => {
                let Some(&id) = sender.data() else {
                    continue;
                };
                let data = packet.data();

                if channel_id == INPUT_CHANNEL {
                    match serde_json::from_slice::<Vec<PlayerIntent>>(data) {
                        Ok(parsed) => {
                            inputs.write(PlayerInputReceived {
                                id,
                                intents: parsed,
                            });
                        }
                        Err(e) => {
                            warn!("failed to parse PlayerIntent from player {}: {}", id, e);
                        }
                    }
                } else {
                    let message = std::str::from_utf8(data).unwrap_or("<invalid utf8>");
                    info!(
                        "received from player {} on channel {}: '{}'",
                        id, channel_id, message
                    );
                }
            }
        }
    }
}

fn drain_outgoing(mut state: NonSendMut<EnetState>, mut snapshots: MessageReader<SendSnapshot>) {
    for msg in snapshots.read() {
        let target = msg.id;
        let json = match serde_json::to_string(&msg.snapshot) {
            Ok(s) => s,
            Err(e) => {
                warn!("failed to serialize snapshot for {}: {}", target, e);
                continue;
            }
        };
        let data = json.as_bytes();

        for mut peer in state.host.peers() {
            if peer.state() == PeerState::Connected
                && let Some(&peer_id) = peer.data()
                && peer_id == target
            {
                let _ = peer.send_packet(
                    Packet::new(data, PacketMode::UnreliableUnsequenced).unwrap(),
                    INPUT_CHANNEL,
                );
                break;
            }
        }
    }
}
