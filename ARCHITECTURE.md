# Architecture: Systems & Classes

A flat inventory of what this engine is made of, so the moving parts are visible
before any of them are implemented.

**Legend:** `[now]` already exists in the legacy code · `[new]` expected, not yet
written · `[partial]` exists but incomplete.

---

## Resources — engine singletons (one instance per World)

Unity analog: the engine statics (`Time`, `Input`, …). Today these are the
`Rc<RefCell<…>>` values handed around in `main.rs`; they become resources stored in
the World.

| Resource | Status | Role |
|---|---|---|
| `Time` | `[new]` | delta, fixed_delta, frame_count, total time; drives the fixed clock |
| `InputState` | `[partial]` | key/mouse state — read today; needs **writable** path for bots |
| `NavigationGraph` | `[now]` | baked navmesh grid + A* |
| `Console` | `[partial]` | log buffer today (`ConsoleLogs`); add the live REPL |
| `ScriptRuntime` | `[now]` | the mlua state + loaded entity scripts (`ScriptManager`) |
| `ActiveCamera` | `[now]` | the camera the renderer uses (`Camera`) |
| `PlayState` | `[partial]` | editor-vs-play; today a bare `is_playing: bool` |
| `Renderer` | `[now]` | wgpu device/queue/surface/pipelines (render-side only) |
| `EditorState` | `[now]` | egui editor UI state (`EditorUi`) |
| `AssetServer` | `[new]` | typed asset handles + cache (replaces passing path strings) — later |

---

## Components — per-entity data (the Unity-style "classes")

First-class, engine-provided; systems expect them. Custom behaviour goes in
*scripts*, not new built-in components.

| Component | Status | Unity analog |
|---|---|---|
| `Transform` | `[now]` | Transform |
| `Mesh` | `[now]` | MeshFilter/MeshRenderer |
| `Texture`/Material | `[now]` | Material |
| `Camera` | `[now]` | Camera |
| `Light` (+`LightType`) | `[now]` | Light |
| `Collider` (+`ColliderShape`) | `[now]` | Collider |
| `Rigidbody` | `[now]` | Rigidbody |
| `Health` | `[now]` | (gameplay) |
| `Animator` | `[now]` | Animator |
| `NavMeshAgent` | `[now]` | NavMeshAgent |
| `Script` | `[partial]` | MonoBehaviour ref; add `dev_only` flag |
| `VisualCorrection` | `[now]` | post-process volume |
| `AudioSource` | `[new]` | AudioSource — when an audio system lands |
| `ParticleEmitter` | `[new]` | ParticleSystem — later |

---

## Systems — per-frame logic, grouped by stage

A system is a plain `fn(&mut World, &mut Resources)`. Order within a stage is the
order modules `register` them.

**Startup**
- `load_scene` `[now]` · `bake_navmesh` `[now]`

**FixedUpdate** (deterministic, fixed dt — what the harness steps)
- `physics_tick` `[now]` (gravity, box-clip, triggers)
- `script_fixed_update` `[new]` (MonoBehaviour `FixedUpdate`)
- `nav_agent_tick` `[now]` (move agents along paths)
- `rebake_navmesh` `[partial]` (today a wall-clock 1s timer → frame-count based)

**Update**
- `gather_input` `[now]`
- `player_controller` `[partial]` (input→player; today hardcoded in `main.rs`, becomes a system reading `Input`)
- `script_start` / `script_update` `[now]`
- `hitscan_shoot` `[partial]` (raycast on fire; today inline in `main.rs`)
- `dispatch_triggers` `[now]` (physics → script `OnTrigger`/`OnDamage`)
- `animator_update` `[now]`
- `free_fly_camera` `[now]` (editor mode)
- `play_state_transitions` `[partial]` (enter/exit play; today inline)

**LateUpdate**
- `camera_follow` `[partial]` (third-person follow; today inline)
- `update_colliders` / `propagate_hierarchy` `[now]` (world AABBs, parent matrices)

**Render**
- `render_scene` `[now]` (forward lit + gizmos + pathfinding lines)
- `editor_ui` `[now]` (egui panels)
- `screenshot` `[new]` (dev-only, offscreen → PNG)

---

## Core types — `app/` + `ecs/` (all `[new]`)

`App` · `Schedule` · `Stage` · `System` · `Resources` · `World` (hecs wrapper) ·
`EntityId` (generational) · `Commands` (deferred spawn/despawn).

---

## Subsystem types (the bigger non-component structs)

| Type | Status | Where |
|---|---|---|
| `Renderer`, `ShadowRenderer`, `Skybox`, `Vertex` | `[now]` | `render/` |
| `Ray` + ray-AABB cast | `[now]` | `physics/` |
| `ScriptManager` | `[now]` | `scripting/` |
| `EditorUi` + inspectors | `[now]` | `editor/` |
| `NavigationGraph` | `[now]` | `navigation/` |
| `Console` (REPL), `Harness`, `Scenario`, `Screenshot` | `[new]` | `dev/` |

---

## API modules — `api/` (Lua/console/bot bindings, not classes)

`Transform` · `Input` (read + **write**) · `Time` · `Physics` (+ `Raycast`/`Shoot`) ·
`Scene` · `Animator` · `Nav` · `Health` · `Camera` · `Material` · `Debug` (dev-only).
One stable surface, shared by gameplay scripts, the console REPL, and bot-players.
