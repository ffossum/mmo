# Movement & Physics Architecture

## Overview

The game uses an **authoritative server** model for movement and physics. The server is the single source of truth for all entity positions and physics state. Clients never trust their own simulation as final -- they send inputs to the server, which simulates the world and sends back the canonical state.

This prevents cheating (clients cannot teleport or clip through walls) and ensures all players see a consistent world. The tradeoff is added complexity on the client to hide the latency between sending an input and receiving the server's response.

## Tick-Based Simulation

Both the server and client run a fixed-rate simulation loop (a "tick"). Each tick:

1. Inputs are collected (or received, on the server side).
2. Physics and movement are stepped forward by a fixed delta.
3. State is recorded or broadcast.

Using a fixed tick rate (rather than variable frame-time) ensures determinism -- the same sequence of inputs produces the same result regardless of frame rate or machine speed. The rendering frame rate is decoupled from the simulation tick rate.

## Client-Side Prediction

Waiting for the server to confirm every movement input before updating the local player's position would make the game feel sluggish. Client-side prediction solves this by letting the client **immediately simulate its own inputs locally**, so the player sees responsive movement without waiting for a round trip.

When the player presses a movement key, the client:

1. Records the input along with the current tick number.
2. Applies the input to the local player's physics state using the same simulation logic the server uses.
3. Renders the predicted result immediately.
4. Sends the input (tagged with its tick number) to the server.

The client stores a buffer of recent inputs and their resulting states so it can correct itself later if the server disagrees.

## Server Reconciliation

When the server processes an input and sends back the authoritative state for a given tick, the client compares that state to what it predicted for the same tick.

- **If they match**, no correction is needed -- the prediction was accurate.
- **If they differ**, the client snaps its state back to the server's authoritative state for that tick, then **replays all inputs that occurred after that tick** on top of the corrected state. This brings the client back up to the present with a corrected position, without discarding inputs the server hasn't processed yet.

This replay step is why the client keeps a buffer of recent inputs. The correction is invisible to the player in most cases, since the replayed inputs quickly converge back to where the player expects to be.

## Input Replication

Inputs are sent to the server as **actions and tick numbers**, not as positions or velocities. The client tells the server "on tick N, the player pressed forward" rather than "the player is now at position X." This keeps the server authoritative -- it applies the input to its own simulation and decides the outcome.

Key considerations:

- **Redundancy**: Inputs are sent redundantly (the client includes the last several ticks of input in each packet) so that a single dropped packet doesn't cause a missed input.
- **Ordering**: Each input is tagged with a tick number so the server can process them in the correct order, even if packets arrive out of order.
- **Compactness**: Inputs are small (a direction vector and a set of action flags), keeping bandwidth low even at high tick rates.

## Entity Interpolation

Client-side prediction only applies to the **local player**. Other players' positions come from the server, which means they arrive late and at a lower rate than the local frame rate.

Rendering remote entities directly at their last-known server position would look choppy. Instead, the client **interpolates** between the two most recent server snapshots for each remote entity, rendering them at a position slightly in the past but moving smoothly.

This introduces a small visual delay for remote entities (typically one snapshot interval), but produces smooth motion without predicting other players' inputs. The alternative -- **extrapolation** -- guesses where a remote entity is heading and corrects when the next snapshot arrives, which can cause visible pops. Interpolation is preferred for most cases; extrapolation may be used selectively when the visual delay is unacceptable.
