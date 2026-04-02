mod network;
mod physics;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use network::{Network, PlayerId, ServerEvent};
use physics::PhysicsWorld;
use rapier3d::prelude::RigidBodyHandle;

struct PlayerState {
    body_handle: RigidBodyHandle,
    yaw: f32,
    move_x: f32,
    move_z: f32,
}

fn main() -> anyhow::Result<()> {
    let mut physics = PhysicsWorld::new();

    let enet = enet::Enet::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let mut network = Network::new(&enet, 9001)?;

    let mut players: HashMap<PlayerId, PlayerState> = HashMap::new();

    let tick_rate = Duration::from_secs_f64(1.0 / 30.0);
    let mut last_tick = Instant::now();

    loop {
        match network.poll()? {
            ServerEvent::PlayerConnected(id) => {
                let body_handle = physics.add_player();
                players.insert(
                    id,
                    PlayerState {
                        body_handle,
                        yaw: 0.0,
                        move_x: 0.0,
                        move_z: 0.0,
                    },
                );
            }
            ServerEvent::PlayerDisconnected(id) => {
                if let Some(state) = players.remove(&id) {
                    physics.remove_player(state.body_handle);
                }
            }
            ServerEvent::PlayerInput(id, intent) => {
                if let Some(state) = players.get_mut(&id) {
                    state.yaw = intent.yaw;
                    state.move_x = intent.move_x;
                    state.move_z = intent.move_z;
                }
            }
            ServerEvent::None => {}
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick += tick_rate;

            let speed = 5.0_f32;
            for state in players.values() {
                physics.set_player_velocity(
                    state.body_handle,
                    state.move_x * speed,
                    state.move_z * speed,
                );
            }

            physics.step();

            for (&id, state) in &players {
                if state.move_x != 0.0 || state.move_z != 0.0 {
                    if let Some(pos) = physics.get_position(state.body_handle) {
                        println!(
                            "Player {} pos=({:.2}, {:.2}, {:.2})",
                            id, pos[0], pos[1], pos[2]
                        );
                    }
                }
            }
        }
    }
}
