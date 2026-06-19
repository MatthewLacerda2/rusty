# API faithfulness catalog

Every `api::` **setter** (and the writes behind `Spawn`/`Emit`-style verbs) is
listed here with its faithfulness status and the **read-site** that proves it. The
audit (issue #178) exists because the `api::` surface can carry setters that are
*named-but-not-wired*: a `Set*` that writes a field nothing downstream reads, so a
headless agent saves a scene that only looks wrong when a human finally opens it.
In play-testing you'd catch it; in fire-and-forget authoring you wouldn't.

A setter is **faithful** only if a downstream system actually consumes what it
writes. Three kinds of read-site count:

- **renderer** — a per-frame GPU read (the field reaches a uniform / texture / pass).
- **sim** — a system reads it during a tick (`FixedUpdate`/`Update`/`LateUpdate`).
- **round-trip** — the field is part of the serialized `SceneData` and survives a
  save→load (authoring intent persists even if the live effect is elsewhere).

Status legend: ✅ faithful · ⚠️ partial (works on one path, not another) ·
❌ no-op (written, never read).

## How a new binding proves faithfulness

When you add a setter, **name its read-site in the same change** — and prove it one
of two ways:

1. **A read-site** — point to the system or GPU pass that consumes the field, and
   add a row to the table below. If you can't name one, the setter is a no-op and
   doesn't belong on the surface yet; file it as a `bug`/`feature` instead.
2. **A round-trip test** — for fields whose only job is to persist (authoring data),
   assert the value survives `save → load` so a future system can rely on it.

This is the runtime half of the four-axis completeness mindset (see
`docs/linting.md`): the completeness gate proves a component *exists* on all four
axes; this catalog proves each setter's write is *observed*. Prefer a real test
over a doc claim — `tests/graphics_api.rs`, `tests/time_scale.rs`, and the
per-module unit tests (e.g. `api/light.rs`) are the pattern.

## Catalog

### `Transform` — over `Entity.transform`

| Setter | Status | Read-site |
|---|---|---|
| `SetPosition` | ✅ | renderer (`draw_resources::solid_entity_uniform` model matrix) + sim (`compute_world_matrix`, collider re-sync); round-trips |
| `SetRotation` | ✅ | same model-matrix read; collider re-sync on set |
| `SetScale` | ✅ | same model-matrix read; collider re-sync on set |
| `MoveTowards` | ✅ | writes `position`; same read-sites |

### `Material` — over `Entity.texture` (`TextureComponent`)

| Setter | Status | Read-site |
|---|---|---|
| `SetMetallic` | ✅ | renderer — `draw_resources::solid_entity_uniform` packs `metallic` into `EntityUniform`; `shader.wgsl` PBR uses it |
| `SetRoughness` | ✅ | renderer — same path, `roughness` uniform |
| `SetTexture` | ✅ | renderer — `draw::upload_scene_assets` calls `load_texture(path)` for every active entity **each frame**, and `draw_resources::build_solid_resource` binds `gpu_textures[path]`. A runtime path change loads + binds next frame. (This is the issue's "canonical no-op"; it is **wired** in the current renderer.) Round-trips. |
| `SetMetallicMap` | ❌ | **none** — written + serialized + shown in inspector, but no renderer/shader read. Deep hole → **#184** |
| `SetRoughnessMap` | ❌ | **none** — same as above → **#184** |

### `Animator` — over `Entity.animator`

| Setter | Status | Read-site |
|---|---|---|
| `Play` | ✅ | sim — `app/animation.rs` samples `current_clip` against imported clips |
| `Crossfade` | ✅ | sim — drives blend state, sampled by `app/animation.rs` (#80) |
| `Stop` | ✅ | sim — `is_playing=false` halts the sampler |

### `Input` — over the shared `InputState`

| Setter | Status | Read-site |
|---|---|---|
| `Press` | ✅ | sim — scripts read `IsKeyDown`; engine input systems read the same state |
| `Release` | ✅ | same |

### `Scene` — over `Entity`

| Setter | Status | Read-site |
|---|---|---|
| `DestroyEntity` | ✅ | sim + renderer — sets `active=false`; both `draw_resources` and the sim skip inactive entities; round-trips |

### `NavMeshAgent` — over `Entity.nav_agent`

| Setter | Status | Read-site |
|---|---|---|
| `SetTarget` | ✅ | sim — `navigation/agents.rs` steers toward `target` |
| `SetSpeed` | ✅ | sim — `navigation/agents.rs` clamps velocity to `speed` |
| `SetAcceleration` | ✅ | sim — `navigation/agents.rs` lerps velocity by `acceleration` |
| `SetStoppingDistance` | ✅ | sim — `navigation/agents.rs` arrival test |
| `SetRadius` | ✅ | sim — agent footprint; round-trips |
| `SetActive` | ✅ | sim — gates agent stepping |

### `Physics` — over `Entity.rigidbody`

| Setter | Status | Read-site |
|---|---|---|
| `SetVelocity` | ✅ | sim — `physics/world.rs` integrates / writes back `velocity` |
| `AddForce` | ✅ | sim — folds impulse into `velocity` (read above); skips kinematic |
| `SetKinematic` | ✅ | sim — `physics/build.rs::is_kinematic` / `world.rs` body class |
| `Shoot` (write: damage) | ✅ | sim — routes through `apply_damage` (see `Health`) |

### `Health` — over `Entity.health`

| Setter | Status | Read-site |
|---|---|---|
| `Set` | ✅ | sim/renderer — `current_health`/`is_dead` drive death state + dead-tint in `solid_entity_uniform`; round-trips |
| `Heal` | ✅ | same |
| `Damage` | ✅ | sim — `apply_damage` flips `is_dead`, freezes the death clip, logs |

### `Time` — over the `Time` resource

| Setter | Status | Read-site |
|---|---|---|
| `SetTimeScale` | ✅ | sim — `time/mod.rs` scales `delta_time` the whole sim reads (`tests/time_scale.rs`) |

### `Camera` — over the shared `render::Camera`

| Setter | Status | Read-site |
|---|---|---|
| `SetPosition` | ✅ | renderer — `app/camera_sync.rs` + view matrix |
| `SetYaw` / `SetPitch` | ✅ | renderer — `Camera::forward`/view matrix (pitch clamped) |
| `SetFov` | ✅ | renderer — projection matrix (clamped 1..179°) |

### `Light` — over `Entity.light`

| Setter | Status | Read-site |
|---|---|---|
| `SetColor` | ✅ | renderer — `apply_scene_lights` lighting uniform |
| `SetIntensity` | ✅ | renderer — same (clamped ≥ 0) |
| `SetRange` | ✅ | renderer — point/spot attenuation |
| `SetType` | ✅ | renderer — selects light path; unknown names ignored |

### `Particles` — over `Entity.particles`

| Setter | Status | Read-site |
|---|---|---|
| `Emit` / `Burst` | ✅ | sim/renderer — `emit_at` spawns into the runtime buffer `app/particles.rs` advances and the renderer draws |
| `SetActive` | ✅ | sim — gates continuous emission |
| `SetRate` | ✅ | sim — continuous spawn cadence |
| `Clear` | ✅ | renderer — empties the live buffer that's drawn |

### `Decals` — over `Scene.decals`

| Setter | Status | Read-site |
|---|---|---|
| `Spawn` | ✅ | renderer — `render/decals_draw.rs` projects each decal; loads its texture |
| `Clear` | ✅ | renderer — empties the projected set |

### `Layers` — over `Entity.layer`

| Setter | Status | Read-site |
|---|---|---|
| `SetLayer` | ✅ | renderer + sim — `draw_resources` culling mask (#92) and `physics/build.rs::interaction_groups` collision groups (#91); round-trips |

### `Graphics` — over the active `VisualCorrectionComponent` / `CameraComponent` / `QualityPreset`

| Setter | Status | Read-site |
|---|---|---|
| `SetBloomActive` / `SetBloomIntensity` / `SetBloomThreshold` | ✅ | renderer — `build_post_params` rebuilds the post-FX uniform each frame |
| `SetExposure` / `SetContrast` / `SetSaturation` / `SetGamma` | ✅ | renderer — same post-FX uniform (`tests/graphics_api.rs`) |
| `SetTonemap` | ✅ | renderer — post-FX operator; unknown names ignored |
| `SetSsrActive` / `SetSsrQuality` | ✅ | renderer — SSR pass (gated by High preset) |
| `SetMotionBlurActive` / `SetMotionBlurSamples` | ✅ | renderer — `render/postfx_params.rs` motion-blur params (samples clamped 2..32) |
| `SetQuality` | ✅ | renderer — platform layer hands the shared cell to `renderer.set_quality` |

### `Video` — over the shared `VideoSettings`

| Setter | Status | Read-site |
|---|---|---|
| `SetResolution` | ✅ | renderer/window — `main.rs` reconfigures the wgpu surface when the cell changes (clamped ≥ 1) |
| `SetVsync` | ✅ | renderer/window — present-mode reconfigure |
| `SetFullscreen` | ✅ | window — winit fullscreen reconfigure |

### `Storage` — over the `Storage` resource

| Setter | Status | Read-site |
|---|---|---|
| `Set` / `SetTable` | ✅ | round-trip — readable via `Get`/`GetTable`; flushed to disk at Stop/quit (#86) |
| `Delete` | ✅ | round-trip — removes the key from the same store |

### `Debug` (dev-only) — over `ConsoleLogs`

| Setter | Status | Read-site |
|---|---|---|
| `Log` / `Warn` / `Error` | ✅ | sim — appends to the console buffer the REPL/overlay render |

## Summary

| Status | Count |
|---|---|
| ✅ faithful | 49 |
| ⚠️ partial | 0 |
| ❌ no-op | 2 |

The only no-ops are `Material.SetMetallicMap` / `SetRoughnessMap` — a renderer
feature gap, spun out as **#184** (they need GPU bindings + shader sampling, not a
one-field wire). The issue's "canonical no-op" — `Material.SetTexture` — is in fact
**faithful** against the current renderer (`upload_scene_assets` lazily loads any
texture path each frame), so no fix was needed there.
