extern crate enet;

use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::time::{Duration, Instant};

use anyhow::Context;
use enet::*;
use rapier3d::prelude::*;
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

struct PlayerState {
    body_handle: RigidBodyHandle,
    yaw: f32,
}

fn main() -> anyhow::Result<()> {
    // --- Initialize Rapier ---
    let gravity = vector![0.0, -9.81, 0.0];
    let integration_parameters = IntegrationParameters {
        dt: 1.0 / 30.0,
        ..Default::default()
    };
    let mut physics_pipeline = PhysicsPipeline::new();
    let mut island_manager = IslandManager::new();
    let mut broad_phase = DefaultBroadPhase::new();
    let mut narrow_phase = NarrowPhase::new();
    let mut rigid_body_set = RigidBodySet::new();
    let mut collider_set = ColliderSet::new();
    let mut impulse_joint_set = ImpulseJointSet::new();
    let mut multibody_joint_set = MultibodyJointSet::new();
    let mut ccd_solver = CCDSolver::new();

    // Create ground plane (large flat box)
    let floor_body =
        rigid_body_set.insert(RigidBodyBuilder::fixed().translation(vector![0.0, -1.0, 0.0]));
    collider_set.insert_with_parent(
        ColliderBuilder::cuboid(100.0, 1.0, 100.0),
        floor_body,
        &mut rigid_body_set,
    );

    // Player capsule collider dimensions (shared config)
    let player_half_height = 0.75;
    let player_radius = 0.3;

    println!("Rapier physics initialized: floor ready");

    // --- Initialize ENet ---
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

    let tick_rate = Duration::from_secs_f64(1.0 / 30.0);
    let mut last_tick = Instant::now();

    loop {
        // --- Network ---
        match host.service(1).context("service failed")? {
            Some(Event::Connect(ref mut peer)) => {
                let id = next_id;
                next_id += 1;
                peer.set_data(Some(id));

                let body_handle = rigid_body_set
                    .insert(RigidBodyBuilder::dynamic().translation(vector![0.0, 2.0, 0.0]));
                collider_set.insert_with_parent(
                    ColliderBuilder::capsule_y(player_half_height, player_radius),
                    body_handle,
                    &mut rigid_body_set,
                );

                println!("Player {} connected from {:?}", id, peer.address());
                players.insert(
                    id,
                    PlayerState {
                        body_handle,
                        yaw: 0.0,
                    },
                );
            }
            Some(Event::Disconnect(ref peer, _)) => {
                if let Some(&id) = peer.data() {
                    if let Some(state) = players.remove(&id) {
                        rigid_body_set.remove(
                            state.body_handle,
                            &mut island_manager,
                            &mut collider_set,
                            &mut impulse_joint_set,
                            &mut multibody_joint_set,
                            true,
                        );
                        println!("Player {} disconnected, body removed", id);
                    }
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
                                state.yaw = intent.yaw;

                                let speed = 5.0_f32;
                                if let Some(body) = rigid_body_set.get_mut(state.body_handle) {
                                    body.set_linvel(
                                        vector![
                                            intent.move_x * speed,
                                            body.linvel().y,
                                            intent.move_z * speed
                                        ],
                                        true,
                                    );
                                }
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

        // --- Physics tick ---
        if last_tick.elapsed() >= tick_rate {
            last_tick += tick_rate;

            physics_pipeline.step(
                &gravity,
                &integration_parameters,
                &mut island_manager,
                &mut broad_phase,
                &mut narrow_phase,
                &mut rigid_body_set,
                &mut collider_set,
                &mut impulse_joint_set,
                &mut multibody_joint_set,
                &mut ccd_solver,
                None,
                &(),
                &(),
            );

            for (&id, state) in &players {
                if let Some(body) = rigid_body_set.get(state.body_handle) {
                    let pos = body.translation();
                    println!(
                        "Player {} pos=({:.2}, {:.2}, {:.2})",
                        id, pos.x, pos.y, pos.z
                    );
                }
            }
        }
    }
}
