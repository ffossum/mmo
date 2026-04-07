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

fn handle_event(
    network: &mut Network,
    physics: &mut PhysicsWorld,
    players: &mut HashMap<PlayerId, PlayerState>,
    timeout_ms: u32,
) -> anyhow::Result<bool> {
    match network.poll(timeout_ms)? {
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
            Ok(true)
        }
        ServerEvent::PlayerDisconnected(id) => {
            if let Some(state) = players.remove(&id) {
                physics.remove_player(state.body);
            }
            Ok(true)
        }
        ServerEvent::PlayerInput(id, intents) => {
            if let Some(state) = players.get_mut(&id) {
                for intent in intents {
                    let newest = state
                        .input_queue
                        .back()
                        .map(|i| i.tick)
                        .unwrap_or(state.last_tick);
                    if intent.tick > newest {
                        state.yaw = intent.yaw;
                        state.input_queue.push_back(intent);
                    }
                }
            }
            Ok(true)
        }
        ServerEvent::None => Ok(false),
    }
}

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
        let remaining = tick_rate.saturating_sub(last_tick.elapsed());
        let timeout_ms = remaining.as_millis() as u32;
        handle_event(&mut network, &mut physics, &mut players, timeout_ms)?;

        if last_tick.elapsed() >= tick_rate {
            // Drain all pending events before physics tick
            while handle_event(&mut network, &mut physics, &mut players, 0)? {}

            last_tick += tick_rate;

            for state in players.values_mut() {
                let (move_x, move_z, jump) = if let Some(input) = state.input_queue.pop_front() {
                    state.last_tick = input.tick;
                    (input.move_x, input.move_z, input.jump)
                } else {
                    (0.0, 0.0, false)
                };
                if jump {
                    let pos = physics.get_position(&state.body).unwrap_or([0.0; 3]);
                    println!(
                        "Jump intent at tick {}: grounded={}, y={:.4}, queue_remaining={}",
                        state.last_tick, state.body.is_grounded(), pos[1], state.input_queue.len()
                    );
                }
                physics.update_player(&mut state.body, move_x, move_z, jump);
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
