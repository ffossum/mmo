extern crate enet;

use std::collections::HashMap;
use std::ffi::CStr;
use std::net::Ipv4Addr;
use std::ptr;
use std::time::{Duration, Instant};

use anyhow::Context;
use enet::*;
use joltc_sys::*;
use rolt::{
    BodyId, BroadPhaseLayer, BroadPhaseLayerInterface, ObjectLayer, ObjectLayerPairFilter,
    ObjectVsBroadPhaseLayerFilter,
};
use serde::Deserialize;

type PlayerId = u32;

// --- Jolt layer setup ---

const OL_NON_MOVING: JPC_ObjectLayer = 0;
const OL_MOVING: JPC_ObjectLayer = 1;

const BPL_NON_MOVING: JPC_BroadPhaseLayer = 0;
const BPL_MOVING: JPC_BroadPhaseLayer = 1;
const BPL_COUNT: JPC_BroadPhaseLayer = 2;

struct BroadPhaseLayers;

impl BroadPhaseLayerInterface for BroadPhaseLayers {
    fn get_num_broad_phase_layers(&self) -> u32 {
        BPL_COUNT as u32
    }

    fn get_broad_phase_layer(&self, layer: ObjectLayer) -> BroadPhaseLayer {
        match layer.raw() {
            OL_NON_MOVING => BroadPhaseLayer::new(BPL_NON_MOVING),
            OL_MOVING => BroadPhaseLayer::new(BPL_MOVING),
            _ => unreachable!(),
        }
    }
}

struct ObjectVsBroadPhase;

impl ObjectVsBroadPhaseLayerFilter for ObjectVsBroadPhase {
    fn should_collide(&self, layer1: ObjectLayer, layer2: BroadPhaseLayer) -> bool {
        match layer1.raw() {
            OL_NON_MOVING => layer2.raw() == BPL_MOVING,
            OL_MOVING => true,
            _ => unreachable!(),
        }
    }
}

struct ObjectLayerPair;

impl ObjectLayerPairFilter for ObjectLayerPair {
    fn should_collide(&self, layer1: ObjectLayer, layer2: ObjectLayer) -> bool {
        match layer1.raw() {
            OL_NON_MOVING => layer2.raw() == OL_MOVING,
            OL_MOVING => true,
            _ => unreachable!(),
        }
    }
}

// --- Data types ---

#[derive(Debug, Deserialize)]
struct PlayerIntent {
    tick: i32,
    move_x: f32,
    move_z: f32,
    yaw: f32,
    jump: bool,
}

struct PlayerState {
    body_id: BodyId,
    yaw: f32,
}

// --- Shape helpers ---

fn create_box_shape(half_x: f32, half_y: f32, half_z: f32) -> anyhow::Result<*mut JPC_Shape> {
    let settings = JPC_BoxShapeSettings {
        HalfExtent: JPC_Vec3 {
            x: half_x,
            y: half_y,
            z: half_z,
            _w: half_z,
        },
        ..Default::default()
    };
    let mut shape: *mut JPC_Shape = ptr::null_mut();
    let mut err: *mut JPC_String = ptr::null_mut();
    unsafe {
        if JPC_BoxShapeSettings_Create(&settings, &mut shape, &mut err) {
            Ok(shape)
        } else {
            let msg = CStr::from_ptr(JPC_String_c_str(err))
                .to_string_lossy()
                .into_owned();
            anyhow::bail!("Failed to create box shape: {}", msg);
        }
    }
}

fn create_capsule_shape(half_height: f32, radius: f32) -> anyhow::Result<*mut JPC_Shape> {
    let settings = JPC_CapsuleShapeSettings {
        HalfHeightOfCylinder: half_height,
        Radius: radius,
        ..Default::default()
    };
    let mut shape: *mut JPC_Shape = ptr::null_mut();
    let mut err: *mut JPC_String = ptr::null_mut();
    unsafe {
        if JPC_CapsuleShapeSettings_Create(&settings, &mut shape, &mut err) {
            Ok(shape)
        } else {
            let msg = CStr::from_ptr(JPC_String_c_str(err))
                .to_string_lossy()
                .into_owned();
            anyhow::bail!("Failed to create capsule shape: {}", msg);
        }
    }
}

fn main() -> anyhow::Result<()> {
    // --- Initialize Jolt ---
    rolt::register_default_allocator();
    rolt::factory_init();
    rolt::register_types();

    let (temp_allocator, job_system) = unsafe {
        let ta = JPC_TempAllocatorImpl_new(10 * 1024 * 1024);
        let js =
            JPC_JobSystemThreadPool_new2(JPC_MAX_PHYSICS_JOBS as _, JPC_MAX_PHYSICS_BARRIERS as _);
        (ta, js)
    };

    let mut physics = rolt::PhysicsSystem::new();
    physics.init(
        1024, // max bodies
        0,    // num body mutexes (0 = auto)
        1024, // max body pairs
        1024, // max contact constraints
        BroadPhaseLayers,
        ObjectVsBroadPhase,
        ObjectLayerPair,
    );

    let body_interface = physics.body_interface();

    // Create ground plane (large flat box)
    let floor_shape = create_box_shape(100.0, 1.0, 100.0)?;
    let floor_id = unsafe {
        let floor = body_interface.create_body(&JPC_BodyCreationSettings {
            Position: JPC_RVec3 {
                x: 0.0,
                y: -1.0,
                z: 0.0,
                _w: 0.0,
            },
            MotionType: JPC_MOTION_TYPE_STATIC,
            ObjectLayer: OL_NON_MOVING,
            Shape: floor_shape,
            ..Default::default()
        });
        floor.id()
    };
    body_interface.add_body(floor_id, JPC_ACTIVATION_DONT_ACTIVATE);

    physics.optimize_broad_phase();

    // Player capsule shape (shared by all players)
    let player_shape = create_capsule_shape(0.75, 0.3)?;

    println!("Jolt physics initialized: floor + player capsule shape ready");

    // --- Initialize ENet ---
    let enet = Enet::new().context("could not initialize ENet")?;
    let host_addr = Address::new(Ipv4Addr::UNSPECIFIED, 9001);

    let mut host = enet
        .create_host::<PlayerId>(
            Some(&host_addr),
            10,
            ChannelLimit::Maximum,
            BandwidthLimit::Unlimited,
            BandwidthLimit::Unlimited,
        )
        .context("could not create host")?;

    println!(
        "Server listening on {hostname}:{port}",
        hostname = host_addr.ip(),
        port = host_addr.port()
    );

    let mut players: HashMap<PlayerId, PlayerState> = HashMap::new();
    let mut next_id: PlayerId = 1;

    let tick_rate = Duration::from_secs_f64(1.0 / 30.0);
    let mut last_tick = Instant::now();

    loop {
        // --- Network ---
        match host.service(1).context("service failed")? {
            Some(Event::Connect(ref mut peer)) => {
                let id = next_id;
                next_id += 1;
                peer.set_data(Some(id));

                // Create a physics body for this player
                let body_id = unsafe {
                    let body = body_interface.create_body(&JPC_BodyCreationSettings {
                        Position: JPC_RVec3 {
                            x: 0.0,
                            y: 2.0,
                            z: 0.0,
                            _w: 0.0,
                        },
                        MotionType: JPC_MOTION_TYPE_DYNAMIC,
                        ObjectLayer: OL_MOVING,
                        Shape: player_shape,
                        ..Default::default()
                    });
                    body.id()
                };
                body_interface.add_body(body_id, JPC_ACTIVATION_ACTIVATE);

                println!("Player {} connected from {:?}", id, peer.address());
                players.insert(id, PlayerState { body_id, yaw: 0.0 });
            }
            Some(Event::Disconnect(ref peer, _)) => {
                if let Some(&id) = peer.data() {
                    if let Some(state) = players.remove(&id) {
                        body_interface.remove_body(state.body_id);
                        body_interface.destroy_body(state.body_id);
                        println!("Player {} disconnected, body removed", id);
                    }
                }
            }
            Some(Event::Receive {
                ref sender,
                channel_id,
                ref packet,
                ..
            }) => {
                let id = match sender.data() {
                    Some(&id) => id,
                    None => continue,
                };
                let data = packet.data();

                if channel_id == 1 {
                    match serde_json::from_slice::<PlayerIntent>(data) {
                        Ok(intent) => {
                            if let Some(state) = players.get_mut(&id) {
                                state.yaw = intent.yaw;

                                let speed = 5.0_f32;
                                body_interface.set_linear_velocity(
                                    state.body_id,
                                    rolt::Vec3::new(
                                        intent.move_x * speed,
                                        0.0,
                                        intent.move_z * speed,
                                    ),
                                );
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to parse PlayerIntent from player {}: {}", id, e);
                        }
                    }
                } else {
                    let message = std::str::from_utf8(data).unwrap_or("<invalid utf8>");
                    println!(
                        "Received from player {} on channel {}: '{}'",
                        id, channel_id, message
                    );
                }
            }
            _ => (),
        }

        // --- Physics tick ---
        if last_tick.elapsed() >= tick_rate {
            last_tick += tick_rate;

            unsafe {
                physics.update(1.0 / 30.0, 1, temp_allocator, job_system);
            }

            for (&id, state) in &players {
                let pos = body_interface.center_of_mass_position(state.body_id);
                println!(
                    "Player {} pos=({:.2}, {:.2}, {:.2})",
                    id, pos.x, pos.y, pos.z
                );
            }
        }
    }
}
