## `Debug` — **dev builds only**

Mirrors Unity's `[Conditional]` `Debug`. Registered only under the `dev` Cargo
feature and stripped from ship builds; calling it from a ship build is a no-op
because the namespace is absent.

| Function | Signature | Returns |
|---|---|---|
| `Debug.Log` | `(message)` | — |
| `Debug.Warn` | `(message)` | — |
| `Debug.Error` | `(message)` | — |
| `Debug.Snapshot` | `([opts])` | a pretty **JSON string**: the whole live world (below). Skeleton bones are left out unless `opts.bones` is `true` |
| `Debug.SnapshotEntity` | `(id)` | a pretty **JSON string**: one entity (below), or `null` if absent |
| `Debug.Preview` | `(asset_path, out_png [, opts])` | the written path, or `nil` if the machine has no GPU |
| `Debug.PreviewMaterial` | `(name, out_png [, opts])` | the written path, or `nil` if the machine has no GPU |
| `Debug.Stats` | `()` | a **table**: the frame stats — timings and counters (below) |

### The preview — eyes on an authored asset (#353)

`Debug.Snapshot` is the structured read; **`Debug.Preview` is the visual one**. It
renders an asset in the same isolated preview scene the editor's Inspector "Preview"
tab shows a human — one stand-in mesh, a procedural sky, one key light — and writes it
to a PNG, **headlessly**. That closes the authoring loop for shaders in particular:
`Shader.Bake` proves a module *compiles*, and this shows what it *renders*.

```lua
-- bake -> preview -> look -> iterate, in one line
local png = Debug.Preview(Shader.Bake(recipe), "out/preview.png")

-- a texture on the cube instead of the default sphere, at 256²
Debug.Preview("project/assets/brick.png", "out/brick.png", { mesh = "cube", resolution = 256 })
```

**Previewable kinds** — the same set the Inspector tab accepts, dispatched on the file
extension: models (`gltf` / `glb` / `obj` / `fbx`, rendered as themselves), textures
(`png` / `tga` / `jpg` / `jpeg`, applied as the base-color map), and shaders (`wgsl`,
which shades the stand-in mesh). Material assets have no on-disk file, so they have
their own call, below.

**Material assets — `Debug.PreviewMaterial(name, out_png [, opts])`** (#404) renders a
named asset from the scene's material library (anything `Material.DefineAsset` defined,
or a glTF import brought in) on the stand-in mesh — the same picture its Inspector
Material card shows in its Preview tab. It is the assembled result, every map at once:
albedo, metallic/roughness, normal and emissive maps, the render mode, and the authored
`shader` with its stored runtime params (`Material.SetAssetShaderParam` values, else the
baked defaults) and its extra texture slots such as `mask`. An `rt:<name>` render
texture in any slot samples **white** here: the preview scene is isolated and has no
camera drawing that texture. A name the library doesn't hold raises, naming it.

```lua
-- define -> look -> iterate
Material.DefineAsset("brick", { base_color_map = "brick_albedo.png", roughness = 0.8 })
Debug.PreviewMaterial("brick", "out/brick_mat.png", { mesh = "cube" })
```

**Options** (all optional): `mesh` is `"sphere"` (default), `"cube"` or `"suzanne"` and
is ignored for models; `resolution` is the square output's edge in pixels (default
`512`, clamped to `16`–`4096`).

The framing is **fixed** — there are deliberately no camera parameters. Orbit/zoom is
the human affordance in the editor; an agent needs the shot reproducible so two
captures of the same asset are comparable pixel-for-pixel. Parent directories of
`out_png` are created for you.

Errors are raised for a caller mistake (a path that doesn't exist, an extension with no
preview story, an unknown `mesh` name) so a typo can't quietly return a picture of an
empty sky. The one non-error miss is a machine with no GPU or software adapter, where
the call **returns `nil`** after a warning rather than failing the run.

### The frame stats — performance you can read and assert (#433)

`Debug.Stats()` is the perf read: the numbers an agent needs to prove a rendering or
gameplay change is cheaper, since it never *feels* the frame rate. Each metric is a
`{ last, min, avg, max, samples }` table summarising every frame of the **current Play
session** (the stats restart when Play starts):

```lua
local s = Debug.Stats()
print(s.frames, s.fixed_update_ms.avg, s.systems.update_scripts.max, s.entities.last)
```

| Metric | What it counts |
|---|---|
| `frame_ms` | CPU ms of the whole per-frame schedule (the four stages below) |
| `fixed_update_ms` / `update_ms` / `late_update_ms` / `render_stage_ms` | CPU ms per schedule stage. `render_stage_ms` is the `Render` *stage* (draw-data prep), not the GPU |
| `systems.<name>` | CPU ms per registered system (`update_scripts`, `step_physics`, `tick_nav`, …) |
| `entities` / `rigid_bodies` / `nav_agents` / `particles` / `scripts` | world counters: live entities, RigidBody components, NavMeshAgents, live particles, loaded script instances |
| `draw_calls` | geometry draw calls: solids (mesh particles among them), transparents, shadow casters, the SSAO depth prepass, decals, sprite-particle batches, UI batches (post-FX and skybox excluded). Copies of one mesh + material are **one instanced call** (#470), so this tracks distinct looks, not entity count |
| `triangles` | triangles submitted by the solid, transparent, shadow and SSAO-prepass draws, every instance counted |
| `visible_entities` / `culled_entities` | mesh entities drawn / skipped by the frustum cull, summed over the camera stack |
| `lod_hidden_entities` | mesh entities skipped because their `LODGroup` showed another level (#472), summed over the camera stack |
| `lights` / `lights_dropped` | active lights, and those left unlit: directional lights past 4, every ambient but the last, and per camera the point/spot lights in view past the clustered budget of 256 (the farthest go first, #434) |
| `lights_visible` / `lights_culled` | point/spot lights binned into at least one light cluster / outside a camera's view and skipped before binning, so they cost nothing (#434); summed over the camera stack |
| `light_cluster_refs` | entries in the cluster light lists (#434): the (cluster, light) pairs shaders may visit, summed over the camera stack — the clustered lighting's workload |
| `light_bin_us` | CPU **microseconds** spent binning lights into clusters (#434), summed over the camera stack — wall-clock |
| `shadow_draws` / `ui_draws` | shadow-caster draw calls (one per caster mesh, instanced, #470; the sun's cascades and the point/spot shadow atlas) / UI batches |
| `shadowed_lights` / `shadow_lights_dropped` | point/spot lights given a shadow in the atlas this frame / ones that cast shadows and reach the view but found no room, so they shade unshadowed (the least important go first, #468) |
| `shadow_atlas_tiles` / `shadow_atlas_texels` | atlas tiles drawn (one per spotlight, six per point light) / the texels they cover, out of 2048² = 4194304 (#468) |
| `shadow_atlas_cached` / `shadow_atlas_rebaked` | atlas tiles whose static casters came from the cache / were re-drawn this frame (#694). A tile re-bakes when its light moves or turns, when it gets another place or size in the atlas, or when the editor changes static geometry; a still scene reads all cached |
| `ssao_samples` | depth taps the SSAO pass traced (occlusion texels × the tier's samples, #436); `0` when AO is off or on the Low tier |
| `particles_drawn` | particles the renderer drew — sprite instances plus mesh particles, summed over the camera stack (#440). Their draws are in `draw_calls`: one per merged sprite batch, one per instanced run of mesh particles |
| `render_texture_draws` | cameras drawn into render textures this frame (#430) — their geometry is already in `draw_calls` / `triangles`; a camera skipped (unreferenced, or between its `update_every` frames) is not counted |
| `ui_mask_passes` / `ui_blur_passes` | UI `Mask` coverage textures rendered (#428) / fullscreen passes of the UI backdrop blur, its composite included — `0` with no backdrop visible (#426) |
| `renderer_ms` | CPU ms `Renderer::render` took to record the frame |

Keys ending in `_ms`, and `light_bin_us`, are **wall-clock** and differ run to run; everything else is a
count and is deterministic for a deterministic run. The timings are measured *around*
the sim by the dev layer and never fed back into it, so reading them cannot change a
replay. Render counters appear only on frames something rendered through the headless
renderer (a harness `Harness.Screenshot`); a run that never renders reports timings and
world counters only. Before any Play frame, `frames` is `0` and no metric is present.

Scenarios get the same table as `Harness.Stats()` and can turn it into pass/fail with
`Harness.AssertBudget{ draw_calls = 2000, fixed_update_ms = 4 }` — see *Performance
budgets* in `docs/testing.md`.

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

A skinned character's skeleton is ~65 bone entities (#453), so bones are **left
out** of `entities` by default. `Debug.Snapshot({ bones = true })` includes them,
each marked `"bone": true`. `Debug.SnapshotEntity(id)` reads a bone like any
entity, and `Animator.GetBone(id, name)` finds one by name.

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
                 "shape": { "kind": "Box", "size": [x,y,z] },   // or Capsule: radius, height, axis
                 "material": { "friction": 0.5, "bounciness": 0.0,
                               "friction_combine": "Average", "bounce_combine": "Average" } },
  "rigidbody": { "active": true, "is_kinematic": false, "mass": .., "velocity": [x,y,z],
                 "use_gravity": true },
  "camera":    { "active": true, "fov": .., "near": .., "far": .., "culling_mask": ..,
                 "render_order": 0, "projection": "Perspective",
                 "target_texture": null },                      // or "rt:<name>" (#430)
  "nav_agent": { "active": true, "radius": .., "target": [x,y,z], "speed": .., .. },
  "particles": { "active": true, "texture": null, "rate": .., "lifetime": .., .. },
  "animator":  { "clip": "Idle", "time": .., "speed": .., "playing": true,
                 "loop": false, "paused": false,
                 "parameters": { "Jump": { "Trigger": false }, "speed": { "Float": 4.2 } },
                 "graph": "guard.animgraph", "graph_enabled": true, "node": "Locomotion",
                 "layers": [ { "name": "UpperBody", "weight": 1.0, "node": "Aim" } ] },
  "audio":     { "clip": "music/theme.ogg", "volume": .., "loop": false,
                 "play_on_start": false, "is_time_scaled": true,
                 "spatial_blend": 0.0, "initial_distance": .., "final_distance": ..,
                 "output_group": "", "occlusion_enabled": true },
  "reverb_zone": { "min_distance": 10.0, "max_distance": 15.0, "preset": "Room",
                   "decay_time": .., "pre_delay": .., "damping": .., "wet": .. },
  "canvas":    { "render_mode": "ScreenSpaceOverlay", "sort_order": 0,
                 "reference_resolution": [1920, 1080], "match_width_or_height": 0.0,
                 "pixels_per_unit": 100.0, "plane_distance": 1.0, "tilt": [x,y],
                 "sway": 0.0 },
  "rect_transform": { "anchor_min": [x,y], "anchor_max": [x,y], "pivot": [x,y],
                      "anchored_position": [x,y], "size_delta": [x,y],
                      "world_anchor": null },  // or { "target", "offset": [x,y,z],
                      // "clamp_to_screen_edge", "edge_padding",
                      // "rotate_toward_target", "hide_when_behind" } on a marker
  "image":     { "color": [r,g,b,a], "texture": null, "type": "Filled",
                 "border": [l,b,r,t], "fill_method": "Horizontal", "fill_origin": "Left",
                 "fill_amount": 0.5, "fill_clockwise": true, "preserve_aspect": false,
                 "raycast_target": true },
  "canvas_group": { "alpha": 1.0, "interactable": true, "blocks_raycasts": true },
  "rect_mask": { "padding": [l,b,r,t] },
  "ui_rect":   { "x": .., "y": .., "width": .., "height": ..,      // UI.GetRect's shape
                 "screen": { "x": .., "y": .., "width": .., "height": .. },
                 "corners": [ { "x": .., "y": .. }, ... ], "canvas": 3, "scale_factor": ..,
                 "world": false }
}
```

Every per-component key is present only when the entity carries that component
(absent ones serialize as `null`); `bounds` comes from the mesh when geometry is
present, else the collider, else `null`. `ui_rect` is the element's computed UI
layout (see `UI.GetRect`) — `null` unless it is a canvas or laid out under one —
so the agent can reason about the UI without a screenshot. The shape is shared with the harness's
`Harness.Snapshot` (the play-testing path), so the read is identical wherever it's
taken.
