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
| `Debug.AudioEvents` | `([after_seq])` | an **array** of the audio event log's entries whose `seq` is greater than `after_seq` (default `0`, all), oldest first, **and** the oldest `seq` the log still holds (below) |
| `Debug.GetAdaptedExposure` | `()` | `number` (EV): the automatic exposure the screen last applied, read back from the GPU a frame or so late (exactly, in a headless capture); `nil` before an `Automatic` frame has metered (#1444; see `Graphics.md`, *Automatic exposure*) |
| `Debug.AudioVoices` | `()` | an **array**: one roster row per entity carrying an `AudioSource` (below) |

### The preview — eyes on an authored asset (#353)

`Debug.Snapshot` is the structured read; **`Debug.Preview` is the visual one**. It
renders an asset in the same isolated preview scene the editor's Inspector "Preview"
tab shows a human — one stand-in mesh, a procedural sky, one key light — and writes it
to a PNG, **headlessly**. That closes the authoring loop for shaders in particular:
`Shader.Bake` proves a module *compiles*, and this shows what it *renders*.

```lua no-run
-- bake -> preview -> look -> iterate, in one line
local png = Debug.Preview(Shader.Bake(recipe), "out/preview.png")

-- a texture on the cube instead of the default sphere, at 256²
Debug.Preview("assets/brick.png", "out/brick.png", { mesh = "cube", resolution = 256 })
```

**Previewable kinds** — the same set the Inspector tab accepts, dispatched on the file
extension: models (`gltf` / `glb` / `obj` / `fbx` / `blend`, rendered as themselves,
centred and scaled to fit the frame by their bounds, so a 1 cm or a 50 m model both fill it), textures
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

```lua no-run
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
session** (the stats restart when Play starts). Until the first frame runs there is
nothing to summarise: `frames` is `0`, `systems` is empty and every metric is absent
(`nil`), so check `frames` before reading one:

```lua
local s = Debug.Stats()
if s.frames > 0 then
  print(s.frames, s.fixed_update_ms.avg, s.systems.update_scripts.max, s.entities.last)
end
```

| Metric | What it counts |
|---|---|
| `frame_ms` | CPU ms of the whole per-frame schedule (the four stages below) |
| `fixed_update_ms` / `update_ms` / `late_update_ms` / `render_stage_ms` | CPU ms per schedule stage. `render_stage_ms` is the `Render` *stage* (draw-data prep), not the GPU |
| `systems.<name>` | CPU ms per registered system (`update_scripts`, `step_physics`, `tick_nav`, …) |
| `entities` / `rigid_bodies` / `nav_agents` / `particles` / `scripts` | world counters: live entities, RigidBody components, NavMeshAgents, live particles, loaded script instances |
| `draw_calls` | geometry draw calls: solids (mesh particles among them), transparents, shadow casters, the SSAO depth prepass, sprite-particle batches, UI batches (post-FX and skybox excluded; decals draw nothing of their own, #638). Copies of one mesh + material are **one instanced call** (#470), so this tracks distinct looks, not entity count |
| `triangles` | triangles submitted by the solid, transparent, shadow and SSAO-prepass draws, every instance counted |
| `visible_entities` / `culled_entities` | mesh entities drawn / skipped by the frustum cull, summed over the camera stack |
| `lod_hidden_entities` | mesh entities skipped because their `LODGroup` showed another level (#472), summed over the camera stack |
| `lights` / `lights_dropped` | active lights, and those left unlit: realtime directional lights past 4, every ambient but the last, and per camera the point/spot lights in view past the clustered budget of 64 (the least contributing go first, #434, #834) |
| `cluster_lights_dropped` | (cluster, light) entries cut because more than 32 point/spot lights reached one cluster, the least contributing there first (#834); non-zero means some corner of the view is over the per-pixel budget |
| `lights_visible` / `lights_culled` | point/spot lights binned into at least one light cluster / outside a camera's view and skipped before binning, so they cost nothing (#434); summed over the camera stack |
| `light_cluster_refs` | entries in the cluster light lists (#434): the (cluster, light) pairs shaders may visit, summed over the camera stack — the clustered lighting's workload |
| `light_bin_us` | CPU **microseconds** spent binning lights and decals into clusters (#434, #638), summed over the camera stack — wall-clock |
| `decals_visible` / `decal_cluster_refs` | decals binned into at least one cluster / the (cluster, decal) pairs shaders may visit (#638), summed over the camera stack; a stacked `DepthOnly` camera (a viewmodel) bins none |
| `decal_maps_dropped` | decal maps drawn without their texture this frame because more distinct maps were in use than the decal atlas's 32 layers (#638) |
| `shadow_draws` / `ui_draws` | shadow-caster draw calls (one per caster mesh, instanced, #470; the sun's cascades and the point/spot shadow atlas) / UI batches |
| `shadowed_lights` / `shadow_lights_dropped` | point/spot lights given a shadow in the atlas this frame / ones that cast shadows and are shaded (within the base camera's light budget of 64) but found no room, so they shade unshadowed (the least important go first, #468). A light the budget drops gets no tiles and is counted in `lights_dropped` only, never here (#873) |
| `shadow_atlas_tiles` / `shadow_atlas_texels` | atlas tiles drawn (one per spotlight, six per point light) / the texels they cover, out of 2048² = 4194304 (#468) |
| `shadow_atlas_cached` / `shadow_atlas_rebaked` | atlas tiles whose static casters came from the cache / were re-drawn this frame (#694). A tile re-bakes when its light moves or turns, when it gets another place or size in the atlas, or when the editor changes static geometry; a still scene reads all cached |
| `ssao_samples` | depth taps the SSAO pass traced (occlusion texels × the tier's samples, #436); `0` when AO is off or on the Low tier |
| `particles_drawn` | particles the renderer drew — sprite instances plus mesh particles, summed over the camera stack (#440). Their draws are in `draw_calls`: one per merged sprite batch, one per instanced run of mesh particles |
| `render_texture_draws` | cameras drawn into render textures this frame (#430) — their geometry is already in `draw_calls` / `triangles`; a camera skipped (unreferenced, or between its `update_every` frames) is not counted |
| `velocity_passes` | opaque scene passes that wrote a velocity buffer (#1435): one per stacked camera of each view whose base camera runs motion blur, depth of field or SSR (SSR's holds only depths, #1526); `0` when none does, and always on the Low tier |
| `primed_passes` | opaque scene passes that started from their SSAO depth prepass (#1499): one per camera that ran SSAO, which then shades each lit pixel once; `0` with AO off, never for a camera with a forward override of its own (the shader preview), and never on Metal: Apple GPUs are tile-based and remove hidden opaque fragments themselves (Unity URP's *Auto* depth priming is off on tile-based targets too) |
| `sky_reflection_bakes` | sky reflection cubes baked this frame (#1553): one when the scene's panorama changed, or on the first frame showing the procedural sky; `0` on every frame the sky holds (the procedural sky's tint never rebakes it) |
| `ssr_surface_passes` | opaque scene passes that wrote SSR's surface targets (#1526): one per view whose base camera runs SSR (High tier only); a stacked overlay camera (a viewmodel) draws with the plain pipelines and writes none, so it never counts |
| `opaque_copies` | opaque copies made this frame (#1523), Unity's *Opaque Texture*: one per camera whose visible translucents refract or absorb (`Material.SetRefractionModel`, `SetTransmittanceColor`) or trace SSR (`Material.SetReceiveSsr`, on a base camera running SSR, #1570), drawn after its opaque pass and SSR; `0` on a frame showing none, which skips the copy |
| `texture_bytes` | GPU bytes of the image textures the renderer holds (material maps, UI images, particle and render textures, and the uncompressed copy a slot that cannot read a compressed one gets), every mip included (#1574): the resident total, not a per-frame count. What *Compression* and *Max Size* (`Assets.SetTextureImportSettings`) shrink: a 2048² map is 21.3 MiB uncompressed with mips, 2.7 MiB as BC4, 5.3 MiB as BC5 or BC7 |
| `grade_lut_bakes` | views whose colour-grading LUT was rebaked this frame (#1440): a grading knob or the tonemap changed, or the view is new; `0` on every frame the grade holds |
| `ui_mask_passes` / `ui_blur_passes` | UI `Mask` coverage textures rendered (#428) / fullscreen passes of the UI backdrop blur, its composite included — `0` with no backdrop visible (#426) |
| `renderer_ms` | CPU ms `Renderer::render` took to record the frame |
| `gpu_ms` | GPU ms the frame's render passes kept the GPU busy (#835): the sum of `gpu_passes` below. **Only where the adapter has timestamp queries** (Metal, most Vulkan GPUs; not lavapipe) — elsewhere the key is absent, never `0` |
| `gpu_passes.<pass>` | GPU ms per kind of render pass, in the same `{ last, min, avg, max, samples }` shape, under stable names (below). Absent as a whole without timestamps |

**GPU time per pass (#835).** Each render pass writes a GPU timestamp at its start
and end; the renderer resolves them after the frame and reads them back without
ever stalling it (the live loop gets them a frame or two later; the harness waits
for its frames anyway, so each reports its own). The passes are grouped by kind,
each kind's passes summed, under names that never change so two reports diff:

| `gpu_passes` key | What it covers |
|---|---|
| `sky_reflection` | the sky's reflection cube (#1553): the capture and GGX prefilter of the panorama or the procedural sky, on the frames the sky changed (counted in `sky_reflection_bakes`) |
| `shadows` | the sun's cascades (static bake + dynamic casters) and the point/spot shadow atlas |
| `ssao_prepass` | the SSAO depth prepass (#1578): every visible solid's depth into the view's depth target, which a primed forward pass starts from |
| `ssao` | the SSAO occlusion pass, at its quality's resolution |
| `ssao_blur` | the SSAO blur at the occlusion's resolution, and the depth-aware upsample to full resolution when that is lower |
| `forward` | the opaque scene pass: solids, decals folded in, skybox — every camera's, render-texture cameras included |
| `ssr` | screen-space reflections (#1526): the march over the base camera's opaque image (High tier only); the copy of that image it reads is recorded before the timed pass, so it isn't counted |
| `transparent` | translucent solids |
| `ribbons` | trails and lines |
| `particles` | particle systems |
| `depth_of_field` | depth of field (#1445): its circle-of-confusion, prefilter, blur and composite passes |
| `motion_blur` | per-object motion blur (#1436): its TileMax and NeighborMax tile passes and the gather |
| `lens_flare` | the screen-space lens flare (#1443): its flare pass (the composite adds it, under `post_fx`) |
| `post_fx` | bloom, the composite, authored effects, FXAA |
| `outline` | the editor's selection outline (its mask, dilation and ring) and the gizmos drawn over it after post-FX: grid, collider boxes, axis arrows, probes (editor only) |
| `ui` | world and screen UI, Mask textures and the backdrop blur |

A kind appears once a frame ran it (`samples` counts those frames). GPUs overlap
neighbouring passes — a tiled GPU starts the next pass's vertices while the last
one's fragments still run — so each pass is charged its **exclusive** time: from
where the work queued before it ended (or its own start, if later) to its own end.
The kinds therefore add up to `gpu_ms`, and a heavy pass shows as heavy instead of
smearing into the light ones behind it. Like every `_ms` key, these are wall-clock
and differ run to run, and they are this GPU's: compare them on one machine.

Keys ending in `_ms`, and `light_bin_us`, are **wall-clock** and differ run to run; everything else is a
count and is deterministic for a deterministic run. The timings are measured *around*
the sim by the dev layer and never fed back into it, so reading them cannot change a
replay. Render counters (and GPU times) appear only on frames something rendered through the
headless renderer (a harness `Harness.Screenshot`, or `Harness.Render`, which draws
without writing a PNG); a run that never renders reports timings and world counters
only. Before any Play frame, `frames` is `0` and no metric is present.

Scenarios get the same table as `Harness.Stats()` and can turn it into pass/fail with
`Harness.AssertBudget{ draw_calls = 2000, fixed_update_ms = 4 }` — see *Performance
budgets* in `docs/testing.md`.

### The audio reads — what played, where, and by whom (#1247)

`Debug.Snapshot` shows each entity's `AudioSource` *component*. An `Audio.PlayAt`
one-shot leaves no component, so the snapshot never sees it; the audio maestro's
**event log** does. Both reads work with no sound card: the headless backend makes
no sound, but every action is logged all the same.

`Debug.AudioEvents([after_seq])` returns the log's entries, oldest first, as plain
tables:

| Field | Meaning |
|---|---|
| `seq` | the event's sequence number: from `1`, one per logged event, never reused while the engine runs. The reader's cursor |
| `kind` | `"Play"` (an `AudioSource` started: `Audio.Play` or `play_on_start`), `"Stop"` (`Audio.Stop`) or `"PlayAt"` (a one-shot) |
| `tick` | the frame it happened on (`Time.frameCount`), deterministic. Every action in a frame shares it, so it is not a cursor |
| `clip` | the clip path (always `""` for a `Stop`) |
| `entity` | the source entity; for `PlayAt` the emitter passed, else `0` |
| `position` | `{ x, y, z }`, where it played: the entity's world position, or `PlayAt`'s point |
| `volume` | the per-source volume asked for, before the master (`0` for a `Stop`) |

A reader keeps the last `seq` it saw and passes it back: an action logged later in
the same frame as the poll still comes back next time. The log is a ring of the
**last 1024** entries and is **cleared on entering Play**; `seq` keeps counting
across the clear, so a cursor from an earlier run returns the new run's events,
never a repeat. The **second return value** is the oldest `seq` still held (the
next `seq` to be given when the log is empty): when it is greater than
`after_seq + 1`, the events in between rolled off the ring or were cleared, and are
gone.

```lua
Audio.PlayAt("sfx/shot.ogg", 1, 2, 3)
local seen = 0
local events, oldest = Debug.AudioEvents(seen)
if oldest > seen + 1 then print("missed", oldest - seen - 1, "events") end
for _, e in ipairs(events) do
  print(e.seq, e.tick, e.kind, e.clip, e.position.x, e.position.y, e.position.z)
  seen = e.seq
end
```

`Debug.AudioVoices()` is the live roster: one row per entity carrying an
`AudioSource`, in creation order, whether it is playing or not, heard from the active
camera (the listener). A one-shot has no row; read it from the log.

| Field | Meaning |
|---|---|
| `entity` | the owning entity |
| `clip` / `volume` / `looping` / `is_time_scaled` | the `AudioSource`'s own fields |
| `playing` | whether the maestro has its voice live |
| `spatial` | `{ gain, pan }` as `Audio.GetSpatial` resolves it: gain after rolloff, the 2D↔3D blend and the occlusion muffle; pan in `[-1, 1]` |
| `occlusion` | the smoothed occlusion factor in `[0, 1]`, `0` when not playing |

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
  "id": 1, "name": "Crate", "active": true, "active_in_hierarchy": true, "static": false, "layer": 0,
  "parent": null, "children": [],
  "components": ["Mesh", "Material", "Collider"],   // optional-component inventory
  "transform": { "pos": [x,y,z], "rot": [x,y,z], "scale": [x,y,z] },  // rot = Euler°
  "bounds": { "min": [x,y,z], "max": [x,y,z] },     // world-space AABB, or null
  "scripts": ["assets/scripts/foo.lua"],
  "mesh":      { "primitive_type": "Box", "asset_ref": "models/crates.glb::Barrel" },
                                // + "subdivisions" when not 1, "cast_shadows" when not "On"
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
