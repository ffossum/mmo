# Next steps

This is a list of things to implement in the project, mostly meant as a reminder
for the developer.

1.  [x] **Track connected players and their state on the server.**

    The server currently has no concept of players — it receives packets and
    echoes them back. Add a data structure (e.g. a HashMap keyed by peer ID)
    that stores each connected player's current position, velocity, and yaw.
    Create and remove entries on connect/disconnect events. Deserialize incoming
    `PlayerIntent` messages into this state instead of echoing raw bytes. This
    is a prerequisite for everything below — the server needs to know who is
    connected and where they are before it can simulate physics or broadcast
    state.

1.  [x] **Add Jolt physics engine to the Rust back-end.**

    Integrate a Jolt physics binding (e.g. `rolt` or similar crate) into the
    server. Initialize a Jolt physics world on startup and step it at a fixed
    tick rate (30 Hz, matching the client). Create a dynamic body for each
    connected player and a static body for the ground plane. No gameplay logic
    yet — the goal is just to have Jolt running, stepping, and producing
    positions for player bodies each tick.

    **Update:** We added Rapier physics instead, since it is written in Rust and
    is more ergonomic to use. We'll evaluate later whether we should also change
    the client physics to Rapier, or if Jolt's behavior is close enough so it
    doesn't matter.

1.  [x] **Get collision meshes from client scenes for server physics.**

    Export the static collision geometry (terrain, walls, floors) from the Godot
    client scenes into a format the server can load (e.g. `.glb`/`.gltf` meshes
    or a custom binary format). On server startup, parse these meshes and
    register them as static triangle mesh bodies in the physics world. This
    replaces the temporary ground plane from step 2 so the server simulates
    against the same world geometry the client renders.

1.  [x] **Change player movement message format to match physics engine input.**

    Replace the current `PlayerIntent` JSON format with whatever input the
    physics engine expects. This is in order to simplify the code, and eliminate
    the need to convert data.

    We'll keep using JSON as the wire format for now, to keep things simple and
    readable.

    **Update**: This turned out to be a no-op. Our existing format was already
    ideal for the physics engine.

1.  [x] **Server-authoritative movement.**

    Each server tick: apply the latest received player input to that player's
    character controller, step the physics world, then read back the resulting
    position and velocity for each player. Send each player their own
    authoritative position after each tick.

    The client must accept the server's position as ground truth — if the
    client's local position diverges from what the server sent, snap to the
    server position. No smoothing or prediction yet; this will feel choppy but
    proves the authority model works.

1.  [x] **Client-side prediction and server reconciliation.**

    Use client-side prediction and server reconciliation to eliminate the choppy
    movement from the previous step. The client predicts movement locally every
    physics tick, stores input and position history in a ring buffer, and sends
    every input with a tick number. The server tags position responses with the
    last-processed tick. On receiving a server correction, the client compares
    its predicted position at that tick — if they diverge beyond a threshold, it
    snaps to the server position and replays all unconfirmed inputs.

1.  [x] **Input redundancy in player intent packets.**

    Each input packet currently contains a single tick's input. If that packet
    is lost (inputs are sent unreliably), the server substitutes idle input and
    the player hitches for a tick. Bundle the last 3 inputs in every packet so
    the server can recover from up to 2 consecutive packet losses. The server
    already rejects out-of-order and duplicate inputs (`intent.tick > newest`),
    so redundant inputs are naturally ignored.

1.  [ ] **Broadcast player states to all clients.**

    Each server tick (or at a lower rate like 10 Hz to save bandwidth), send
    every client a snapshot of all other players' positions and yaws. Each
    snapshot should include the server tick number and a list of
    `(player_id, x, y, z, yaw)` entries. Only include players whose state has
    changed since the last broadcast to that client. The client must be able to
    receive and store this data for rendering in the next step.

1.  [ ] **Spawn and despawn remote player entities on the client.**

    When the client receives a snapshot containing a player ID it hasn't seen
    before, instance a remote player scene (a copy of the player model without
    camera or input handling) at the received position. When a player
    disconnects (signaled by the server via a disconnect message or absence from
    snapshots for N ticks), remove their scene. Maintain a dictionary of
    `player_id -> Node` to track active remote players.

1.  [ ] **Interpolation for remote players.**

    Remote player positions arrive at the snapshot rate (e.g. 10 Hz) but the
    client renders at 60+ FPS. Buffer the two most recent snapshots for each
    remote player and interpolate between them over the snapshot interval. This
    means remote players render one snapshot interval behind real-time but move
    smoothly instead of teleporting between updates. Use linear interpolation
    for position and shortest-arc lerp for yaw.

1.  [ ] **Basic chat.**

    Add a text chat system as the first form of player interaction. The client
    sends chat messages to the server on a separate ENet channel (reliable). The
    server validates the message (non-empty, length-limited), prepends the
    sender's player ID or name, and broadcasts it to all connected clients. The
    client displays incoming chat messages in a scrollable UI panel. No
    channels, whispers, or history — just global broadcast chat.
