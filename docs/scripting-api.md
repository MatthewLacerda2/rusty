# Scripting API reference

The stable surface available to Lua gameplay scripts, the console REPL, and
bot-players — the rusty equivalent of Unity's Scripting Reference. One surface,
three callers: anything documented here works identically in a `.lua` entity
script, a console line, and a headless harness scenario.

Entities are addressed by a numeric `id` (`u32`). `Vector3` values cross the
boundary as three `f32`s (`x, y, z`) rather than a table. Getters that miss
return a sensible default (zeros, or `(1,1,1)` for scale) instead of erroring.

> **Maintenance:** this file is currently **hand-written** and must be kept in
> sync with `src/scripting/mod.rs` and `src/scripting/bindings/`. A generator
> that emits it from self-describing bindings (with a CI drift check) is a tracked
> follow-up — see the PR for issue #28.

---

## `Transform`

Per-entity position / rotation (Euler degrees) / scale. Mutators re-sync the
entity's collider.

| Function | Signature | Returns |
|---|---|---|
| `Transform.GetPosition` | `(id)` | `x, y, z` |
| `Transform.SetPosition` | `(id, x, y, z)` | — |
| `Transform.GetRotation` | `(id)` | `x, y, z` (Euler degrees) |
| `Transform.SetRotation` | `(id, x, y, z)` | — |
| `Transform.GetScale` | `(id)` | `x, y, z` |
| `Transform.SetScale` | `(id, x, y, z)` | — |
| `Transform.MoveTowards` | `(id, tx, ty, tz, step)` | — (steps toward target, snaps within `step`) |

## `Material`

PBR material params and texture maps on an entity's renderer.

| Function | Signature |
|---|---|
| `Material.SetMetallic` | `(id, value)` |
| `Material.SetRoughness` | `(id, value)` |
| `Material.SetMetallicMap` | `(id, path)` |
| `Material.SetRoughnessMap` | `(id, path)` |
| `Material.SetTexture` | `(id, path)` |

## `Animator`

Drive an entity's animation clips.

| Function | Signature |
|---|---|
| `Animator.Play` | `(id, clip)` |
| `Animator.Crossfade` | `(id, clip, duration)` |
| `Animator.Stop` | `(id)` |

## `Input`

Read key state, and (writable half) inject input so a script can play as the
user. Keys are named strings (e.g. `"W"`, `"Space"`).

| Function | Signature | Returns |
|---|---|---|
| `Input.IsKeyDown` | `(key)` | `bool` |
| `Input.Press` | `(key)` | — |
| `Input.Release` | `(key)` | — |

## `Scene`

| Function | Signature | Returns |
|---|---|---|
| `Scene.FindEntityByName` | `(name)` | `id` (or `0` if none) |
| `Scene.DestroyEntity` | `(id)` | — |

## `Navigation`

| Function | Signature | Returns |
|---|---|---|
| `Navigation.GetNextPathStep` | `(cx, cy, cz, tx, ty, tz)` | next waypoint `x, y, z` along the A* path |

## `NavMeshAgent`

Per-entity navmesh agent control.

| Function | Signature | Returns |
|---|---|---|
| `NavMeshAgent.SetTarget` | `(id, x, y, z)` | — |
| `NavMeshAgent.GetTarget` | `(id)` | `x, y, z` |
| `NavMeshAgent.SetSpeed` | `(id, speed)` | — |
| `NavMeshAgent.SetAcceleration` | `(id, acceleration)` | — |
| `NavMeshAgent.SetStoppingDistance` | `(id, distance)` | — |
| `NavMeshAgent.SetRadius` | `(id, radius)` | — |
| `NavMeshAgent.IsAtTarget` | `(id)` | `bool` |
| `NavMeshAgent.GetVelocity` | `(id)` | `x, y, z` |
| `NavMeshAgent.SetActive` | `(id, active)` | — |

## `Physics`

Rigidbody control plus raycast queries. `Raycast`/`Shoot` return
`(hit, entity_id, distance)`; on a miss the id and distance are `0`.

| Function | Signature | Returns |
|---|---|---|
| `Physics.GetVelocity` | `(id)` | `x, y, z` |
| `Physics.SetVelocity` | `(id, vx, vy, vz)` | — |
| `Physics.AddForce` | `(id, fx, fy, fz)` | — |
| `Physics.SetKinematic` | `(id, is_kinematic)` | — |
| `Physics.Raycast` | `(ox, oy, oz, dx, dy, dz [, ignore_id [, layer_mask]])` | `hit, entity_id, distance` |
| `Physics.Shoot` | `(ox, oy, oz, dx, dy, dz, damage [, ignore_id [, layer_mask]])` | `hit, entity_id, distance` (applies damage on hit) |

The optional trailing `ignore_id` skips one entity in the cast — pass the shooter's
own id so a shot can't hit its source. The engine has no built-in "don't hit the
player" rule; the only thing it never hits is a dead entity.

The optional `layer_mask` is a Unity-style bitmask (one bit per layer): the cast
only hits entities whose layer's bit is set, ignoring all others. Build one from a
layer name with `1 << Layers.NameToIndex("Enemy")`, or OR several together. Omit it
(or pass `nil`) to hit every layer. This is independent of the **Layer Collision
Matrix** (Scene Settings), which governs which layers physically collide.

## `Health`

| Function | Signature | Returns |
|---|---|---|
| `Health.Get` | `(id)` | `current, max` |
| `Health.Set` | `(id, value)` | — (clamped to `[0, max]`) |
| `Health.Heal` | `(id, amount)` | — |
| `Health.Damage` | `(id, amount)` | — (plays death clip + logs at 0) |

## `Time`

Read-only clock accessors.

| Function | Signature | Returns |
|---|---|---|
| `Time.deltaTime` | `()` | seconds since last frame |
| `Time.fixedDeltaTime` | `()` | fixed-step seconds |
| `Time.frameCount` | `()` | frames since start |

## `Camera`

| Function | Signature | Returns |
|---|---|---|
| `Camera.GetPosition` | `()` | `x, y, z` |
| `Camera.SetPosition` | `(x, y, z)` | — |
| `Camera.GetForward` | `()` | `x, y, z` (unit look direction) |
| `Camera.GetRight` | `()` | `x, y, z` (unit right vector) |
| `Camera.GetYaw` / `SetYaw` | `()` / `(yaw)` | degrees |
| `Camera.GetPitch` / `SetPitch` | `()` / `(pitch)` | degrees (clamped ±89) |
| `Camera.GetFov` / `SetFov` | `()` / `(fov)` | degrees (clamped 1–179) |

## `Particles`

Drive an entity's particle emitter (`ParticleEmitterComponent`). `Emit`/`Burst`
spawn through the emitter's own seeded path, so scripted emission stays
deterministic in a headless replay. `Emit`/`Burst` return the number of particles
actually spawned (the `max_particles` cap may swallow some).

| Function | Signature | Returns |
|---|---|---|
| `Particles.Emit` | `(id, count)` | spawned count |
| `Particles.Burst` | `(id)` | spawned count (fires the configured `burst_count`) |
| `Particles.SetActive` | `(id, active)` | — |
| `Particles.SetRate` | `(id, rate)` | — (continuous particles/sec, clamped ≥ 0) |
| `Particles.IsActive` | `(id)` | `bool` |
| `Particles.GetCount` | `(id)` | live particle count |
| `Particles.Clear` | `(id)` | — (despawns all live particles) |

## `Layers`

An entity's layer is a single index (`0..31`) into the project's shared Layers
registry — Unity's per-object layer. Layer 0 is the fixed `"Default"`; the names
are managed in the **Tags & Layers** section of the Scene Settings panel and
persist with the scene. This is groundwork: a layer carries no collision or
rendering behaviour on its own yet (that arrives with the collision matrix and
camera culling masks).

| Function | Signature | Returns |
|---|---|---|
| `Layers.GetLayer` | `(id)` | layer index (`0` if the entity is missing) |
| `Layers.SetLayer` | `(id, index)` | — |
| `Layers.GetName` | `(index)` | the slot's name, or `Layer N` if unnamed |
| `Layers.NameToIndex` | `(name)` | layer index, or `nil` if unknown |

## `Storage`

A namespaced, JSON-backed key-value store that survives across runs — the engine's
PlayerPrefs analog, backed by `project/storage.json` (human-readable so an agent can
diff a save in a PR). A value may be a scalar **or** a structured table.

| Function | Signature | Returns |
|---|---|---|
| `Storage.Set` | `(namespace, key, value)` | — (value: number / string / bool / table) |
| `Storage.Get` | `(namespace, key)` | the value, or `nil` |
| `Storage.Has` | `(namespace, key)` | `bool` |
| `Storage.Delete` | `(namespace, key)` | `bool` (whether something was removed) |
| `Storage.GetTable` | `(namespace)` | the whole namespace as a table, or `nil` |
| `Storage.SetTable` | `(namespace, table)` | — (replace a whole namespace at once) |

**Determinism.** Reads resolve from an in-memory snapshot loaded once at a boundary
(startup); writes mutate that snapshot and are persisted only at boundaries (Stop /
quit), never inside `FixedUpdate`. The headless harness runs the store **pathless**,
so it never reads a developer's real save and replays stay reproducible.

## `Debug` — **dev builds only**

Mirrors Unity's `[Conditional]` `Debug`. Registered only under the `dev` Cargo
feature and stripped from ship builds; calling it from a ship build is a no-op
because the namespace is absent.

| Function | Signature |
|---|---|
| `Debug.Log` | `(message)` |
| `Debug.Warn` | `(message)` |
| `Debug.Error` | `(message)` |
