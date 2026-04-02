use anyhow::Context;
use nalgebra::{Matrix4, Point3};
use rapier3d::prelude::*;

pub struct PhysicsWorld {
    gravity: Vector<f32>,
    integration_parameters: IntegrationParameters,
    pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    rigid_body_set: RigidBodySet,
    collider_set: ColliderSet,
    impulse_joint_set: ImpulseJointSet,
    multibody_joint_set: MultibodyJointSet,
    ccd_solver: CCDSolver,
    player_half_height: f32,
    player_radius: f32,
}

impl PhysicsWorld {
    pub fn new() -> Self {
        Self {
            gravity: vector![0.0, -9.81, 0.0],
            integration_parameters: IntegrationParameters {
                dt: 1.0 / 30.0,
                ..Default::default()
            },
            pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            rigid_body_set: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            player_half_height: 0.9,
            player_radius: 0.2,
        }
    }

    pub fn load_collision(&mut self, path: &str) -> anyhow::Result<usize> {
        let (doc, buffers, _) = gltf::import(path)
            .with_context(|| format!("failed to load collision mesh: {}", path))?;

        let mut count = 0;
        for scene in doc.scenes() {
            for node in scene.nodes() {
                count += self.load_node(&node, &buffers, &Matrix4::identity());
            }
        }

        Ok(count)
    }

    fn load_node(
        &mut self,
        node: &gltf::Node,
        buffers: &[gltf::buffer::Data],
        parent_transform: &Matrix4<f32>,
    ) -> usize {
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

                let vertices: Vec<Point<f32>> = positions
                    .map(|p| {
                        let transformed = global.transform_point(&Point3::from(p));
                        point![transformed.x, transformed.y, transformed.z]
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
                    ColliderBuilder::trimesh(vertices, tri_indices),
                    body,
                    &mut self.rigid_body_set,
                );
                count += 1;
            }
        }

        for child in node.children() {
            count += self.load_node(&child, buffers, &global);
        }

        count
    }

    pub fn add_player(&mut self) -> RigidBodyHandle {
        let body_handle = self
            .rigid_body_set
            .insert(RigidBodyBuilder::dynamic().translation(vector![0.0, 2.0, 0.0]));
        self.collider_set.insert_with_parent(
            ColliderBuilder::capsule_y(self.player_half_height, self.player_radius),
            body_handle,
            &mut self.rigid_body_set,
        );
        body_handle
    }

    pub fn remove_player(&mut self, handle: RigidBodyHandle) {
        self.rigid_body_set.remove(
            handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            true,
        );
    }

    pub fn tick(&mut self, player_inputs: impl Iterator<Item = (RigidBodyHandle, f32, f32)>) {
        let speed = 5.0_f32;
        for (handle, move_x, move_z) in player_inputs {
            if let Some(body) = self.rigid_body_set.get_mut(handle) {
                body.set_linvel(
                    vector![move_x * speed, body.linvel().y, move_z * speed],
                    true,
                );
            }
        }

        self.step();

        #[cfg(debug_assertions)]
        for (handle, body) in self.rigid_body_set.iter() {
            if body.is_dynamic() && body.is_moving() {
                let pos = body.translation();
                println!(
                    "Body {:?} pos=({:.2}, {:.2}, {:.2})",
                    handle, pos.x, pos.y, pos.z
                );
            }
        }
    }

    fn step(&mut self) {
        self.pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.rigid_body_set,
            &mut self.collider_set,
            &mut self.impulse_joint_set,
            &mut self.multibody_joint_set,
            &mut self.ccd_solver,
            None,
            &(),
            &(),
        );
    }
}
