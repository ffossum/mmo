use std::collections::VecDeque;

use anyhow::Context;
use bevy::math::Mat4;
use bevy::prelude::*;
use bevy_rapier3d::prelude::*;

use crate::network::{PlayerId, PlayerIntent, PlayerSnapshot, SendSnapshot};

/// Server tick rate. Must match the client's fixed timestep so reconciliation lines up.
pub const TICK_HZ: f64 = 30.0;
pub const FIXED_DT: f32 = 1.0 / TICK_HZ as f32;

// Matches client CapsuleShape3D: height=1.8, radius=0.2.
const PLAYER_HALF_HEIGHT: f32 = 0.7;
const PLAYER_RADIUS: f32 = 0.2;

const PLAYER_SPEED: f32 = 5.0;
const JUMP_SPEED: f32 = 4.5;
const GRAVITY: f32 = -9.8;

const COLLISION_MESH_PATH: &str = "../shared/collision.glb";

#[derive(Resource, Default)]
pub struct ServerTick(pub i32);

#[derive(Component, Debug, Clone, Copy)]
pub struct Player(pub PlayerId);

#[derive(Component, Default)]
pub struct PlayerYaw(pub f32);

#[derive(Component, Default)]
pub struct PlayerVelocity(pub Vec3);

#[derive(Component, Default)]
pub struct PlayerInputBuffer {
    pub queue: VecDeque<PlayerIntent>,
    pub last_client_tick: i32,
}

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(
            RapierPhysicsPlugin::<NoUserData>::default()
                .with_length_unit(1.0)
                .in_fixed_schedule(),
        )
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .init_resource::<ServerTick>()
        .add_systems(Startup, load_collision_mesh)
        .add_systems(FixedFirst, advance_tick)
        .add_systems(
            FixedUpdate,
            apply_player_inputs.before(PhysicsSet::SyncBackend),
        )
        .add_systems(FixedUpdate, queue_snapshots.after(PhysicsSet::Writeback));
    }
}

fn advance_tick(mut tick: ResMut<ServerTick>) {
    tick.0 = tick.0.wrapping_add(1);
}

fn load_collision_mesh(mut commands: Commands) {
    let count = match spawn_collision_entities(&mut commands, COLLISION_MESH_PATH) {
        Ok(c) => c,
        Err(err) => {
            error!("failed to load collision mesh: {:#}", err);
            return;
        }
    };
    info!(
        "loaded {} collision mesh(es) from {}",
        count, COLLISION_MESH_PATH
    );
}

fn spawn_collision_entities(commands: &mut Commands, path: &str) -> anyhow::Result<usize> {
    let (doc, buffers, _) =
        gltf::import(path).with_context(|| format!("failed to load collision mesh: {}", path))?;

    let mut count = 0;
    for scene in doc.scenes() {
        for node in scene.nodes() {
            count += spawn_node(commands, &node, &buffers, &Mat4::IDENTITY)?;
        }
    }
    Ok(count)
}

fn spawn_node(
    commands: &mut Commands,
    node: &gltf::Node,
    buffers: &[gltf::buffer::Data],
    parent_transform: &Mat4,
) -> anyhow::Result<usize> {
    let local = Mat4::from_cols_array_2d(&node.transform().matrix());
    let global = *parent_transform * local;

    let mut count = 0;

    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let Some(positions) = reader.read_positions() else {
                continue;
            };
            let Some(indices) = reader.read_indices() else {
                continue;
            };

            let vertices: Vec<Vec3> = positions
                .map(|p| global.transform_point3(Vec3::from(p)))
                .collect();

            let tri_indices: Vec<[u32; 3]> = indices
                .into_u32()
                .collect::<Vec<u32>>()
                .chunks(3)
                .map(|c| [c[0], c[1], c[2]])
                .collect();

            commands.spawn((
                RigidBody::Fixed,
                Collider::trimesh(vertices, tri_indices)?,
                Transform::default(),
            ));
            count += 1;
        }
    }

    for child in node.children() {
        count += spawn_node(commands, &child, buffers, &global)?;
    }

    Ok(count)
}

/// Components needed for a player avatar in the physics world.
/// Entity origin = player's feet, so snapshots send foot position directly.
pub fn player_bundle() -> impl Bundle {
    (
        Transform::from_xyz(0.0, 2.0, 0.0),
        RigidBody::KinematicPositionBased,
        Collider::capsule(
            Vec3::new(0.0, PLAYER_RADIUS, 0.0),
            Vec3::new(0.0, PLAYER_RADIUS + 2.0 * PLAYER_HALF_HEIGHT, 0.0),
            PLAYER_RADIUS,
        ),
        KinematicCharacterController {
            offset: CharacterLength::Absolute(0.001),
            snap_to_ground: Some(CharacterLength::Absolute(0.1)),
            ..Default::default()
        },
        PlayerYaw::default(),
        PlayerVelocity::default(),
        PlayerInputBuffer::default(),
    )
}

fn apply_player_inputs(
    mut players: Query<(
        &mut KinematicCharacterController,
        &mut PlayerVelocity,
        &mut PlayerInputBuffer,
        &mut PlayerYaw,
        Option<&KinematicCharacterControllerOutput>,
    )>,
) {
    for (mut controller, mut velocity, mut input_buf, mut yaw, output) in &mut players {
        // Reflects the post-move state from the previous fixed tick.
        let grounded = output.map(|o| o.grounded).unwrap_or(false);

        // Mirrors the original post-move "zero vertical velocity when grounded" step,
        // moved to the start of the next tick now that we read output from last step.
        if grounded {
            velocity.0.y = 0.0;
        } else {
            velocity.0.y += GRAVITY * FIXED_DT;
        }

        let (move_x, move_z, jump) = if let Some(intent) = input_buf.queue.pop_front() {
            input_buf.last_client_tick = intent.tick;
            yaw.0 = intent.yaw;
            (intent.move_x, intent.move_z, intent.jump)
        } else {
            (0.0, 0.0, false)
        };

        if jump && grounded {
            velocity.0.y = JUMP_SPEED;
        }

        let horizontal = Vec2::new(move_x, move_z);
        let clamped = if horizontal.length() > 1.0 {
            horizontal.normalize()
        } else {
            horizontal
        };
        if move_x != 0.0 || move_z != 0.0 {
            velocity.0.x = clamped.x * PLAYER_SPEED;
            velocity.0.z = clamped.y * PLAYER_SPEED;
        } else if grounded {
            velocity.0.x = 0.0;
            velocity.0.z = 0.0;
        }

        controller.translation = Some(velocity.0 * FIXED_DT);
    }
}

fn queue_snapshots(
    tick: Res<ServerTick>,
    mut outgoing: MessageWriter<SendSnapshot>,
    players: Query<(&Player, &Transform, &PlayerVelocity, &PlayerInputBuffer)>,
) {
    for (player, transform, velocity, input_buf) in &players {
        outgoing.write(SendSnapshot {
            id: player.0,
            snapshot: PlayerSnapshot {
                x: transform.translation.x,
                y: transform.translation.y,
                z: transform.translation.z,
                velocity_x: velocity.0.x,
                velocity_y: velocity.0.y,
                velocity_z: velocity.0.z,
                server_tick: tick.0,
                last_client_tick: input_buf.last_client_tick,
            },
        });
    }
}
