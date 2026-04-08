use anyhow::Context;
use nalgebra::{Matrix4, Point3};
use rapier3d::control::{CharacterLength, KinematicCharacterController};
use rapier3d::parry::query::DefaultQueryDispatcher;
use rapier3d::prelude::*;

// Matches client CapsuleShape3D: height=1.8, radius=0.2
// Rapier's capsule_y half_height is half the segment length (excluding hemispheres)
const PLAYER_HALF_HEIGHT: f32 = 0.7;
const PLAYER_RADIUS: f32 = 0.2;
const COLLIDER_OFFSET: f32 = PLAYER_HALF_HEIGHT + PLAYER_RADIUS;

pub struct PlayerBody {
    body_handle: RigidBodyHandle,
    velocity: Vector,
}

pub struct PhysicsWorld {
    gravity: Vector,
    dt: f32,
    pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    character_controller: KinematicCharacterController,
    character_shape: SharedShape,
}

impl PhysicsWorld {
    pub fn new(dt: Real) -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.8, 0.0),
            dt,
            pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            character_controller: KinematicCharacterController {
                offset: CharacterLength::Absolute(0.001),
                snap_to_ground: Some(CharacterLength::Absolute(0.1)),
                ..Default::default()
            },
            character_shape: SharedShape::capsule_y(PLAYER_HALF_HEIGHT, PLAYER_RADIUS),
        }
    }

    pub fn load_collision(&mut self, path: &str) -> anyhow::Result<usize> {
        let (doc, buffers, _) = gltf::import(path)
            .with_context(|| format!("failed to load collision mesh: {}", path))?;

        let mut count = 0;
        for scene in doc.scenes() {
            for node in scene.nodes() {
                count += self.load_node(&node, &buffers, &Matrix4::identity())?;
            }
        }

        Ok(count)
    }

    fn load_node(
        &mut self,
        node: &gltf::Node,
        buffers: &[gltf::buffer::Data],
        parent_transform: &Matrix4<f32>,
    ) -> anyhow::Result<usize> {
        let m = node.transform().matrix();
        let local = Matrix4::from_columns(&[m[0].into(), m[1].into(), m[2].into(), m[3].into()]);
        let global = parent_transform * local;

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

                let vertices: Vec<Vector> = positions
                    .map(|p| {
                        let transformed = global.transform_point(&Point3::from(p));
                        Vec3::new(transformed.x, transformed.y, transformed.z)
                    })
                    .collect();

                let tri_indices: Vec<[u32; 3]> = indices
                    .into_u32()
                    .collect::<Vec<u32>>()
                    .chunks(3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect();

                let body = self.rigid_body_set.insert(RigidBodyBuilder::fixed());
                self.collider_set.insert_with_parent(
                    ColliderBuilder::trimesh(vertices, tri_indices)?,
                    body,
                    &mut self.rigid_body_set,
                );
                count += 1;
            }
        }

        for child in node.children() {
            count += self.load_node(&child, buffers, &global)?;
        }

        Ok(count)
    }

    pub fn add_player(&mut self) -> PlayerBody {
        let body_handle = self.rigid_body_set.insert(
            RigidBodyBuilder::kinematic_position_based().translation(Vec3::new(0.0, 2.0, 0.0)),
        );
        self.collider_set.insert_with_parent(
            ColliderBuilder::capsule_y(PLAYER_HALF_HEIGHT, PLAYER_RADIUS).translation(Vec3::new(
                0.0,
                COLLIDER_OFFSET,
                0.0,
            )),
            body_handle,
            &mut self.rigid_body_set,
        );
        PlayerBody {
            body_handle,
            velocity: Vec3::ZERO,
        }
    }

    pub fn remove_player(&mut self, player: PlayerBody) {
        self.rigid_body_set.remove(
            player.body_handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
    }

    pub fn update_player(&mut self, player: &mut PlayerBody, move_x: f32, move_z: f32) {
        let speed = 5.0_f32;

        let Some(body) = self.rigid_body_set.get(player.body_handle) else {
            return;
        };

        player.velocity.y += self.gravity.y * self.dt;

        let horizontal = nalgebra::Vector2::new(move_x, move_z);
        let clamped = if horizontal.norm() > 1.0 {
            horizontal.normalize()
        } else {
            horizontal
        };
        player.velocity.x = clamped.x * speed;
        player.velocity.z = clamped.y * speed;

        let desired = player.velocity * self.dt;

        let mut shape_pos = *body.position();
        shape_pos.translation.y += COLLIDER_OFFSET;

        let query_pipeline = self.broad_phase.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.rigid_body_set,
            &self.collider_set,
            QueryFilter::default().exclude_rigid_body(player.body_handle),
        );
        let movement = self.character_controller.move_shape(
            self.dt,
            &query_pipeline,
            self.character_shape.as_ref(),
            &shape_pos,
            desired,
            |_| {},
        );

        if movement.grounded {
            player.velocity.y = 0.0;
        }

        if let Some(body) = self.rigid_body_set.get_mut(player.body_handle) {
            let new_pos = body.position().translation + movement.translation;
            body.set_next_kinematic_translation(new_pos);
        }
    }

    pub fn tick(&mut self) {
        self.pipeline.step(
            self.gravity,
            &IntegrationParameters {
                dt: self.dt,
                ..Default::default()
            },
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            &(),
            &(),
        );
    }

    pub fn get_position(&self, player: &PlayerBody) -> Option<[f32; 3]> {
        self.rigid_body_set.get(player.body_handle).map(|body| {
            let pos = body.translation();
            [pos.x, pos.y, pos.z]
        })
    }

    pub fn get_velocity(&self, player: &PlayerBody) -> [f32; 3] {
        [player.velocity.x, player.velocity.y, player.velocity.z]
    }
}
