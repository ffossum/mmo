mod network;
mod physics;

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;

use network::{NetworkPlugin, PlayerConnected, PlayerDisconnected, PlayerInputReceived};
use physics::{Player, PhysicsPlugin, PlayerInputBuffer, player_bundle};

/// How often the schedule runner ticks. Network polling happens on every Update,
/// so we want this fast enough that ENet events aren't queued for long; the fixed
/// physics tick at 30 Hz still drives game state updates.
const FRAME_INTERVAL: Duration = Duration::from_millis(2);

fn main() {
    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(FRAME_INTERVAL)))
        .add_plugins(LogPlugin::default())
        .add_plugins(TransformPlugin)
        .add_plugins(NetworkPlugin)
        .add_plugins(PhysicsPlugin)
        .add_systems(
            FixedPreUpdate,
            (spawn_players, despawn_players, route_inputs),
        )
        .run();
}

fn spawn_players(mut commands: Commands, mut events: MessageReader<PlayerConnected>) {
    for event in events.read() {
        commands.spawn((Player(event.0), player_bundle()));
    }
}

fn despawn_players(
    mut commands: Commands,
    mut events: MessageReader<PlayerDisconnected>,
    players: Query<(Entity, &Player)>,
) {
    for event in events.read() {
        let id = event.0;
        if let Some((entity, _)) = players.iter().find(|(_, p)| p.0 == id) {
            commands.entity(entity).despawn();
        }
    }
}

fn route_inputs(
    mut events: MessageReader<PlayerInputReceived>,
    mut players: Query<(&Player, &mut PlayerInputBuffer)>,
) {
    for event in events.read() {
        let Some((_, mut input_buf)) = players.iter_mut().find(|(p, _)| p.0 == event.id) else {
            continue;
        };
        for intent in &event.intents {
            let newest = input_buf
                .queue
                .back()
                .map(|i| i.tick)
                .unwrap_or(input_buf.last_client_tick);
            if intent.tick > newest {
                input_buf.queue.push_back(intent.clone());
            }
        }
    }
}

