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

> **Faithfulness:** every setter here is expected to be *observed* by a downstream
> system — a renderer/sim read or a `SceneData` round-trip. When you add a setter,
> name its read-site (or add a round-trip test) in the same change and record it in
> [`api-faithfulness.md`](api-faithfulness.md), so the surface never grows a
> write-only no-op that silently fails headless authoring (issue #178).

---

## Driving a headless session

For agentic use there is a **fourth caller**: a long-lived, headless, **edit-mode**
engine process you talk to over a command channel. It holds a live world and the
same evaluator the console uses, so every namespace below resolves through it
identically — there is no separate, thinner headless surface.

```
cargo run --bin session --features dev               # boot + seed the default scene
cargo run --bin session --features dev -- <scene>    # boot a specific scene file
cargo run --bin session --features dev -- --empty    # start from an empty scene
```

**Protocol.** One Lua command per line on **stdin**; one JSON response per command
on **stdout**, in lockstep:

- success: `{"ok":true,"result":"<rendered value, or empty for a statement>"}`
- failure: `{"ok":false,"error":"<message>"}`

A line is evaluated as an expression first (so it echoes its value), then as a
statement. **State persists for the life of the process**: globals set on one line
are visible on the next, and any world mutation (a loaded scene, baked nav, an
imported mesh, a moved transform) stays put across commands. The session runs in
**edit mode** — it does *not* force play — and a failed command is reported in its
response **without** tearing the session down. The channel ends at EOF.

```
> pid = Scene.FindEntityByName("Player")   ->  {"ok":true,"result":""}
> Transform.SetPosition(pid, 7, 8, 9)      ->  {"ok":true,"result":""}
> Transform.GetPosition(pid)               ->  {"ok":true,"result":"7, 8, 9"}
```

Note: `local` bindings are scoped to their own line; use a **global** (no `local`)
to carry a value across commands, as with `pid` above.

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

A *material* is a reusable **asset** — glTF 2.0 metallic-roughness data — stored
once in the scene's per-World **material library**; an entity carries only a thin
**reference** to one by name (its `MaterialComponent`). Many entities can share a
single material, so editing it once updates all of them. These functions resolve
the entity's referenced material in the library and mutate that shared asset;
calling one on an entity that has no material yet **creates** a default library
material and attaches the reference. `SetTexture` sets the albedo (`base_color`)
map; an empty path clears it.

| Function | Signature |
|---|---|
| `Material.SetMetallic` | `(id, value)` |
| `Material.SetRoughness` | `(id, value)` |
| `Material.SetMetallicMap` | `(id, path)` — sampled by the renderer; the map's blue channel scales the metallic value (glTF metallic-roughness convention) |
| `Material.SetRoughnessMap` | `(id, path)` — sampled by the renderer; the map's green channel scales the roughness value |
| `Material.SetTexture` | `(id, path)` — the albedo map (sampled today) |

> Normal and emissive maps round-trip through save/load and the inspector but are
> not sampled by the renderer yet (deferred follow-ups: normal maps need vertex
> tangents; emissive needs extra uniform plumbing).

> **Imported materials.** Instantiating a glTF model populates the material library
> from the file's authored metallic-roughness materials and points each sub-object's
> entity at the matching library entry via its `MaterialComponent`. The library key
> is deterministic — `"<path>::<material-name>"` (or `"<path>::material_<index>"`
> when the material is unnamed) — so sub-objects that share one glTF material share a
> single library entry. glTF packs metallic + roughness in one texture; on import it
> is mapped to *both* the engine's `metallic_map` and `roughness_map` (the shader
> reads the blue/green channel from each). External texture URIs resolve to paths
> relative to the glTF file; embedded images are not yet extracted (a follow-up). OBJ
> imports only `Kd` (base color) and `map_Kd` (albedo) — it is static-mesh only.

> **Starter materials.** The engine ships a small ready-to-go set so a new scene has
> sane PBR values to grab instead of starting from raw factors. They live as an
> ordinary glTF asset at `project/materials/starter.gltf` — **no special-casing**: it
> is discovered by `Assets.Manifest()` and imported through the same path as any user
> model. The four materials are `Matte` (rough dielectric), `Metal` (polished
> conductor), `Plastic` (smooth coloured dielectric), and `Emissive` (a glowing
> dielectric). Instantiating the file populates the library with keys
> `project/materials/starter.gltf::Matte` (and so on for `Metal` / `Plastic` /
> `Emissive`); point any entity's `MaterialComponent` at one of those keys to reuse it,
> or copy its factors as the starting point for your own. No external textures — the
> factors stand alone, so the set stays tiny and license-clean.

## `Animator`

Drive an entity's animation clips. Clips are imported from the entity's skinned
glTF mesh (its `animations`); the named `clip` selects one to play. A keyframe
sampler poses the skeleton each fixed step, so playback is deterministic.

| Function | Signature | Notes |
|---|---|---|
| `Animator.Play` | `(id, clip)` | Hard-cut to `clip` from its start. |
| `Animator.Crossfade` | `(id, clip, duration)` | Blend out of the current clip over `duration` seconds (a zero/negative duration, or a fade into the current clip, degrades to `Play`). |
| `Animator.Stop` | `(id)` | Halt playback (freezes the pose). |

## `Input`

Read key state, and (writable half) inject input so a script can play as the
user. Keys are named strings (e.g. `"W"`, `"Space"`).

| Function | Signature | Returns |
|---|---|---|
| `Input.IsKeyDown` | `(key)` | `bool` |
| `Input.Press` | `(key)` | — |
| `Input.Release` | `(key)` | — |

## `Scene`

The structural-authoring surface: the API equivalent of the editor's hierarchy
toolbar (create / destroy / parent) and the inspector's Add Component menu. Every
verb routes through the shared `scene::authoring` module, so it behaves identically
to the editor and uses the same default values.

| Function | Signature | Returns |
|---|---|---|
| `Scene.FindEntityByName` | `(name)` | `id` (or `0` if none) |
| `Scene.CreateEntity` | `(name, [primitive])` | new entity `id` |
| `Scene.Deactivate` | `(id)` | — |
| `Scene.DestroyEntity` | `(id)` | `true` if an entity existed at `id` |
| `Scene.AddComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.RemoveComponent` | `(id, kind)` | `true` if the entity exists |
| `Scene.SetParent` | `(id, parent_id)` | — (errors on a parenting cycle) |
| `Scene.ClearParent` | `(id)` | — |
| `Scene.Save` | `([path])` | the written path |
| `Scene.SavePrefab` | `(rootId, path)` | the written path |
| `Scene.Instantiate` | `(path, [parentId])` | new root entity `id` |

**`primitive`** (optional) is one of the hierarchy toolbar's primitives,
case-insensitive: `Box`, `Sphere`, `Plane`, `Cylinder` (meshes) or `PointLight`,
`DirectionalLight`, `SpotLight`. Omit it (or pass an unknown name) to create a bare
entity carrying only its mandatory `Transform`.

**`kind`** is one of the Add Component menu's first-class components,
case-insensitive: `Light`, `Health`, `Animator`, `Collider`, `RigidBody`,
`Texture` (alias `Material`), `NavMeshAgent`, `Camera`, `Particles`,
`VisualCorrection`. Each is added with the inspector's default values; adding an
existing kind replaces it. Removing `Health` also removes its `Animator`, and
removing `Camera` also removes `VisualCorrection`, matching the inspector's
cascades. (Scripts attach by path, not as a defaulted kind — a separate concern.)

**`Scene.Deactivate`** is Unity's deferred `Object.Destroy`: it sets `active =
false` but leaves the entity in the scene. **`Scene.DestroyEntity`** is the
editor's Destroy button: it actually removes the entity.

**`Scene.Save([path])`** persists the live world. With no path it writes back to
the current scene file (the file the editor/session loaded); an explicit path
writes there and becomes the new current file (Save As). It errors if no path is
given and no current scene file is set.

### Prefabs

A **prefab** is a configured GameObject — a root entity plus its whole child
subtree, every component configured and every asset reference intact — saved to its
own `.prefab` asset so it can be stamped into any scene. It is the configure-once,
stamp-many template (Unity's prefab) and the runtime spawn primitive a wave-spawner
script calls. v1 is **unpacked**: each instance is an independent copy with no live
link back to the asset.

**`Scene.SavePrefab(rootId, path)`** extracts the subtree rooted at `rootId` and
writes it to `path` (a `.prefab` JSON document with local 0-based ids, the referenced
material slice, and no GPU buffers). Returns the written path; errors if no entity
has that id. This is the same verb the hierarchy's right-click **Save as Prefab**
runs.

**`Scene.Instantiate(path, [parentId])`** loads a `.prefab` and stamps an independent
copy into the scene, assigning fresh deterministic ids, merging the prefab's
materials into the scene library (an identical existing material is reused; a
name-conflicting one is inserted under a uniquified name and the instance's
references are rewritten to it), and parenting the new root under `parentId` (or the
scene root when omitted). Returns the new root's id. The geometry is rehydrated from
each mesh's reference on instantiate, exactly like scene load. (Instantiating an
importable model asset by reference is a separate, queued verb; `Scene.Instantiate`
is the single surface both will share, dispatching on whether the path is a `.prefab`
or an importable asset.)

## `Assets`

The project's importable assets — the "see what I can place" half of authoring.
`Assets.Manifest()` (alias `Assets.List()`) walks the `project` asset root, imports
every model file (`.gltf`/`.glb`/`.obj`), and returns a structured catalogue of the
addressable sub-objects inside each file plus their footprint (so the agent can lay
things out without overlap) and material count. Unlike the rest of the surface,
this returns a Lua **table** (not a scalar triple): the result is nested data.

Each `subObject`'s `reference` is the canonical `path::sub_object` string and
round-trips with `AssetRef` / `import_sub_mesh`, so it can be handed straight to the
mesh-instantiation path to place exactly what the manifest names. Files that fail to
import are skipped; a missing root yields an empty list. Output is deterministic
(files sorted by path, sub-objects in source order).

| Function | Signature | Returns |
|---|---|---|
| `Assets.Manifest` | `()` | array of asset tables (see shape below) |
| `Assets.List` | `()` | alias for `Assets.Manifest` |

Returned shape (Lua, 1-indexed arrays):

```lua
{
  {
    path = "project/models/crates.glb",
    materialCount = 2,            -- materials in the file's shared table
    subObjects = {
      {
        id = "Sedan",
        reference = "project/models/crates.glb::Sedan",  -- round-trips with AssetRef
        materialCount = 1,         -- 0 or 1 (a sub-mesh uses at most one material)
        size = { x = 4.2, y = 1.5, z = 1.8 },            -- AABB extent (max - min)
        min  = { x = -2.1, y = 0.0, z = -0.9 },          -- absent if the mesh is empty
        max  = { x =  2.1, y = 1.5, z =  0.9 },
      },
    },
  },
}
```

## `Navigation`

The navmesh is a height-field surface (#130): each grid cell carries a baked
surface height, so paths follow ramps, stairs, and multi-level terrain in real `y`
rather than a flat `y = 0` plane. The returned waypoint's `y` is the surface height
of the next cell — agents climb and descend instead of sliding through geometry.

| Function | Signature | Returns |
|---|---|---|
| `Navigation.GetNextPathStep` | `(cx, cy, cz, tx, ty, tz)` | next waypoint `x, y, z` on the baked surface along the A* path |

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

## `Light`

Read and tune an entity's `LightComponent` — colour, intensity, range and type.
Every setter maps onto a field the renderer reads when it packs the lighting
uniform, so a change takes effect on the next frame. A light has no per-component
"active" flag: it is gated by its owning entity's `active` (a `Scene` concern), so
this surface exposes the light's own data and nothing more. Getters return a
neutral default (`(1,1,1)` colour, `0` scalars, `"None"` type) when the entity has
no light.

| Function | Signature | Returns |
|---|---|---|
| `Light.GetColor` | `(id)` | `r, g, b` (linear) |
| `Light.SetColor` | `(id, r, g, b)` | — |
| `Light.GetIntensity` / `SetIntensity` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetRange` / `SetRange` | `(id)` / `(id, value)` | `number` (clamped ≥ 0) |
| `Light.GetType` / `SetType` | `(id)` / `(id, name)` | `"Ambient"` / `"Directional"` / `"Point"` / `"Spotlight"` |

`SetType` is case-insensitive; an unrecognized name is ignored (the current type
is kept).

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

## `Audio`

Play sound through the engine's `AudioMaestro` (the audio engine singleton). An
entity carries an `AudioSource` component (`clip`, `volume`, `loop`,
`play_on_start`, `is_time_scaled`, plus the spatial fields stored for the 3D
follow-up); these verbs start/stop and retune it, fire one-shots, and set the single
master volume. Playback is **2D (non-spatialized)** for now — `spatial_blend` and the
rolloff distances round-trip and are consumed by the 3D follow-up. Decode is
`.ogg` / `.wav`, path-cached.

`Play`/`Stop`/`PlayAt` return a `bool` that is `true` when the maestro accepted the
voice; on the **headless harness the audio backend is a no-op**, so playback makes no
sound but every action is still recorded in the maestro's introspection log (so a
play-test can assert *what* played, *where*, and *by whom* — one-shots included).

| Function | Signature | Returns |
|---|---|---|
| `Audio.Play` | `(id)` | `bool` — started the entity's `AudioSource` (logs a Play event) |
| `Audio.Stop` | `(id)` | — (stops the entity's voice, logs a Stop event) |
| `Audio.SetVolume` | `(id, v)` | — (retunes the live voice's volume, pre-master; no-op if not playing) |
| `Audio.PlayAt` | `(path, x, y, z [, vol])` | `bool` — fire-and-forget one-shot at a world position (`vol` defaults to 1.0); leaves no component, logged as a `PlayAt` event |
| `Audio.GetMasterVolume` | `()` | `number` (linear, `[0, 1]`) |
| `Audio.SetMasterVolume` | `(v)` | — (clamped to `[0, 1]`; re-folds every live voice) |

A voice's volume is its per-source `volume` multiplied by the master volume.
`is_time_scaled` chooses whether the voice follows `Time.timeScale` (gameplay sound)
or runs at wall-clock rate (music / UI that should play through a pause). The play
events are also visible in the `Debug.Snapshot` per-entity `audio` block (the
component's authoring fields).

## `Decals`

Stamp **box-projector decals** (bullet holes, scorch, blood splats) onto the
surface a shot hit. A decal is a projected *volume*, not a flat sticker: the decal
pass reconstructs the underlying surface from the scene depth and wraps the texture
onto whatever geometry the box overlaps. Spawn from the world point + surface
normal a hit already gives you (`Physics.Raycast` returns `(hit, id, distance)`, so
the point is `origin + dir * distance`; supply the surface normal). The registry is
a bounded FIFO — oldest decals are evicted past the cap.

| Function | Signature | Returns |
|---|---|---|
| `Decals.Spawn` | `(x,y,z, nx,ny,nz, [size], [texture], [rotation_deg], [r,g,b,a])` | — |
| `Decals.Count` | `()` | live decal count |
| `Decals.Clear` | `()` | — (drops every live decal) |

`size` (default `0.5`) is the stamp's width/height in world units; `texture` is the
decal sprite path (default checker); `rotation_deg` spins the stamp around its
projection axis; `r,g,b,a` tints the texel (default opaque white).

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

## `Graphics`

Live control over the engine's **existing** post-FX and quality knobs, so a script
can drive its own settings logic. The per-volume getters/setters target the first
**active** `VisualCorrectionComponent` (color/bloom/SSR) and the first **active**
`CameraComponent` (motion blur) in the scene — the same volume `build_post_params`
packs into the GPU uniform each frame, so a write here takes effect next frame.
Getters return a neutral default when no active volume/camera exists.

| Function | Signature | Returns |
|---|---|---|
| `Graphics.GetBloomActive` / `SetBloomActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetBloomIntensity` / `SetBloomIntensity` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetBloomThreshold` / `SetBloomThreshold` | `()` / `(value)` | `number` (≥ 0) |
| `Graphics.GetExposure` / `SetExposure` | `()` / `(ev)` | `number` (EV) |
| `Graphics.GetContrast` / `SetContrast` | `()` / `(value)` | `number` |
| `Graphics.GetSaturation` / `SetSaturation` | `()` / `(value)` | `number` |
| `Graphics.GetGamma` / `SetGamma` | `()` / `(value)` | `number` (clamped ≥ 0.01) |
| `Graphics.GetTonemap` / `SetTonemap` | `()` / `(name)` | `"None"` / `"Reinhard"` / `"Aces"` |
| `Graphics.GetSsrActive` / `SetSsrActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetSsrQuality` / `SetSsrQuality` | `()` / `(name)` | `string` |
| `Graphics.GetMotionBlurActive` / `SetMotionBlurActive` | `()` / `(bool)` | `bool` |
| `Graphics.GetMotionBlurSamples` / `SetMotionBlurSamples` | `()` / `(n)` | `number` (clamped 2–32) |
| `Graphics.GetQuality` / `SetQuality` | `()` / `(name)` | `"Low"` / `"Medium"` / `"High"` |

The global **quality preset** gates the heavier passes (SSR is High-tier only;
motion blur is off on Low). `SetQuality` writes a shared resource cell that the
platform layer reads each frame and hands to the renderer, which reallocates its
bloom buffers when the tier actually changes. Unrecognized tonemap/quality names
are ignored (the current value is kept).

**Determinism.** Every `Graphics` write is **one-way** into render-only state: the
post-FX volume and the preset cell are read by the render layer, never by
`FixedUpdate`. Toggling these knobs therefore cannot change how the deterministic
sim evolves, and a headless replay stays bit-for-bit stable regardless of them.

The quality preset and the `Video` settings below are **persisted** through
`Storage` (the `graphics.quality` key and the `video` namespace): the windowed app
reads them at startup and writes them back at the Stop / quit boundary, so the app
relaunches at the last-chosen tier and video settings.

## `Video`

Runtime **video** settings for the windowed app — framebuffer resolution, vsync, and
fullscreen. Kept separate from `Graphics` (which drives per-volume post-FX + the
quality tier) because these are window/surface state, not scene-graph state — the
same split Unity draws between `Screen` and `QualitySettings`.

| Function | Signature | Returns |
|---|---|---|
| `Video.GetResolution` / `SetResolution` | `()` / `(width, height)` | `width, height` (each clamped ≥ 1) |
| `Video.GetVsync` / `SetVsync` | `()` / `(bool)` | `bool` |
| `Video.GetFullscreen` / `SetFullscreen` | `()` / `(bool)` | `bool` |

A `Video.*` setter writes a shared settings cell that the platform layer reads each
frame and applies: resolution and vsync reconfigure the wgpu surface, fullscreen
toggles the winit window (borderless). Vsync off requests the surface's `Immediate`
present mode; if the backend doesn't support it, vsync stays on and `GetVsync`
reports the true state. Settings persist through the `video` `Storage` namespace
(loaded at startup, flushed on Stop / quit), so the app relaunches as last set. In
the headless harness there is no window/surface, so the setters are inert.

**Determinism.** Like `Graphics`, every `Video` write is **one-way** into
render/platform state and is never read by `FixedUpdate`, so it cannot change how
the deterministic sim evolves; a headless replay is unaffected.

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

### Keybindings — rebindable controls

The windowed app reads a **physical→logical key remap** from `Storage` at startup
and applies it at the input source, *before* the simulation reads any key. Gameplay
and the `Input` API only ever see **logical** keys, so the remap is invisible to
scripts — `Input.IsKeyDown("W")` still asks about the logical `"W"`. To offer
rebindable controls, persist the chosen bindings as a flat `{ physical: logical }`
table under the `keybindings` namespace, key `bindings`; only entries that differ
from identity need to be stored, and the mapping loads on next launch:

```lua
-- Make the arrow keys drive the same logical keys as WASD.
Storage.SetTable("keybindings", { bindings = { UP = "W", DOWN = "S", LEFT = "A", RIGHT = "D" } })
```

The remap is a platform-layer (windowed) concern: the headless harness and
bot-players inject **logical** keys directly via `Input.Press`/`Input.Release`, so
they bypass the keymap and stay deterministic.

## `Debug` — **dev builds only**

Mirrors Unity's `[Conditional]` `Debug`. Registered only under the `dev` Cargo
feature and stripped from ship builds; calling it from a ship build is a no-op
because the namespace is absent.

| Function | Signature | Returns |
|---|---|---|
| `Debug.Log` | `(message)` | — |
| `Debug.Warn` | `(message)` | — |
| `Debug.Error` | `(message)` | — |
| `Debug.Snapshot` | `()` | a pretty **JSON string**: the whole live world (below) |
| `Debug.SnapshotEntity` | `(id)` | a pretty **JSON string**: one entity (below), or `null` if absent |

### The snapshot — the structured scene-read (#180)

`Debug.Snapshot()` is the **read half of editor↔API parity**: it returns the live
world as a stable, diffable JSON document rich enough to *author* against — the
agent's "look at the scene" verb in a headless session. It reads the **live world**
(the source of truth), never the scene file, and never dumps GPU buffers — only
references and values, mirroring the saved `SceneData`.

Top level:

```json
{
  "frame": 0,
  "play_state": "editor",            // or "playing"
  "camera": { "pos": [x,y,z], "yaw": .., "pitch": .., "fov": .. },
  "entities": [ <entity>, ... ]
}
```

Each `<entity>` (also what `Debug.SnapshotEntity(id)` returns):

```json
{
  "id": 1, "name": "Crate", "active": true, "static": false, "layer": 0,
  "parent": null, "children": [],
  "components": ["Mesh", "Material", "Collider"],   // optional-component inventory
  "transform": { "pos": [x,y,z], "rot": [x,y,z], "scale": [x,y,z] },  // rot = Euler°
  "bounds": { "min": [x,y,z], "max": [x,y,z] },     // world-space AABB, or null
  "scripts": ["project/scripts/foo.lua"],
  "mesh":      { "primitive_type": "Box", "asset_ref": "models/crates.glb::Barrel" },
  "material":  { "color": [r,g,b], "metallic": .., "roughness": .., "texture": "..",
                 "metallic_map": null, "roughness_map": null },
  "light":     { "type": "Point", "color": [r,g,b], "intensity": .., "range": ..,
                 "inner_cone": .., "outer_cone": .. },
  "collider":  { "active": true, "is_trigger": false,
                 "shape": { "kind": "Box", "size": [x,y,z] } },
  "rigidbody": { "active": true, "is_kinematic": false, "mass": .., "velocity": [x,y,z],
                 "use_gravity": true },
  "camera":    { "active": true, "fov": .., "near": .., "far": .., "culling_mask": ..,
                 "render_order": 0 },
  "nav_agent": { "active": true, "radius": .., "target": [x,y,z], "speed": .., .. },
  "particles": { "active": true, "texture": null, "rate": .., "lifetime": .., .. },
  "animator":  { "clip": "Idle", "time": .., "speed": .., "playing": true },
  "health":    { "current": .., "max": .., "dead": false },
  "audio":     { "clip": "music/theme.ogg", "volume": .., "loop": false,
                 "play_on_start": false, "is_time_scaled": true,
                 "spatial_blend": 0.0, "initial_distance": .., "final_distance": .. }
}
```

Every per-component key is present only when the entity carries that component
(absent ones serialize as `null`); `bounds` comes from the mesh when geometry is
present, else the collider, else `null`. The shape is shared with the harness's
`Harness.Snapshot` (the play-testing path), so the read is identical wherever it's
taken.

---

## Script field schema (inspector decorators)

The engine's equivalent of Unity's `[SerializeField]` + `[Range]` / `[Tooltip]` /
`[Header]` attributes. A script may `return` an optional `fields` table describing
its inspector-editable serialized fields. The generic Lua Script inspector then
renders a typed control per field — no hand-written egui card needed — and the
values you set persist with the scene.

```lua
return {
  fields = {
    speed   = { type = "number", range = {0, 10}, default = 3.0, tooltip = "units/sec", header = "Movement" },
    jumps   = { type = "number", default = 2 },
    canFly  = { type = "boolean", default = false },
    label   = { type = "string", default = "Rusty" },
    -- A bare value is shorthand: its type is inferred and it becomes the default.
    health  = 100,
  },
  Start  = function(id) end,
  Update = function(id, dt) end,
}
```

At Play — before `Start` runs — each field is written as a key on the table the
script returns (the inspector override, or the schema `default`). So write the
script in the `local M = {} … return M` form and read the configured value off
that captured table:

```lua
local M = {}
M.speed = 0  -- overwritten by the inspector value at load
function M.Update(id, dt)
  local x = Transform.GetPosition(id)
  Transform.SetPosition(id, x + M.speed * dt, 0, 0)
end
return M
```

**Supported `type` values** (omit `type` and it's inferred from `default`):

| `type` | Inspector control | Stored as |
|---|---|---|
| `"number"` | drag value, or a **slider** when `range` is given | `f64` |
| `"boolean"` (`"bool"`) | checkbox | `bool` |
| `"string"` (`"text"`) | single-line text edit | `String` |

**Decorators** (all optional):

| Key | Effect |
|---|---|
| `range = {min, max}` | Renders a slider clamped to `[min, max]` (numbers only). |
| `default = <value>` | Initial value before the inspector overrides it; also fixes the inferred type. |
| `tooltip = "..."` | Hover text on the control. |
| `header = "..."` | A bold section label drawn above the field. |

Fields with no metadata fall back to a default control inferred from their value.
Fields are listed in the inspector sorted by name (Lua table order is
unspecified). Edited values are stored per script instance in the scene file, so
two entities running the same script can carry different field values.
