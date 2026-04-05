mod network;
mod physics;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use network::{Network, PlayerId, PlayerIntent, PlayerPosition, ServerEvent};
use physics::{PhysicsWorld, PlayerBody};

struct PlayerState {
    body: PlayerBody,
    yaw: f32,
    input_queue: VecDeque<PlayerIntent>,
    last_tick: i32,
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
                        input_queue: VecDeque::new(),
                        last_tick: 0,
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
                    let newest = state.input_queue.back()
                        .map(|i| i.tick)
                        .unwrap_or(state.last_tick);
                    if intent.tick > newest {
                        state.yaw = intent.yaw;
                        state.input_queue.push_back(intent);
                    }
                }
            }
            ServerEvent::None => {}
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick += tick_rate;

            for state in players.values_mut() {
                let (move_x, move_z) = if let Some(input) = state.input_queue.pop_front() {
                    state.last_tick = input.tick;
                    (input.move_x, input.move_z)
                } else {
                    (0.0, 0.0)
                };
                physics.update_player(&mut state.body, move_x, move_z);
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
                            last_tick: state.last_tick,
                        },
                    );
                }
            }
        }
    }
}
