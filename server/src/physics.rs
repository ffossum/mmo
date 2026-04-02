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
        let mut rigid_body_set = RigidBodySet::new();
        let mut collider_set = ColliderSet::new();

        // Create ground plane (large flat box)
        let floor_body =
            rigid_body_set.insert(RigidBodyBuilder::fixed().translation(vector![0.0, -1.0, 0.0]));
        collider_set.insert_with_parent(
            ColliderBuilder::cuboid(100.0, 1.0, 100.0),
            floor_body,
            &mut rigid_body_set,
        );

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
            rigid_body_set,
            collider_set,
            impulse_joint_set: ImpulseJointSet::new(),
            multibody_joint_set: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            player_half_height: 0.75,
            player_radius: 0.3,
        }
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

    pub fn set_player_velocity(&mut self, handle: RigidBodyHandle, vel_x: f32, vel_z: f32) {
        if let Some(body) = self.rigid_body_set.get_mut(handle) {
            body.set_linvel(vector![vel_x, body.linvel().y, vel_z], true);
        }
    }

    pub fn get_position(&self, handle: RigidBodyHandle) -> Option<[f32; 3]> {
        self.rigid_body_set
            .get(handle)
            .map(|body| {
                let pos = body.translation();
                [pos.x, pos.y, pos.z]
            })
    }

    pub fn step(&mut self) {
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
