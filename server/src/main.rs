mod network;
mod physics;

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use network::{Network, PlayerId, PlayerIntent, PlayerSnapshot, ServerEvent};
use physics::{PhysicsWorld, PlayerBody};

struct PlayerState {
    body: PlayerBody,
    yaw: f32,
    input_queue: VecDeque<PlayerIntent>,
    last_client_tick: i32,
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
                    last_client_tick: 0,
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
                        .unwrap_or(state.last_client_tick);
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
    let dt = 1.0_f32 / 30.0_f32;
    let tick_duration = Duration::from_secs_f32(dt);

    let mut physics = PhysicsWorld::new(dt);
    let count = physics.load_collision(COLLISION_MESH_PATH)?;
    println!(
        "Loaded {} collision mesh(es) from {}",
        count, COLLISION_MESH_PATH
    );

    let enet = enet::Enet::new().map_err(|e| anyhow::anyhow!("{}", e))?;
    let mut network = Network::new(&enet, 9001)?;

    let mut players: HashMap<PlayerId, PlayerState> = HashMap::new();

    let mut last_tick_time = Instant::now();
    let mut server_tick: i32 = 0;

    loop {
        // Handle all events within the tick
        while last_tick_time.elapsed() < tick_duration {
            let timeout_ms = tick_duration
                .saturating_sub(last_tick_time.elapsed())
                .as_millis() as u32;
            handle_event(&mut network, &mut physics, &mut players, timeout_ms)?;
        }
        last_tick_time += tick_duration;
        server_tick += 1;

        // Move physics simulation one step forward
        for state in players.values_mut() {
            let (move_x, move_z, jump) = if let Some(input) = state.input_queue.pop_front() {
                state.last_client_tick = input.tick;
                (input.move_x, input.move_z, input.jump)
            } else {
                (0.0, 0.0, false)
            };
            physics.update_player(&mut state.body, move_x, move_z, jump);
        }
        physics.tick();

        // Send state to clients
        for (&id, state) in &players {
            if let Some(pos) = physics.get_position(&state.body) {
                let vel = physics.get_velocity(&state.body);
                network.send_snapshot(
                    id,
                    &PlayerSnapshot {
                        x: pos[0],
                        y: pos[1],
                        z: pos[2],
                        velocity_x: vel[0],
                        velocity_y: vel[1],
                        velocity_z: vel[2],
                        server_tick,
                        last_client_tick: state.last_client_tick,
                    },
                );
            }
        }
    }
}
