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
| `SetMetallicMap` | ✅ | renderer — `draw::upload_scene_assets` loads the map, `draw_resources::build_solid_resource` assembles the per-entity group(2) material bind group, and `shader.wgsl` samples `t_metallic` (`.b` channel) × the metallic scalar (#202, closed #184) |
| `SetRoughnessMap` | ✅ | renderer — same path; `shader.wgsl` samples `t_roughness` (`.g` channel) × the roughness scalar (#202, closed #184) |
| `SetEmissive` | ✅ | renderer — `draw_resources::solid_entity_uniform` packs `emissive` into `EntityUniform`; `shader.wgsl` adds it after lighting (blooms when >1.0) (#222) |
| `SetNormalMap` | ✅ | renderer — `draw::upload_scene_assets` loads the map, `build_solid_resource` binds `t_normal` to the group(2) material bind group, and `shader.wgsl` perturbs the shading normal in tangent space (per-vertex tangents + TBN) (#207) |
| `SetEmissiveMap` | ✅ | renderer — same upload/bind path; `shader.wgsl` samples `t_emissive` and modulates the emissive factor (`factor × map.rgb`) (#207) |
| `DefineAsset` | ✅ | renderer + round-trip (#271) — writes a `MaterialAsset` into `scene.materials` under a chosen name; an entity referencing that name via its `MaterialComponent` is sampled by the renderer through the SAME per-entity material path as every row above (`draw_resources::build_solid_resource` + `shader.wgsl`). The asset is part of `SceneData` (`materials` map), so it survives save→load. Proven by `tests/material_authoring.rs` (round-trip; entity-uses-asset; clamps/unknown-mode degrade; packed map slots) |
| `GetAsset` / `HasAsset` | ✅ | round-trip — read-back of the named library asset (canonical JSON / presence), so the authored asset is observable through the same surface |
| `SetShader` (+ the recipe's `shader` key) | ✅ | renderer (#396) — `draw_resources::solid_draw_item` resolves `MaterialAsset::shader` to a pipeline id through `gpu::pipelines::surface::SurfaceShaders` (lazy, cached by name, log-once fallback to the standard shader, rebuilt on re-bake); `draw_batches` binds that variant's opaque/transparent pipeline. Proven by `surface_tests.rs` (a baked variant renders differently; missing/postfx modules fall back pixel-identically; a re-bake rebuilds) |

### `Animator` — over `Entity.animator`

| Setter | Status | Read-site |
|---|---|---|
| `Play` | ✅ | sim — `app/animation.rs` samples `current_clip` against imported clips |
| `Crossfade` | ✅ | sim — drives blend state, sampled by `app/animation.rs` (#80) |
| `Stop` | ✅ | sim — `is_playing=false` halts the sampler |
| `AddTwoBoneIK` / `AddAimIK` / `RemoveIK` | ✅ | sim — `Scene::solve_ik` (system `solve_ik`, after `LateUpdate`) solves `AnimatorComponent.ik`; round-trips (#461) |
| `SetIKTarget[Entity]` / `SetIKHint[Entity]` | ✅ | sim — read by `Scene::solve_ik` each fixed step (runtime only, not saved) (#461) |
| `SetIKWeight` | ✅ | sim — scales the solve in `Scene::solve_ik`; round-trips (#461) |

### `Input` — over the shared `InputState`

| Setter | Status | Read-site |
|---|---|---|
| `Press` | ✅ | sim — scripts read `IsKeyDown`; engine input systems read the same state |
| `Release` | ✅ | same |

### `Scene` — over `Entity`

| Setter | Status | Read-site |
|---|---|---|
| `DestroyEntity` | ✅ | sim + renderer — sets `active=false`; both `draw_resources` and the sim skip inactive entities; round-trips |
| `Instantiate` (linked prefab) | ✅ | stamps `Entity.prefab_link` on every instance entity; read by `apply_scene_data`'s load-time propagation + `ReimportPrefab`; the link + its overrides round-trip through `SceneData` (#216) |
| `RecordPrefabOverrides` | ✅ | round-trip — records the instance↔source diff into `prefab_link.overrides`; observable via `ListPrefabOverrides` and re-applied by propagation |
| `RevertPrefabOverrides` / `ReimportPrefab` | ✅ | rebuild each instance entity from a fresh source baseline (re-applying overrides on reimport); the resulting component values feed the same renderer/sim read-sites as any edited entity |
| `ApplyPrefabToSource` / `ApplyPrefabFieldToSource` | ✅ | write the instance's overrides into the source `.prefab` on disk (reusing the override setter + the one `.prefab` writer), then clear them — other instances pick the edit up on reload/reimport (#268) |
| `SetActive` | ✅ | sim + renderer — the same `active` flag `DestroyEntity` clears: the script lifecycle's `OnEnable`/`OnDisable` edge sweep, layout groups, UI hit-testing and drawing all read it; round-trips (`tests/ui_widgets/reads.rs`, #422) |

### `Navigation` — over `Scene.nav_settings`

The per-scene navmesh bake settings (#276). Each setter writes `scene.nav_settings`
then re-bakes the shared graph, so the **bake** is the read-site — proven in
`src/api/nav/settings_tests.rs` (set via binding → `nav_settings` reflects it → a
re-bake reflects it) and, at the bake itself, in `src/navigation/bake_tests.rs`
(`bake_sources_max_step_from_scene_settings`, `bake_reshapes_grid_from_grid_spacing`),
`src/navigation/erosion_tests.rs` (radius), and `src/navigation/headroom/tests.rs`
(height — `low_overhang_carves_cells_beneath`).

| Setter | Status | Read-site |
|---|---|---|
| `SetMaxSlope` | ✅ | sim — `navigation/bake.rs::bake` copies it into `NavigationGraph::max_slope` (the per-edge slope rule); round-trips through `SceneData` |
| `SetMaxStep` | ✅ | sim — `navigation/bake.rs::bake` copies it into `NavigationGraph::max_step` (the per-edge step rule); round-trips. `bake_sources_max_step_from_scene_settings` proves a ledge flips reachability |
| `SetGridSpacing` | ✅ | sim — `navigation/bake.rs::bake` re-shapes the grid dimensions from it (`apply_grid_spacing`); round-trips |
| `SetAgentRadius` | ✅ | sim — `navigation/bake/erosion.rs` **erodes the walkable spans by the radius** (#277): the agent-radius read-site. Passages narrower than ~`2*radius` close and the surface pulls off walls/world-edge; `radius==0` is an exact no-op. Round-trips through `SceneData`; `thin_passage_closes_under_erosion` / `wide_corridor_keeps_core_pulled_off_both_walls` prove it |
| `SetAgentHeight` | ✅ | sim — `navigation/bake/heightfield.rs::open_spans` **drops spans whose open space is below the height** (#278, layered in #454): the agent-height read-site, also applied to moves by `navigation/links.rs`. The floor under a low overhang goes; the overhang's own top stays. Round-trips through `SceneData`; `set_agent_height_round_trips_and_rebakes` (`api/nav`) + `low_overhang_drops_the_floor_beneath_but_keeps_its_top` / `taller_agent_drops_a_superset` (`navigation::bake::tests::headroom`) prove it |
| `SetDropHeight` / `SetJumpDistance` / `SetJumpHeight` / `SetLinkSpacing` | ✅ | sim — `navigation/offmesh/generate.rs::LinkParams` turns them into the generated drop and jump links (#462); round-trips. `drops_leave_the_platform_edges_and_land_on_the_floor` / `a_gap_is_jumped_both_ways_within_the_jump_distance` prove it |
| `SetAreaCost` / `DefineArea` | ✅ | sim — `astar.rs::expand` multiplies each step by the area cost (`NavigationGraph::area_costs`, refreshed by `sync` without a rebake, #460); round-trips in `nav_settings.areas`. `a_costly_strip_is_routed_around_when_a_detour_exists` / `a_runtime_cost_change_reroutes_without_a_rebake` prove it |

### `NavMeshAgent` — over `Entity.nav_agent`

| Setter | Status | Read-site |
|---|---|---|
| `SetTarget` | ✅ | sim — `navigation/agents/tick.rs` steers toward `target` |
| `SetSpeed` | ✅ | sim — `navigation/agents/tick.rs` clamps velocity to `speed` |
| `SetAcceleration` | ✅ | sim — `navigation/agents/tick.rs` lerps velocity by `acceleration` |
| `SetStoppingDistance` | ✅ | sim — `navigation/agents/tick.rs` arrival test |
| `SetRadius` | ✅ | sim — agent footprint; ORCA keeps agents `radius` apart (#463); round-trips |
| `SetActive` | ✅ | sim — gates agent stepping |
| `SetAvoidancePriority` | ✅ | sim — `navigation/avoidance` picks each pair's dodge share by priority (#463); round-trips |
| `SetAvoidanceEnabled` | ✅ | sim — `navigation/agents/tick.rs` only runs ORCA for agents with it on; round-trips |
| `SetAutoTraverseOffMeshLink` | ✅ | sim — `navigation/agents/tick.rs::on_link` crosses the link itself only with it on (#462); round-trips. `an_agent_waits_on_a_link_until_its_script_completes_it` proves it |
| `SetAreaMask` | ✅ | sim — `navigation/agents/mod.rs::plan_agent_path` plans with it (`calculate_path_masked`, #460) and the op drops the cached path; round-trips. `a_masked_out_area_is_never_entered` / `the_area_table_masks_and_masked_queries_reach_the_navmesh` prove it |

### `NavMeshObstacle` — over `Entity.nav_obstacle`

| Setter | Status | Read-site |
|---|---|---|
| `SetShape` / `SetCenter` / `SetSize` / `SetRadius` / `SetHeight` | ✅ | sim — `navigation/obstacle.rs::ObstacleVolume` is the carved volume (`bake/carve.rs`) and the avoidance disc; round-trips. `a_carving_obstacle_blocks_the_path_and_reopens_when_removed` proves it |
| `SetActive` / `SetCarving` | ✅ | sim — `NavMeshObstacleComponent::is_carving` gates carving (`obstacle.rs::carving_volumes`) and avoidance (`avoidance_obstacles`); round-trips |
| `SetCarveOnlyStationary` / `SetMoveThreshold` / `SetTimeToStationary` | ✅ | sim — `navigation/obstacle.rs::tick_obstacles` + `is_carving`; `carve_only_stationary_waits_for_the_obstacle_to_stand_still` proves it; round-trips |

### `OffMeshLink` — over `Entity.offmesh_link`

| Setter | Status | Read-site |
|---|---|---|
| `SetStart` / `SetEnd` / `SetBidirectional` / `SetCost` / `SetActive` | ✅ | sim — `navigation/offmesh/authored.rs::authored_keys` reads them into the bake inputs; `resolve_authored` snaps the ends and A\* crosses the link (`link_moves`); round-trips. `an_authored_ladder_connects_two_floors` proves it |
| `SetArea` | ✅ | sim — `link_moves` skips a link whose area the query's mask excludes and charges its length at the area's cost (#460); round-trips. `an_agent_never_climbs_a_ladder_its_area_mask_excludes` proves it |

### `NavMeshModifierVolume` — over `Entity.nav_modifier`

| Setter | Status | Read-site |
|---|---|---|
| `SetCenter` / `SetSize` / `SetArea` / `SetActive` | ✅ | sim — `navigation/bake/modifiers.rs::modifier_keys` reads them into the bake inputs and `apply` assigns the area to the spans inside the box (`NotWalkable` removes them); round-trips. `a_volume_assigns_its_area_to_the_floor_inside_it` / `volume_edits_rebake_incrementally_to_a_full_bake` prove it |

### `Physics` — over `Entity.rigidbody`

| Setter | Status | Read-site |
|---|---|---|
| `SetVelocity` | ✅ | sim — `physics/world.rs` integrates / writes back `velocity` |
| `SetAngularVelocity` | ✅ | sim — `physics/world.rs` pushes `set_angvel` / writes back `angvel` |
| `AddForce` | ✅ | sim — folds impulse into `velocity` (read above); skips kinematic |
| `AddForceAtPosition` / `AddImpulseAtPosition` | ✅ | sim — `physics/ragdoll.rs::impulse_response` turns the push into `velocity` / `angular_velocity` (read above) from the body's mass properties (`src/physics/live_tests.rs`, `tests/ragdoll/fall.rs`) |
| `SetKinematic` | ✅ | sim — `physics/build.rs::class_of`; a mid-play switch reaches rapier through `physics/live.rs::body_type` in `world.rs::apply_body_state` (`set_kinematic_mid_play_stops_and_restarts_a_falling_body`) |
| `SetCollisionDetection` | ✅ | sim — `physics/build.rs::ccd_enabled` / `world.rs` `enable_ccd` (proven by `physics/ccd_tests.rs`: against a moving wall Discrete tunnels, Continuous stops; a static wall stops both since rapier 0.36, #754) |

(The #311 spatial query surface — `Raycast`'s siblings `SphereCast`,
`OverlapSphere`/`OverlapBox`/`OverlapCapsule`, `CheckSphere`/`CheckBox`,
`ClosestPoint`/`ContainsPoint`, and `GetBounds` — is read-only introspection over
the live rapier world, not setters, so it doesn't add to the count. Its
faithfulness claim is script↔engine **agreement**: every query routes through the
same `PhysicsWorld` query pipeline the engine uses, proven by
`src/physics/spatial_tests.rs` (the queries against hand-computed geometry) and
`src/scripting/tests_spatial.rs` (the Lua bindings return those same answers,
including the #91 layer-mask filter). `GetBounds` reads the collider's cached
world AABB — the `calculate_world_aabb` value the scene recomputes on transform
edits and each physics step.)

### `Joint` — over `Entity.joint` (#449)

| Setter | Status | Read-site |
|---|---|---|
| `SetKind` / `SetConnectedBody` / `SetAnchor` / `SetConnectedAnchor` / `SetAutoConfigureConnectedAnchor` / `SetAxis` / `SetSwingAxis` / `SetUseLimits` / `SetLimits` / `SetSwingLimit` / `SetSwing2Limit` / `SetEnableCollision` | ✅ | sim — `PhysicsWorld::resync_joints` builds the rapier joint from them, rebuilding on a change; swing 1 / swing 2 stop a Ball per axis (`src/physics/joints_tests.rs`); round-trips, the connected body remapped through prefabs (`tests/joint_api.rs`) |
| `SetBreakForce` / `SetBreakTorque` | ✅ | sim — `PhysicsWorld::break_joints` reads them live after each step, torque taken about the anchor (`src/physics/joints_tests.rs`, `src/physics/joints/load_tests.rs`, #803); round-trips |

### `Ragdoll` — over the bones' `Entity.rigidbody` / `Entity.joint` (#466)

| Setter | Status | Read-site |
|---|---|---|
| `Build` | ✅ | sim — writes ordinary `Rigidbody` / `Joint` components the physics world builds (`tests/ragdoll/mod.rs`); round-trips by bone name (`scene::skeleton::ragdoll::persist_tests`) |
| `Enable` / `Disable` | ✅ | sim — flips `is_kinematic` (read by `physics/live.rs::body_type`), seeds `velocity`, swaps the hitbox layer (read by `PhysicsWorld::sync_layers`); `PhysicsWorld::pose_bones` then poses the bones (`tests/ragdoll/switch.rs`) |

### `CharacterController` — over `Entity.character_controller` (#451)

| Setter | Status | Read-site |
|---|---|---|
| `SetHeight` / `SetRadius` / `SetCenter` | ✅ | sim — `physics::move_character` sweeps the capsule they describe, `PhysicsWorld::sync_characters` resizes its collider each tick, `physics::can_stand` tests it (`tests/character_controller/`); round-trips |
| `SetStepOffset` / `SetSlopeLimit` / `SetSkinWidth` / `SetMinMoveDistance` | ✅ | sim — `physics::character::controller` configures rapier's controller from them on every `Move` (`tests/character_controller/`); round-trips |

(`Move` is the verb the setters feed; `IsGrounded`, `GetCollisionFlags` and
`GetGroundNormal` read back what it found.)

### `LODGroup` — over `Entity.lod_group` (#472)

| Setter | Status | Read-site |
|---|---|---|
| `SetSize` / `SetLevelHeight` / `SetRenderers` / `AddLevel` / `RemoveLevel` | ✅ | render — `render::lod::LodSelection` picks each group's level per camera and the solid pass and shadow sweeps skip the other levels' renderers (`src/render/lod_tests.rs`, `src/render/lod_gpu_tests.rs`); round-trips, renderers remapped through prefabs (`tests/lod_group_api.rs`, `src/scene/lod_instance_tests.rs`) |

### `Time` — over the `Time` resource

| Setter | Status | Read-site |
|---|---|---|
| `SetTimeScale` | ✅ | sim — `time/mod.rs` scales `delta_time` the whole sim reads (`tests/time_scale.rs`) |
| `Pause` | ✅ | platform — the windowed frame loop (`main::advance_sim`, #283) reads `Time.paused` **before** it ticks and bypasses the wall-clock advance when set (proven by `tests/pause_step/`) |
| `Resume` | ✅ | platform — same read-site: a cleared `paused` returns the loop to the normal real-time `game.tick(delta_time)` path; also clears `pending_steps` (`tests/pause_step/`) |
| `Step` | ✅ | platform — while paused, `main::advance_sim` drains `pending_steps` one `FIXED_DELTA_TIME` tick at a time (the harness's fixed-dt step semantics), so windowed and headless stepping are frame-identical (`tests/pause_step/`) |

### `Tween` — over the tweened component's field (#424)

| Setter | Status | Read-site |
|---|---|---|
| `To` / `Sequence` | ✅ | each tick writes through the property's own authoring op (`Transform` fields directly, with the collider re-sync), so the read-site is that component's setter row above/below — `CanvasGroup.SetAlpha`, `Image.SetColor`, `Light.SetIntensity`, … (`src/scripting/tests_tweens.rs`, `tests/tweens.rs`) |

### `Camera` — over the shared `scene::Camera`

| Setter | Status | Read-site |
|---|---|---|
| `SetPosition` | ✅ | renderer — `app/camera_sync.rs` + view matrix |
| `SetYaw` / `SetPitch` | ✅ | renderer — `Camera::forward`/view matrix (pitch clamped) |
| `SetFov` | ✅ | renderer — projection matrix (clamped 1..179°) |

### `Light` — over `Entity.light`

| Setter | Status | Read-site |
|---|---|---|
| `SetColor` | ✅ | renderer — `apply_scene_lights` lighting uniform (ambient, directional); `clusters::local_lights` light array (point, spot, #434) |
| `SetIntensity` | ✅ | renderer — same (clamped ≥ 0) |
| `SetRange` | ✅ | renderer — point/spot attenuation, and the radius the light is binned into clusters with (#434) |
| `SetType` | ✅ | renderer — selects light path; unknown names ignored |
| `SetCastShadows` | ✅ | renderer — `clusters::local_lights` hands the request to the shadow-atlas plan (`passes::shadows::atlas::plan`, #468), whose tiles the forward shader samples (`src/render/passes/shadows/atlas/gpu_tests.rs`); round-trips |

### `Particles` — over `Entity.particles`

| Setter | Status | Read-site |
|---|---|---|
| `Emit` / `Burst` | ✅ | sim/renderer — `emit_at` spawns into the runtime buffer `app/particles.rs` advances and the renderer draws |
| `SetActive` | ✅ | sim — gates continuous emission |
| `SetRate` | ✅ | sim — continuous spawn cadence |
| `SetShape` | ✅ | sim — `EmitShape::sample` picks each spawn's offset + launch direction in `spawn_particle`; unknown kinds return `false` and change nothing |
| `SetDirection` | ✅ | sim — the shape's axis in `spawn_particle` |
| `SetLifetime` / `SetSpeed` / `SetSize` | ✅ | sim/renderer — sampled per spawn in `spawn_particle`; size reaches the renderer via `size_of` |
| `SetColor` | ✅ | renderer — start tint, scaled by the gradient in `color_of` |
| `SetSubEmitter` | ✅ | sim — `app/particles.rs` queues and dispatches the target's burst on birth/death/collision; unknown triggers return `false` |
| `Clear` | ✅ | renderer — empties the live buffer that's drawn |
| `SetRenderMode` / `GetRenderMode` | ✅ | renderer — `render.mode` picks the sprite quad in `particles.wgsl` (`corner_offset`), or routes the emitter to the forward solids (`passes/particles/mesh.rs`); unknown modes return `false` |
| `SetStretch` | ✅ | renderer — `stretch_length` → the instance's `stretch.w` |
| `SetMesh` | ✅ | renderer — `push_mesh_particles` loads the mesh once and draws it with the named scene material |
| `SetFlipbook` | ✅ | renderer — `Flipbook::frame_of` → the instance's `sheet`, `sheet_uv` in the shader; `random_start` draws each spawn's `start_frame` |
| `SetSoft` | ✅ | renderer — `soft_fade` against the read-only scene depth |
| `SetLit` | ✅ | renderer — `emitter_light` (probe DC or flat ambient) + `particle_light` over the lighting uniform |

### `Trail` — over `Entity.trail` (#441)

| Setter | Status | Read-site |
|---|---|---|
| `SetEmitting` / `SetTime` / `SetMinVertexDistance` | ✅ | sim — `TrailComponent::advance`, run by `app/trails.rs` each fixed tick, records and ages the points with them (`tests/ribbons/trail.rs`); round-trips |
| `Clear` | ✅ | renderer — empties the recorded points the ribbon pass draws |
| `SetWidth` / `SetWidthCurve` / `SetColor` / `SetColors` / `SetTexture` / `SetTextureMode` / `SetBlend` | ✅ | renderer — `render/passes/ribbons` builds the strip's width, colour and `u` from the style and picks the blend pipeline and texture (`tests/gpu/ribbons_screenshot.rs`); round-trips |

### `Line` — over `Entity.line` (#441)

| Setter | Status | Read-site |
|---|---|---|
| `SetPositions` / `SetPosition` / `SetPositionCount` / `SetUseWorldSpace` / `SetLoop` | ✅ | renderer — `render/passes/ribbons` places the strip through the points (local ones through the entity's world matrix), closing it when looping (`tests/gpu/ribbons_screenshot.rs`); round-trips (`tests/ribbons/line.rs`) |
| `SetWidth` / `SetWidthCurve` / `SetColor` / `SetColors` / `SetTexture` / `SetTextureMode` / `SetBlend` | ✅ | renderer — as `Trail`'s style row; round-trips |

### `Audio` — over the `AudioMaestro` resource (+ `Entity.audio`)

| Setter | Status | Read-site |
|---|---|---|
| `Play` | ✅ | platform (`AudioMaestro::play_source` → backend voice) + introspection log; the `AudioSource` it reads round-trips |
| `Stop` | ✅ | platform — drops the live voice; logs a Stop event |
| `SetVolume` | ✅ | platform — re-folds the live voice's gain (master × per-source) |
| `PlayAt` | ✅ | platform (`play_at` one-shot) + introspection log — the log entry is the one-shot's only trace |
| `SetMasterVolume` | ✅ | platform — re-folds every live voice; observable via `GetMasterVolume` |
| `SetSpeakerMode` | ✅ | platform — re-sends every live voice shaped for the mode (headphones narrows pan) and retunes the device's output stage (TV compression); observable via `GetSpeakerMode`, persisted as `audio.speaker_mode` |
| `SetOutputGroup` | ✅ | `AudioSource.output_group` — routes the voice's next `Play` into that mixer group (`PlayParams.group`); observable via `GetOutputGroup` and `Debug.Snapshot` |
| `CreateGroup` / `SetGroupVolume` / `SetGroupMute` / `SetGroupLowPass` / `SetGroupHighPass` / `SetGroupReverbSend` | ✅ | sim (`Mixer`) + platform — the group's kira track (volume, filters, reverb send, `device/groups.rs`); observable via `GetGroupState` / `GetGroups` |
| `DefineSnapshot` / `TransitionToSnapshot` | ✅ | sim — `Mixer::advance` blends on unscaled sim time each `LateUpdate`, then the moved groups reach the device; observable via `GetSnapshot` / `GetGroupState` |
| `AddDuck` / `ClearDucks` | ✅ | sim — duck envelopes stepped with the mixer; observable as `GetGroupState().duck` |
| `SetOcclusionSettings` | ✅ | sim — `AudioMaestro::occlude` (each `LateUpdate`) casts through the rapier world with the mask and budget and scales the muffle by the strength; observable via `GetOcclusionSettings` and `GetSpatial`'s `occlusion` |
| `SetOcclusionEnabled` | ✅ | `AudioSource.occlusion_enabled` — read by the cast each tick; observable via `GetOcclusionEnabled` and `Debug.Snapshot` |
| `GetReverbState` | ✅ | read-back only — the listener's zone blend `AudioMaestro::resolve_reverb` stores each `LateUpdate` (and sends to the reverb bus, `device/reverb.rs`) |

The maestro carries a **no-op backend** on the headless harness, so the *sound* is a
windowed-only side effect; the **introspection log + playing set** are the
device-free read-sites a play-test asserts on (deterministic — voice ids are a
monotone counter, the event tick is `Time.frameCount`). `AudioSource`'s authoring
fields (incl. the spatial fields stored for #213) round-trip through `SceneData`.

### `AudioReverbZone` — over `Entity.reverb_zone` (#469)

| Setter | Status | Read-site |
|---|---|---|
| `SetMinDistance` / `SetMaxDistance` / `SetPreset` / `SetParams` | ✅ | sim — `app/audio.rs::resolve_reverb` blends every active zone around the camera each `LateUpdate`, then the device's reverb bus retunes (`device/reverb.rs`); observable via `Audio.GetReverbState`, round-trips via the getters and `Debug.Snapshot` |

### `Decals` — over `Scene.decals`

| Setter | Status | Read-site |
|---|---|---|
| `Spawn` | ✅ | renderer — `render/decals/gpu.rs` uploads each decal and loads its maps into the decal atlas; the clusters bin it and `fs_main` folds it into the surface's material (#638); its `owner` / `lifetime` / `fade` (#639) are read by `Scene::decal_pose` each frame and aged by `app/decals.rs::tick_decals` on the fixed tick |
| `Remove` | ✅ | sim + renderer — drops the decal (or retires it to fade out) from `Scene.decals`, which the upload reads (`tests/decals_lifecycle_api.rs`) |
| `Clear` | ✅ | renderer — empties the uploaded set |

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
| `SetResolution` | ✅ | renderer/window — the shell (`shell/settings.rs`) reconfigures the wgpu surface when the cell changes (clamped ≥ 1) |
| `SetVsync` | ✅ | renderer/window — present-mode reconfigure |
| `SetFullscreen` | ✅ | window — winit fullscreen reconfigure |

### `Application` — over the shared `Application` resource (#431)

| Setter | Status | Read-site |
|---|---|---|
| `Quit` | ✅ | platform — the shell's frame loop takes the request (`shell::frame::quit_action`): the player exits, the editor stops Play; the harness stops stepping (`dev::harness::tick_unless_quit`, `tests/application_quit.rs`) |
| `SetStartupScene` | ✅ | round-trip — written to `project.rusty` (editor); the player loads that scene at boot (`shell::player::launch`) |
| `SetProductName` | ✅ | round-trip — same file; the player's window title |
| `SetWindowMode` | ✅ | round-trip — same file; the player's first-launch fullscreen default (`shell::player::video_defaults`) |

### `Storage` — over the `Storage` resource

| Setter | Status | Read-site |
|---|---|---|
| `Set` / `SetTable` | ✅ | round-trip — readable via `Get`/`GetTable`; flushed to disk at Stop/quit (#86) |
| `Delete` | ✅ | round-trip — removes the key from the same store |

### `Texture` — writes a `.png` the renderer loads (over the material map slots)

`Texture` writes files rather than mutating a component, so its "read-site" is the
renderer's texture loader: a baked PNG path handed to any `Material.Set*Map` is
loaded by `draw::upload_scene_assets` → `load_texture(path)` and bound for sampling,
exactly like an imported map. Proven by `tests/texture_api.rs` (a baked map wired
into a slot decodes at the requested resolution) and the bake tests
(`tests/procgen_bake.rs`: byte-identical determinism, per-slot sRGB/linear encoding,
metallic→B/roughness→G packing, seamless tiling).

| Setter | Status | Read-site |
|---|---|---|
| `Bake` | ✅ | renderer — writes a `.png` consumed by `load_texture` via the material map slots; per-slot glTF encoding applied on bake |
| `ToJson` | ✅ | round-trip — the recipe's canonical serde form; re-bakes byte-identically |

(`Texture.Ops` is read-only introspection — the op catalog with each op's params,
types and defaults (#411) — not a setter, so it doesn't add to the count.)

### `Shader` — writes a `.wgsl` the `ShaderRegistry` loads (over the pass contract)

`Shader` writes files rather than mutating a component, so its **read-site is the
engine's own shader loader**: a baked `<name>.wgsl` is exactly the file a
`ShaderRegistry` reads (`<base>/<name>.wgsl`), composes through the same `naga_oil`
`Composer` (with `common` registered), and compiles to a wgpu module. The bake
**validates by composing through that identical path before it writes** — so a baked
module is, by construction, one the engine can load: validated-at-bake is the same
code as load-time. A surface variant keeps the standard `vs_main`/`fs_main` + the
forward bind groups (it binds against the forward pipeline unchanged); a postfx
variant has the fullscreen `vs_fullscreen`/`fs_main` shape. Proven by the spine
tests (`src/shadergen/tests.rs`: a baked surface AND postfx variant compose cleanly,
an intentionally-broken recipe fails the bake with no file written, the same recipe
assembles byte-identical WGSL) and the API tests (`src/api/shader/mod.rs`).

| Setter | Status | Read-site |
|---|---|---|
| `Bake` | ✅ | renderer — writes a `.wgsl` a material naming it renders with (`Material.SetShader`, #396), composed through the `ShaderRegistry` path; **validated through the same `naga_oil` compose path at bake** (a non-composing module is rejected, never written) |
| `ToJson` | ✅ | round-trip — the recipe's canonical serde form; re-assembles byte-identically |

(`Shader.Validate` and `Shader.Blocks` are read-only introspection — a compose
dry-run and the block-catalog listing with each block's params — not setters, so they
don't add to the count.)

### `Sound` — writes a `.wav` the audio runtime decodes (over `AudioSource.clip`)

`Sound` writes files rather than mutating a component, so its **read-site is the
engine's own audio decoder**: a baked `.wav` is handed to `Audio.PlayAt` or stored as
an `AudioSource`'s `clip`, and `audio::device::decode::ClipCache::get_or_decode` decodes it
into PCM the maestro plays — the identical path an imported `.ogg`/`.wav` takes. The
bake — rendered by scorsese's zimmer crate (#413) and written by the adapter —
is exactly what that decoder expects (stereo 16-bit PCM at 44.1 kHz) and always
passes a limiter, so a baked clip can never arrive clipped. Proven by
`tests/sound_api.rs` (a baked one-shot fired through `Audio.PlayAt` shows up in the
maestro's event log under its baked path; a saved patch re-bakes byte-for-byte),
`tests/sound_song_api.rs` (a song track naming its patch by path resolves to the
same mix as the inline patch; an unreadable path fails and writes nothing) and the
module tests (`src/api/sound/bake_tests.rs`: the engine's own `ClipCache` decodes
the bake as stereo at 44.1 kHz; byte-identical determinism; no file written on a
rejected patch, note or option). zimmer's own render tests live in scorsese.

| Setter | Status | Read-site |
|---|---|---|
| `Bake` | ✅ | sim — writes a `.wav` decoded by `ClipCache` and played by the `AudioMaestro` (via `Audio.PlayAt` or an `AudioSource.clip`); limited at bake so it cannot clip |
| `ToJson` | ✅ | round-trip — the patch's canonical serde form; re-bakes byte-identically |
| `Level` (and every bake's second return) | ✅ | read-only report (#378) — zimmer's meter over the bake's own samples, or over the PCM `ClipCache`'s decoder produces for a file; proven by `src/api/sound/level_tests.rs` (a full-scale sine reads −3.01 / 0 dBFS; silence reads silent; an inter-sample overshoot reads a true peak over its sample peak; a bake's figures match `Sound.Level` of its file) |
| `Diff` (and the report's `sections` / `bands` / `tracks`) | ✅ | read-only report (#379) — zimmer's `Profile` sections, `Bands`, per-track `Layer` rows measured post-gain at the mixer, and `Difference`; proven by `src/api/sound/report_tests.rs` (halves 6 dB apart report it on their own rows; a 60 Hz tone sits in the low band only; a clip diffed with itself is `same`; a one-shot has no section or track rows; a song's rows carry its pattern names and a track with no notes reads silent) |
| `Survey` | ✅ | read-only report (#380) — counts over the patch/song documents themselves (zimmer's `SongSurvey` for songs, rusty's rows for one-shot patches); no bake, no decode, no write. Proven by `src/api/sound/survey/tests.rs` (a set of impacts rolls up to one kind and its cutoff span; the loudest track is gain × duty, not the highest gain; a set of one has no rollup; an unreadable document is skipped, an unresolved patch leaves its columns absent) |

### `Canvas` — over `Entity.canvas` (#417)

| Setter | Status | Read-site |
|---|---|---|
| `SetRenderMode` | ✅ | round-trips; single-valued (`ScreenSpaceOverlay`) until #429 adds the modes the UI render pass (#418) will branch on |
| `SetSortOrder` | ✅ | sim — `ui::UiLayout::compute` orders root canvases by it (draw / hit order); round-trips |
| `SetReferenceResolution` | ✅ | sim — `CanvasComponent::scale_factor` / `size` in the layout pass; round-trips (`tests/ui_api.rs`) |
| `SetMatchWidthOrHeight` | ✅ | sim — same scaler read; round-trips |

### `RectTransform` — over `Entity.rect_transform` (#417)

| Setter | Status | Read-site |
|---|---|---|
| `SetAnchorMin` / `SetAnchorMax` / `SetPivot` / `SetAnchoredPosition` / `SetSizeDelta` | ✅ | sim — `RectTransformComponent::layout_in` in the `LateUpdate` layout pass (`Resources::ui_layout`) and `UI.GetRect`; round-trips (`tests/ui_api.rs`) |

### `Image` — over `Entity.image` (#418)

| Setter | Status | Read-site |
|---|---|---|
| `SetColor` / `SetTexture` / `SetType` / `SetBorder` / `SetFillMethod` / `SetFillOrigin` / `SetFillAmount` / `SetFillClockwise` / `SetPreserveAspect` | ✅ | render — `render::ui::mesh::build_canvas_meshes` + `render::ui::geometry::image_triangles` in the UI pass (`tests/gpu/ui_hud_screenshot.rs`); round-trips (`tests/ui_graphics_api.rs`) |
| `SetRaycastTarget` | ✅ | sim — `ui::events::raycast` (`UI.Raycast`, pointer dispatch; `src/ui/events/raycast_tests.rs`); round-trips |
| `SetGradient` / `SetBlend` | ✅ | render — `render::ui::vertex::Fill` (the stops per vertex) and the batch's blend pipeline (`render::ui::blend`) in the UI pass (`tests/gpu/ui_shapes_screenshot.rs`); round-trips (`tests/ui_look_api.rs`) |

### `CanvasGroup` — over `Entity.canvas_group` (#418)

| Setter | Status | Read-site |
|---|---|---|
| `SetAlpha` | ✅ | render — multiplied down the subtree in `render::ui::mesh` (`tests/gpu/ui_hud_screenshot.rs`) |
| `SetInteractable` / `SetBlocksRaycasts` | ✅ | sim — `ui::events::is_interactable` (Selectable `Disabled`, interaction gating) and `ui::events::raycast` (pass-through); round-trips |

### `RectMask` — over `Entity.rect_mask` (#418)

| Setter | Status | Read-site |
|---|---|---|
| `SetPadding` | ✅ | render — the subtree's scissor rect in `render::ui::mesh`; round-trips |
| `SetFeather` | ✅ | render — the batch's soft clip (`render::ui::clip`, cut in `ui.wgsl`; `tests/gpu/ui_mask_screenshot.rs`); round-trips (`tests/ui_mask_api.rs`) |

### `Mask` — over `Entity.mask` (#428)

| Setter | Status | Read-site |
|---|---|---|
| `SetShowMaskGraphic` | ✅ | render — whether `render::ui::mesh` batches the mask's own graphic (`tests/gpu/ui_mask_screenshot.rs`); round-trips (`tests/ui_mask_api.rs`) |

### `BackdropFilter` — over `Entity.backdrop_filter` (#426)

| Setter | Status | Read-site |
|---|---|---|
| `SetBlurRadius` / `SetTint` / `SetSaturation` / `SetBrightness` | ✅ | render — the backdrop batch and its blur level in `render::ui::backdrop`, filtered in `ui.wgsl` (`tests/gpu/ui_backdrop_screenshot.rs`); round-trips (`tests/ui_mask_api.rs`) |

### `Text` — over `Entity.text` (#419)

| Setter | Status | Read-site |
|---|---|---|
| `SetText` / `SetFont` / `SetBoldFont` / `SetItalicFont` / `SetFontSize` / `SetColor` / `SetAlignment` / `SetWrap` / `SetOverflow` / `SetLineSpacing` / `SetLetterSpacing` / `SetAutoSize` / `SetRichText` | ✅ | sim — `ui::text::layout_text` (also `Text.GetLayout` / `GetPreferredSize`); render — `render::ui::text::quads` in the UI pass; round-trips (`tests/ui_text_api/`) |
| `SetOutline` / `SetShadow` / `SetGlow` | ✅ | render — the SDF parameters and shadow quads of `render::ui::text::quads`, cut in `ui.wgsl` (`tests/gpu/ui_text_screenshot.rs`); round-trips |
| `SetBlend` | ✅ | render — the batch's blend pipeline (`render::ui::blend`); round-trips (`tests/ui_look_api.rs`) |
| `SetRaycastTarget` | ✅ | sim — `ui::events::raycast`; round-trips |

### `Shape` — over `Entity.shape` (#425)

| Setter | Status | Read-site |
|---|---|---|
| `SetKind` / `SetCorner` / `SetRadius` / `SetInnerRadius` / `SetArc` / `SetThickness` / `SetDash` / `SetColor` / `SetGradient` / `SetBorder` / `SetShadow` / `SetGlow` / `SetBlend` | ✅ | render — `render::ui::shape` (the SDF quads) cut by `shape_distance` / `sdf_shape` in `ui.wgsl` (`tests/gpu/ui_shapes_screenshot.rs`); round-trips (`tests/ui_look_api.rs`) |
| `SetRaycastTarget` | ✅ | sim — `ui::events::raycast`; round-trips |

### `Selectable` — over `Entity.selectable` (#420)

| Setter | Status | Read-site |
|---|---|---|
| `SetInteractable` | ✅ | sim — `ui::events::is_interactable` (the `Disabled` state; press / drag / scroll / submit gating); round-trips (`tests/selectable_api.rs`) |
| `SetTransition` / `SetTargetGraphic` / `SetColor` / `SetFadeDuration` / `SetSprite` | ✅ | sim — `EventSystem::apply_transitions` writes the target's runtime `state_tint` / `override_texture`; render — multiplied / preferred in `render::ui::mesh` and `render::ui::text::emit` (`src/ui/events/transition_tests.rs`); round-trips |
| `SetNavigation` / `SetSelectOn` | ✅ | sim — `ui::events::focus` (arrow-key navigation; `src/ui/events/focus_tests.rs`); round-trips, references remapped through prefabs |

### `LayoutGroup` — over `Entity.layout_group` (#421)

| Setter | Status | Read-site |
|---|---|---|
| `SetKind` / `SetPadding` / `SetSpacing` / `SetChildAlignment` / `SetControlChildSize` / `SetChildForceExpand` | ✅ | sim — `ui::layout::group` places the children inside `UiLayout::compute` (`UI.GetRect`; `src/ui/layout/group_tests.rs`); round-trips (`tests/ui_layout_api.rs`) |
| `SetCellSize` / `SetConstraint` / `SetConstraintCount` / `SetStartCorner` / `SetStartVertical` | ✅ | sim — `ui::layout::grid` (`src/ui/layout/grid_tests.rs`); round-trips |

### `LayoutElement` — over `Entity.layout_element` (#421)

| Setter | Status | Read-site |
|---|---|---|
| `SetIgnoreLayout` / `SetMinSize` / `SetPreferredSize` / `SetFlexibleSize` | ✅ | sim — `ui::layout::sizes` (`layout_children`, `element_sizes`) read by the group pass (`src/ui/layout/group_tests.rs`); round-trips (`tests/ui_layout_api.rs`) |
| `SetFit` | ✅ | sim — `ui::layout::fit` resizes the rect in `UiLayout::compute` (`src/ui/layout/fit_tests.rs`); round-trips |

### `UI` — the event system (#420)

| Setter | Status | Read-site |
|---|---|---|
| `SetSelected` | ✅ | sim — `EventSystem::process` announces it (`OnDeselect` / `OnSelect`) and navigation moves from it (`tests/ui_events.rs`); runtime state, not saved |
| `Click` | ✅ | sim — writes `Input`'s pointer + `Mouse0` edges, read by the next tick's pointer dispatch (`tests/ui_events.rs`) |
| `Create` | ✅ | builds the widget tree through `ui_widgets::create_ui` (the Create ▸ UI menu's path): ordinary entities and components every UI read-site sees, plus the widget script the next tick loads; round-trips (`tests/ui_widgets/create.rs`, #422) |

### `Debug` (dev-only) — over `ConsoleLogs`

| Setter | Status | Read-site |
|---|---|---|
| `Log` / `Warn` / `Error` | ✅ | sim — appends to the console buffer the REPL/overlay render |
| `PreviewMaterial` | ✅ | reads `Scene.materials[name]` into `preview::build_preview_scene`'s `PreviewSubject::Material` — the Inspector Material card's one read-site — and renders it through the ordinary forward path (`tests/gpu/preview_material.rs`, #404) |

## Summary

| Status | Count |
|---|---|
| ✅ faithful | 125 |
| ⚠️ partial | 0 |
| ❌ no-op | 0 |

`Navigation.SetAgentRadius` became fully faithful in #277: the bake now **erodes the
walkable surface inward by the agent radius** (the standard Recast/Unity meaning), so the
radius has a live sim read-site and changes the baked result. It had been stored-but-inert
since #276 — a documented, conscious step, now closed. `Navigation.SetAgentHeight` lands
faithful at birth in #278 — its vertical companion: the bake **carves cells whose overhead
clearance is below the agent height** (no pathing under a low overhang / through a
crawlspace), the standard radius+height pair Unity exposes. Since #454 the navmesh is
layered, so the height drops a low span without touching the floors above it.

The full glTF-PBR map surface is now **faithful**. `Material.SetMetallicMap` /
`SetRoughnessMap` were wired by #202 (per-entity group(2) material bind group +
`shader.wgsl` sampling of `t_metallic`/`t_roughness`), closing the **#184** gap; the
issue's "canonical no-op" — `Material.SetTexture` — was already faithful
(`upload_scene_assets` lazily loads any texture path each frame). The flat **emissive
factor** (`SetEmissive`) was wired by #222 through the `EntityUniform` slot and the
`fs_main` add-after-lighting term (glows, and blooms when >1.0 via the HDR target).
**#207** closed the last two map no-ops: `SetNormalMap` now perturbs the shading
normal in tangent space (a `tangent` vertex attribute — read from the glTF `TANGENT`
accessor or generated from positions + UVs — feeds a TBN basis in `fs_main`), and
`SetEmissiveMap` modulates the emissive factor by the sampled `t_emissive` rgb. No
material-map setter is a write-only no-op anymore.

**#271** added standalone named-asset authoring (`Material.DefineAsset` /
`DefineAssetJson`, plus the `GetAsset` / `HasAsset` read-back): an agent can author a
reusable `MaterialAsset` into the library by name, decoupled from any entity. Its
read-site is the same per-entity material path — an entity that references the named
asset is sampled exactly like any material edited piecemeal — and the asset persists in
`SceneData`, so it is faithful on both the renderer and round-trip axes. The four verbs
take the faithful count from 65 to 69.

**#272** added agent-composed shader authoring (`Shader.Bake` / `BakeJson` / `ToJson`):
an agent composes a recipe from a curated WGSL building-block library and bakes a
contract-conformant `.wgsl` module. Its read-site is the engine's `ShaderRegistry` — the
same loader the shipped shaders use — and the bake **validates through that exact
`naga_oil` compose path before writing**, so a bad shader never reaches disk. The three
write/serialize verbs take the faithful count from 69 to 72.

**#319** surfaced a dynamic body's angular velocity (`Physics.SetAngularVelocity` /
`GetAngularVelocity`): the setter pushes `set_angvel` into rapier each tick and the
sync writes `body.angvel()` back onto the component — the same round-trip
`velocity` already had, so a read reflects the integrated spin and a set injects
one. The one write verb takes the faithful count from 72 to 73.

**#321** added the per-body collision-detection mode (`Physics.SetCollisionDetection`
/ `GetCollisionDetection`): the setter maps `Continuous` onto rapier's per-body CCD
switch (`ccd_enabled` at build, `enable_ccd` re-applied per tick), whose effect on
the simulation is proven by `physics/ccd_tests.rs` — a fast sphere tunnels through a
thin wall under Discrete and is stopped by it under Continuous. (Since rapier 0.36,
#754, every fast dynamic body is swept against static geometry, so the proof uses a
kinematic wall: Continuous is what adds the sweep against moving bodies.) The one write verb
takes the faithful count from 73 to 74.

**#410** folded every `*Json` twin into its verb: `Texture.Bake`, `Shader.Bake`,
`Sound.Bake` / `BakeSong` and `Material.DefineAsset` (and every other verb taking a
recipe) accept either the Lua table or its JSON string, decoded by the one shared
`lua_json::recipe_from_lua`. `BakeJson` / `BakeSongJson` / `DefineAssetJson` are
gone; no read-site changed, so every row above keeps its status.
