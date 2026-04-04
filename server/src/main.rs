mod network;
mod physics;

use std::collections::HashMap;
use std::time::{Duration, Instant};

use network::{Network, PlayerId, PlayerPosition, ServerEvent};
use physics::{PhysicsWorld, PlayerBody};

struct PlayerState {
    body: PlayerBody,
    yaw: f32,
    move_x: f32,
    move_z: f32,
}

const COLLISION_MESH_PATH: &str = "../shared/collision.glb";

fn main() -> anyhow::Result<()> {
    let mut physics = PhysicsWorld::new();
    let count = physics.load_collision(COLLISION_MESH_PATH)?;
    println!(
        "Loaded {} collision mesh(es) from {}",
        count, COLLISION_MESH_PATH
    );

    let enet = enet::Enet::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let mut network = Network::new(&enet, 9001)?;

    let mut players: HashMap<PlayerId, PlayerState> = HashMap::new();

    let tick_rate = Duration::from_secs_f64(1.0 / 30.0);
    let mut last_tick = Instant::now();

    loop {
        match network.poll()? {
            ServerEvent::PlayerConnected(id) => {
                let body = physics.add_player();
                players.insert(
                    id,
                    PlayerState {
                        body,
                        yaw: 0.0,
                        move_x: 0.0,
                        move_z: 0.0,
                    },
                );
            }
            ServerEvent::PlayerDisconnected(id) => {
                if let Some(state) = players.remove(&id) {
                    physics.remove_player(state.body);
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

            for state in players.values_mut() {
                physics.update_player(&mut state.body, state.move_x, state.move_z);
            }
            physics.tick();

            for (&id, state) in &players {
                if let Some(pos) = physics.get_position(&state.body) {
                    network.send_position(
                        id,
                        &PlayerPosition {
                            x: pos[0],
                            y: pos[1],
                            z: pos[2],
                        },
                    );
                }
            }
        }
    }
}
