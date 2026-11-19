## `Debug` — **dev builds only**

Mirrors Unity's `[Conditional]` `Debug`. Registered only under the `dev` Cargo
feature and stripped from ship builds. Unlike Unity's, a call is **not** compiled
away there: the `Debug` global is absent, so `Debug.DrawRay(...)` in a shipped
player raises "attempt to index a nil value" every time it runs. A game script
that ships guards each call:

```lua
if Debug then Debug.Log("only in a dev build") end
```

| Function | Signature | Returns |
|---|---|---|
| `Debug.Log` | `(message)` | — |
| `Debug.Warn` | `(message)` | — |
| `Debug.Error` | `(message)` | — |
| `Debug.Mark` | `(label [, value])` | — : notes a game fact (`value` plain data: a number, string, boolean or a table of them) in the recording demo's index, at this tick, for `Harness.FindInDemo` to find; a no-op when no demo records (#1597, `docs/testing.md`, *Finding a moment*) |
| `Debug.Snapshot` | `([opts])` | a pretty **JSON string**: the whole live world (below). Skeleton bones are left out unless `opts.bones` is `true` |
| `Debug.SnapshotEntity` | `(id)` | a pretty **JSON string**: one entity (below), or `null` if absent |
| `Debug.Preview` | `(asset_path, out_png [, opts])` | the written path, or `nil` if the machine has no GPU |
| `Debug.PreviewMaterial` | `(name, out_png [, opts])` | the written path, or `nil` if the machine has no GPU |
| `Debug.Stats` | `([opts])` | a **table**: the frame stats — timings and counters (below) of the current Play session; `opts.window = n` (a whole number ≥ 1, or `math.huge` for everything kept) summarises only the newest `n` frames, edit mode's too; any other option key raises |
| `Debug.FrameLog` | `([opts])` | a **table** `{ path, records, skipped, truncated, dropped }`: a slice of the rolling frame log on disk (below), oldest first; `opts` are `last_seconds`, `over_ms`, `channel` and `limit` (a look-back is returned whole; without one, the newest 600); `truncated` is at least how many matches a limit left out; any other key raises |
| `Debug.SetLogChannels` | `(switches)` | — : turns the frame log's named channels on or off (`{ audio = false }`), kept in `project.rusty` when a project is open; an unknown channel raises and changes nothing |
| `Debug.GetLogChannels` | `()` | a **table**: each channel's name → `true` / `false` |
| `Debug.AudioEvents` | `([after_seq])` | an **array** of the audio event log's entries whose `seq` is greater than `after_seq` (default `0`, all), oldest first, **and** the oldest `seq` the log still holds (below) |
| `Debug.GetAdaptedExposure` | `()` | `number` (EV): the automatic exposure the screen last applied, read back from the GPU a frame or so late (exactly, in a headless capture); `nil` before an `Automatic` frame has metered (#1444; see `Graphics.md`, *Automatic exposure*) |
| `Debug.AudioVoices` | `()` | an **array**: one roster row per entity carrying an `AudioSource` (below) |
| `Debug.DrawLine` | `(fx, fy, fz, tx, ty, tz [, r, g, b, a [, duration]])` | — : a line from `f` to `t` in the colour (default white), held `duration` seconds of sim time (default `0`: one frame). See *Debug lines* (#1882) |
| `Debug.DrawRay` | `(ox, oy, oz, dx, dy, dz [, r, g, b, a [, duration]])` | — : the line from `o` to `o + d` (the direction's length is the ray's), as `DrawLine` |

### Debug lines (#1882)

Unity's `Debug.DrawLine` / `Debug.DrawRay`: a line seen, never collided with. It
draws in the editor's Scene view always, and over the game when its gizmos are on:
the Game view's **Gizmos** toggle (`Editor.SetGizmo("GameGizmos", true)`), or
`Harness.Screenshot(path, { gizmos = true })`
(`docs/testing.md`, which also has its `settle` option), with every collider's shape beside it. A plain game frame
never shows one.

- Vectors and the colour cross flat, as everywhere in the API: six numbers, then
  all four of `r, g, b, a` (0..1) or none, then `duration`. Any other count raises.
- `duration` is **counted in fixed ticks** (1/60 s each, rounded up), never the
  clock, so a replay draws the same lines on the same frames. The ticks are
  unscaled: under `Time.SetTimeScale(0.25)` a 2 s line lasts 120 ticks, which is
  0.5 s of `Time.time`. A debug line is the developer's overlay, not gameplay, so
  it keeps to the tick rate, not to the game's clock. `0` (or none) is
  one tick: drawn from `Update`, it shows on the frame after that tick and is gone
  at the next. Outside Play no tick passes, so a line drawn in edit mode stays until
  Play starts; Stop clears every line, and so does opening a scene in edit mode. A
  `Scene.Load` in Play leaves them to run out their duration.
- Draw-only: nothing in the sim reads a line back. A non-finite end or colour is
  dropped; past 16384 held lines the oldest go.

```lua
local ox, oy, oz, dx, dy, dz = 0, 1, -5, 0, 0, 1
local hit, id, dist, px, py, pz = Physics.Raycast(ox, oy, oz, dx, dy, dz)
if hit and Debug then -- `Debug` is absent in a ship build
    Debug.DrawRay(ox, oy, oz, px - ox, py - oy, pz - oz, 1, 0, 0, 1, 2) -- red, 2 s
end
```

### The preview — eyes on an authored asset (#353)

`Debug.Snapshot` is the structured read; **`Debug.Preview` is the visual one**. It
renders an asset in the same isolated preview scene the editor's Inspector "Preview"
tab shows a human — one stand-in mesh, a procedural sky, one light — and writes it
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

**Options** (all optional; `mesh` and `repeats` are ignored for models):

| Option | Values | Default | What it does |
|---|---|---|---|
| `mesh` | `"sphere"`, `"cube"`, `"suzanne"`, `"plane"` | `"sphere"` | the stand-in the subject is drawn on |
| `repeats` | a whole number, 1 or more, up to what the renderer can show | left out: the material as authored | how many times the material repeats across the stand-in |
| `resolution` | pixels | `512` | the square output's edge, clamped to `16`–`4096` |

The sphere, cube and Suzanne are a **prop's** view: a three-quarter key light, the
subject in the middle of the frame. The **plane** is a **tiling surface's** view (Unity's
quad preview, tilted): a flat 4 m square seen from about 25° above its surface, filling
the bottom of the frame with its far edge high in it, under a light raking across it at
about 20°. That is how a player sees a floor or a corridor wall, and the angle at which
a normal map's relief (mortar depth, grain, bevels) shows: the faces turned toward the
light catch it, the ones turned away go dark.

Pass `repeats` to see the material tile; leave it out to see its real scale. Left out,
the material is drawn as authored: a UV-mapped one once across the stand-in (times its
own `tiling`), a world-mapped one (`uv_mapping` `Planar` or `Triplanar`) at its own
`world_size`, which shows its true texel density (the stand-ins are about 1 m across,
the plane 4 m). Given, `repeats` multiplies a UV-mapped material's `tiling`, so a stain
or crack that comes back every tile shows as a pattern; under a world mapping the
preview's copy of the material has its `world_size` (and its detail layer's, in
proportion) set so that `repeats × world_size` metres span the stand-in: the same
number of repeats shows either way, and the camera never moves. The library's own asset
is not changed. A count the renderer would clamp raises, naming the count that fits:
`tiling` past ±1000, or a `world_size` outside 0.01–100 m (at most 400 repeats on the
plane, 100 on the others, fewer with a fine detail layer).

```lua no-run
-- a wall or floor material: three repeats on the plane, to judge repetition and relief
Debug.PreviewMaterial("brick", "out/brick_plane.png", { mesh = "plane", repeats = 3 })
```

`repeat` is a Lua keyword, so the option is spelled `repeats`.

The framing is **fixed**, one per mesh — there are deliberately no camera parameters. Orbit/zoom is
the human affordance in the editor; an agent needs the shot reproducible so two
captures of the same asset are comparable pixel-for-pixel. Parent directories of
`out_png` are created for you.

Errors are raised for a caller mistake (a path that doesn't exist, an extension with no
preview story, an unknown `mesh` name, a `repeats` below 1 or past what the renderer can show) so a typo can't quietly return a picture of an
empty sky. The one non-error miss is a machine with no GPU or software adapter, where
the call **returns `nil`** after a warning rather than failing the run.

### The frame stats — performance you can read and assert (#433)

`Debug.Stats()` is the perf read: the numbers an agent needs to prove a rendering or
gameplay change is cheaper, since it never *feels* the frame rate. Each metric is a
`{ last, min, avg, max, samples }` table summarising every frame of the **current Play
session**: the stats restart when Play starts, take Play's frames only, and after Stop
hold the session that ended until the next Play. Before any Play (a fresh editor)
there is nothing to summarise: `frames` and `drawn_frames` are `0`, `systems` is
empty and every metric is absent (`nil`), so check `frames` before reading one:

```lua
local s = Debug.Stats()
if s.frames > 0 then
  print(s.frames, s.fixed_update_ms.avg, s.systems.update_scripts.max, s.entities.last)
end
```

**Two clocks.** `frames` counts **sim ticks** (the fixed 1/60 s step) and
`drawn_frames` counts **drawn frames**: a harness screenshot or `Harness.Render`, or a
frame of the window (the editor's or the player's, in a `dev` build, #2205). They
differ in a window, where one drawn frame runs zero or several ticks. The schedule's
timings and the world counters (`frame_ms`, the stages, `systems`, `entities`, …) are
per tick; the render counters, `renderer_ms`, `gpu_ms`, `gpu_passes`, `wall_ms` and
`fps` are per drawn frame. Each metric's `samples` says how many of its own it holds.

**Now, not the session: `Debug.Stats{ window = n }`.** The same table over the newest
`n` frames only: a sample is in when it was recorded within the last `n` drawn
frames, and a per-tick one within the last `n` ticks too; `frames` and
`drawn_frames` count the ticks and drawn frames inside. A key with no sample inside
is absent, so a pass that stopped running (SSR turned off, `taa` and `ui` on the
Scene tab) or a load long ago leaves the window. Unlike the session, a window reads
**every** frame the window draws, edit mode included, as Unity's Stats overlay does:
in a fresh editor or after Stop it holds `wall_ms`, `fps` and the render keys of the
tab on screen, while `frames` is `0` and the per-tick keys are absent once `n` frames
were drawn without a tick. The newest 600 samples of each key are kept, so a wider
window (`math.huge` too) reads those. `wall_ms` is the one key a frame gets late,
once the next frame starts (its row, below), and before that frame's work: so a read
during a frame (a script, the REPL, `session-mcp --attach`, the overlay) finds it on
all `n` frames, the newest being the one before. Only a read between a frame's draw
and the next frame's start would find the newest without it, and then a window's
`wall_ms` covers `n - 1` frames while every other metric still covers `n`. It reads; it resets nothing, so two readers never disturb each
other. Its one option is `window`: another key raises, naming it. The Game view's **Stats** overlay (`Editor.SetGizmo("Stats", true)`,
[`Editor.md`](Editor.md)) shows `Debug.Stats{ window = 60 }`, its frame time the
mean `wall_ms` over those 60 frames:

```lua
local s = Debug.Stats{ window = 120 }   -- the last two seconds of a 60 Hz window
if s.wall_ms then print(s.drawn_frames, s.fps.avg, s.wall_ms.max) end  -- a window's
if s.gpu_ms then print(s.gpu_ms.avg) end                                -- timestamps
```

| Metric | What it counts |
|---|---|
| `frame_ms` | CPU ms of the whole per-frame schedule (the four stages below) |
| `fixed_update_ms` / `update_ms` / `late_update_ms` / `render_stage_ms` | CPU ms per schedule stage. `render_stage_ms` is the `Render` *stage* (draw-data prep), not the GPU |
| `systems.<name>` | CPU ms per registered system (`update_scripts`, `tick_script_timers`, `step_physics`, `tick_nav`, …) |
| `entities` / `rigid_bodies` / `nav_agents` / `particles` / `scripts` | world counters: live entities, RigidBody components, NavMeshAgents, live particles, loaded script instances |
| `skeletons` / `bones` | skinned meshes with a skeleton, and their bound bone slots summed per skeleton (#1617; a bone two skeletons list counts twice): what a scene's characters cost the bone systems (`animate`, `pose_ragdolls`, `follow_bones`, `build_skin_palettes`) each tick |
| `changes_<channel>` | per change channel the World keeps a log for (`ecs::changes`, #1705), the entities its log took in this frame: with a system reading it every tick, the distinct entities dirtied on it (one already waiting unread is not counted again). Channels: `world_pose`, `activity`, `layer`, `static_set` and the component columns (`transform`, `collider`, `scripts`, …). `changes_static_set` is always present (`0` until something reads it); the others appear once a system opens a cursor on them |
| `load_ms` / `load_<step>_ms` | CPU ms of each play-mode `Scene.Load` or `Scene.LoadAsync` that swapped a scene in (#2123), one sample per load (the sum of its steps: the swap's bookkeeping between them, such as the survivors list and the `Loaded scene` line, is not counted), and of its steps in order: `read` (read and parse the `.scene`), `meshes` (rebuild every mesh from its reference, model imports included), `sidecars` (read the saved navmesh, lighting SH and PVS), then at the swap `teardown` (the outgoing scripts' callbacks, queries and voices), `swap` (the World replaced, the sidecars applied, prefab instances propagated), `physics` (the physics world built), `navmesh` (restored from the saved one, or baked) and `sources` (sound sources started). A `Scene.LoadAsync` prepared its scene off the tick, so its sample holds only the steps at the swap: `read`, `meshes` and `sidecars` are absent from it, and its `navmesh` restores what the preparation baked. Absent until a load. The same breakdown goes to the console as one `Scene load <path>: <ms> (read …, …)` line. The new scene's GPU uploads happen on its first drawn frame (`renderer_ms`, `asset_uploads`; a windowed `LoadAsync` makes most of them before it is ready, `Scene.md`), and its scripts' `Awake`/`Start` on its first tick (`systems.init_scripts`) |
| `draw_calls` | geometry draw calls: solids (mesh particles among them), transparents, shadow casters, the SSAO depth prepass, sprite-particle batches, UI batches (post-FX and skybox excluded; decals draw nothing of their own, #638). Copies of one mesh + material are **one instanced call** (#470), so this tracks distinct looks, not entity count, except a batch of a big mesh drawn by cluster (#1968), which counts one call per instance |
| `triangles` | triangles submitted by the solid, transparent, shadow and SSAO-prepass draws, every instance counted |
| `visible_entities` / `culled_entities` | mesh entities drawn / skipped by the frustum cull, summed over the camera views (the stack and render-texture cameras) |
| `cull_tested` / `size_culled_entities` | mesh entities tested against a camera's frustum (every one the camera draws at all whose LOD level it shows) / kept by the frustum but past their max draw distance or under their min screen size (#1714, [`Mesh`](Mesh.md)), summed over the camera views |
| `occlusion_culled` | static mesh entities inside a camera's frustum and over its small-object limits, but out of sight of its cell by the scene's baked occlusion (#1714; `Lighting.md`, *Occlusion culling*), summed over the camera views. `0` with no bake, for a camera with `use_occlusion_culling` off, an orthographic one or one whose near plane is past 1 m, and while the bake is older than a static occluder. A light's shadow casters are counted apart, in `shadow_occlusion_culled` |
| `hiz_tested` / `hiz_culled` | opaque draw instances (one per mesh entity and material run) tested against a camera's GPU depth pyramid, every one whose bounds are known and that the selection outline doesn't ring / of them, the ones it found hidden and left undrawn (#1963; `Lighting.md`, *GPU occlusion culling*), summed over the camera views. `hiz_culled` is read back from the GPU without waiting, so it describes a frame or so before this one (`0` on the first). Both `0` for a camera with `use_occlusion_culling` off, under a quality preset with `gpu_occlusion_culling` off, during a capture, and on a GPU without indirect first-instance draws. `draw_calls` and `triangles` count the batched instances, culled ones included |
| `cluster_tested` / `cluster_culled` | clusters of big static meshes (split at import, #1968) tested one by one after the Hi-Z instance test: every cluster of each instance it judged, in a batch drawn by cluster / of them, the ones left undrawn (out of the frustum, facing away, behind the depth pyramid, or of an instance found hidden), summed over the camera views (`Lighting.md`, *GPU occlusion culling*). `cluster_culled` is read back like `hiz_culled`, a frame or so late, and is `0` on a frame that draws nothing by cluster. Both `0` where `hiz_tested` is, under a preset with `gpu_cluster_culling` off, for a batch past the per-camera budget, which is drawn whole, and for a batch none of whose instances the instance test judges (mesh particles, a vertex-moving material), which keeps one instanced draw |
| `shadow_cull_tested` / `shadow_frustum_culled` / `shadow_size_culled` / `shadow_occlusion_culled` | shadow casters tested against a light volume (a cascade or an atlas tile), one per entity per volume, counting only the casters the scene's spatial index finds near the volume (#1874), so a far **static** caster is neither tested nor culled (dynamic, skinned and just-moved casters are still tested by every volume) / outside it / inside it but too far or too small from the main camera (#1714) / kept by both but out of sight of the point or spot light's cell by the scene's baked occlusion (#1714; never for a sun, a shadowmasked light, a light outside every cell, or while a baked occluder is older than the bake or casts no shadow), summed over every shadow view the frame drew: the per-frame passes, and the cached static bakes on a frame they re-bake |
| `shadow_gather_us` | CPU **microseconds** the shadow sweeps took to gather, cull and batch their casters (#1714), summed over the sweeps the frame ran (a cached static bake costs nothing on a frame it does not re-bake), inside `renderer_ms` — wall-clock |
| `lod_hidden_entities` | mesh entities skipped because their `LODGroup` showed another level (#472), summed over the camera stack |
| `lights` / `lights_dropped` | active lights, and those left unlit: realtime directional lights past 4, every ambient but the last, and per camera the point/spot lights in view past the clustered budget of 64 (the least contributing go first, #434, #834) |
| `cluster_lights_dropped` | (cluster, light) entries cut because more than 32 point/spot lights reached one cluster, the least contributing there first (#834); non-zero means some corner of the view is over the per-pixel budget |
| `lights_visible` / `lights_culled` | point/spot lights binned into at least one light cluster / listed in none: outside a camera's view and skipped before binning (#434), or kept but contributing nothing (zero intensity or colour, #1712) or with a cone that misses the view, so no pixel shades them; summed over the camera stack |
| `light_cluster_refs` | entries in the cluster light lists (#434): the (cluster, light) pairs shaders may visit, summed over the camera stack — the clustered lighting's workload |
| `light_bin_us` | CPU **microseconds** spent binning lights and decals into clusters (#434, #638), summed over the camera stack — wall-clock |
| `decals_visible` / `decal_cluster_refs` | decals binned into at least one cluster / the (cluster, decal) pairs shaders may visit (#638), summed over the camera stack; a stacked `DepthOnly` camera (a viewmodel) bins none |
| `decal_maps_dropped` | decal maps drawn without their texture this frame because more distinct maps were in use than the decal atlas's 32 layers (#638) |
| `shadow_draws` / `ui_draws` | shadow-caster draw calls (one per caster mesh, instanced, #470; the sun's cascades and the point/spot shadow atlas) / UI batches |
| `shadowed_lights` / `shadow_lights_dropped` | point/spot lights given a shadow in the atlas this frame / ones that cast shadows and are shaded (within the base camera's light budget of 64) but found no room, so they shade unshadowed (the least important go first, #468). A light the budget drops gets no tiles and is counted in `lights_dropped` only, never here (#873) |
| `shadow_atlas_tiles` / `shadow_atlas_texels` | atlas tiles drawn (one per spotlight, six per point light) / the texels they cover, out of 2048² = 4194304 (#468) |
| `shadow_atlas_cached` / `shadow_atlas_rebaked` | atlas tiles whose static casters came from the cache / were re-drawn this frame (#694). A tile re-bakes when its light moves or turns, when it gets another place or size in the atlas (a size change past the hysteresis, or a repack, #1711), or when the editor changes static geometry; a still scene, or a light blinking off and on, reads all cached |
| `ssao_samples` | depth taps the SSAO pass traced (occlusion texels × the samples its quality picks, #436, #1522); `0` when AO is off |
| `ssil_samples` | samples the SSIL gather took (its texels × the samples its quality picks, #1750); `0` when SSIL is off |
| `volumetric_froxels` | froxels the volumetric fog lit (its grid, on the base camera of each view running it, #1703); `0` when it is off |
| `particles_drawn` | particles the renderer drew — sprite instances plus mesh particles, summed over the camera stack (#440). Their draws are in `draw_calls`: one per merged sprite batch, one per instanced run of mesh particles |
| `render_texture_draws` | cameras drawn into render textures this frame (#430) — their geometry is already in `draw_calls` / `triangles`; a camera skipped (unreferenced, or between its `update_every` frames) is not counted |
| `velocity_passes` | opaque scene passes that wrote a velocity buffer (#1435): one per stacked camera of each view whose base camera runs motion blur, depth of field or SSR (SSR's holds only depths, #1526); `0` when none does |
| `primed_passes` | opaque scene passes that started from their SSAO depth prepass (#1499): one per camera that ran SSAO, which then shades each lit pixel once; `0` with AO off, never for a camera with a forward override of its own (the shader preview), and never on Metal: Apple GPUs are tile-based and remove hidden opaque fragments themselves (Unity URP's *Auto* depth priming is off on tile-based targets too) |
| `history_passes` | reasons this frame reads history an earlier frame left (#2092), counted per reason, not per view: one for each view (the screen, each render-texture camera's) whose frame runs TAA, SSIL's accumulation, the volumetric fog's reprojection, an authored effect, or motion vectors on a frame that is not a cut; one for each render texture shown, every frame whatever its `update_every` (its picture is kept from a draw: an earlier one between draws, and last frame's for a camera that sees a monitor); and one while a shadowed light's tile resize is pending (#1711's hysteresis counts frames). So a render-texture camera running TAA whose texture is shown counts twice. `0` means drawing the frame again with no sim step gives the same image, which is how `Harness.Screenshot`'s `settle` knows to draw nothing more |
| `sky_reflection_bakes` | sky reflection cubes baked this frame (#1553): one when the scene's panorama changed, or on a frame showing procedural sky settings (#1620) neither procedural cube holds (the first procedural frame, or a settings change); `0` on every frame the sky holds, including a switch back to the settings the other cube holds (the procedural sky's tint never rebakes it) |
| `ssr_surface_passes` | opaque scene passes that wrote SSR's surface targets (#1526): one per view whose base camera runs SSR; a stacked overlay camera (a viewmodel) draws with the plain pipelines and writes none, so it never counts |
| `opaque_copies` | opaque copies made this frame (#1523), Unity's *Opaque Texture*: one per camera whose visible translucents refract or absorb (`Material.SetRefractionModel`, `SetTransmittanceColor`) or trace SSR (`Material.SetReceiveSsr`, on a base camera running SSR, #1570), drawn after its opaque pass and SSR; `0` on a frame showing none, which skips the copy |
| `texture_bytes` | GPU bytes of the image textures the renderer holds (material maps, UI images, particle and render textures, and the uncompressed copy a slot that cannot read a compressed one gets), every mip included (#1574): the resident total, not a per-frame count. What *Compression* and *Max Size* (`Assets.SetTextureImportSettings`) shrink: a 2048² map is 21.3 MiB uncompressed with mips, 2.7 MiB as BC4, 5.3 MiB as BC5 or BC7 |
| `lightmap_bytes` | GPU bytes of every lightmap page array the renderer holds (#1694): the scene's bound colour and direction arrays with every mip, and another scene's page list still cached beside them (a rebake's old pages are freed as it deletes them). The resident total, not a per-frame count. An irradiance page is BC6H, a byte a texel, plus 5/16 of that for its two mips; a direction page is RGBA8, four bytes a texel (#1996): a directional 2048² page pair is 26.3 MiB. On a GPU without BC compression the irradiance pages upload from their PNGs as `Rgba16Float`, eight times the BC6H size. Pages still loading in a windowed editor or game are not counted until they land |
| `reflection_bytes` | GPU bytes of the reflection probes' cube array (#2085): every layer it is allocated with (the resident cubes and the spare slots the budget keeps), every mip, at the array's face (the largest resident cube's). The array is the only copy of a resident cube, so this is all the probe memory there is, held to `reflectionBudgetMB` except for the nearest cube and the fallback, which are kept past it (`Reflection.md`). A 128-texel BC6H cube is 128 KiB with its mips, a 512 one 2 MiB; on a GPU without BC compression eight times that. The resident total, not a per-frame count; `0` while no cube is resident |
| `grade_lut_bakes` | views whose colour-grading LUT was rebaked this frame (#1440): a grading knob or the tonemap changed, or the view is new; `0` on every frame the grade holds |
| `ui_mask_passes` / `ui_blur_passes` | UI `Mask` coverage textures rendered (#428) / fullscreen passes of the UI backdrop blur, its composite included — `0` with no backdrop visible (#426) |
| `renderer_ms` | CPU ms `Renderer::render` took for the frame: recording it, resolving its GPU timestamps and submitting it. A cubemap capture (a reflection or light-probe bake) and the Inspector's preview are not the frame and are not in it |
| `wall_ms` / `fps` | a window's frames only (#2205): wall-clock ms from a frame's start to the next one's, the frame time a player feels, recorded on the frame that took it (as the frame log's `ms`) as the next frame starts, so it arrives one frame late (*Now, not the session*, above, says what a window sees); a frame the window could not show has no entry, so its time is dropped (the frame log keeps it) and the drawn frame before it keeps its own (outside Play the editor draws on demand, so an idle gap counts whole); `fps` is the window clock's last whole-second reading: the frames drawn since its previous reading, over the seconds between them (at least one), so an editor redrawing on demand reads below 1. `fps` is absent for the window's first second, before the clock has a reading. Both are timings, never in a replay-stable `results.json` |
| `render_extracted` / `render_posed` / `render_extract_us` | the retained render scene's update this frame (#1706): objects re-derived whole (every mesh entity on a first frame, a load, an Undo, a Play restore, a lightmap or light probe change, or a renderer-side one such as a re-written texture; otherwise only the meshes whose mesh, material, activity, layer, static flag, light or shader overrides changed, those naming a library material that was added, removed or edited in what a draw's key reads (render mode, shader, a map path, face, a Cutout's test), those overriding the runtime params of a material whose params were edited, and those sharing a re-uploaded geometry; an edit of a material's colour, scalars or params alone rewrites its record in place and re-derives no other object, unless it flips a term a pass is chosen by, which re-derives every user: a glass's roughness crossing the SSR threshold, `depth_postpass`, `writes_motion`, `blend_mode`) / objects only re-placed because they moved / CPU µs the update took, inside `renderer_ms` (wall-clock). `0` / `0` on a frame that changed nothing |
| `render_upload_bytes` | bytes the frame wrote into the forward pass's per-draw arrays (#1706), every camera summed: per-draw uniforms, joint palettes (and a motion-blurred view's last-frame poses), 16 bytes per drawn instance, proxy volumes, the 192-byte record of each material (per scene and variant) that changed, and the 240-byte record of each object that moved or changed, with the unchanged records between two changed ones fewer than 32 slots apart, rewritten so the two go in one write (a static, unchanged object costs only its instance). A shadow sweep's casters are not counted |
| `asset_uploads` / `asset_upload_bytes` | scene assets the frame uploaded (#2123), and their bytes: mesh buffers (a mesh's vertex and index data, rewritten in place or new), texture files (a material map, a UI image, a particle texture, the sky's panorama: its GPU size, every mip) and lightmap page arrays. What a windowed `Scene.LoadAsync`'s GPU warm-up made before its scene was swapped in is counted on no frame, so the new scene's first frame reads `0` for what it warmed (`Scene.md`, *Loading in the background*) |
| `render_passes` / `compute_passes` / `submits` / `bind_groups_created` / `buffer_writes` | the GPU calls the renderer made for the frame (#2341): render passes and compute passes begun, queue submits, bind groups created and `write_buffer` calls, every camera, shadow view, post effect and the UI included, counted from the start of the render to its counters' close. What happens outside a render is counted on no frame: setup, a load's warm-up, a bake's readback between frames, and the GPU timer's resolve after the counters close. The editor's own egui paint passes are not the renderer's and are not counted. Where the adapter has timestamp queries, a culling camera's Hi-Z stage records each of its parts in a compute pass of its own, to time them (`gpu_passes.occlusion`): 3 or 4 compute passes where an adapter without them records 2 |
| `gpu_ms` | GPU ms the frame's timed passes kept the GPU busy (#835): the sum of `gpu_passes` below. No pass is charged a gap before it, so the GPU waiting for the CPU between submits is never in it, and `gpu_ms`, `gpu_other_ms` and `gpu_between_submits_ms` together are the span from the frame's first timed start to its last timed end. **Only where the adapter has timestamp queries** (Metal, most Vulkan GPUs; not lavapipe) — elsewhere the key is absent, never `0` |
| `gpu_other_ms` | untimed GPU work: GPU ms inside one of the frame's command buffers that no pass holds, where the GPU never waits for the CPU, so a pass left untimed shows here. Present exactly when `gpu_ms` is |
| `gpu_between_submits_ms` | the GPU waiting for the CPU: GPU ms from one submit's last timed work to the next's first, while the CPU records and submits, plus an untimed command at a submit's head or tail (SSR's copy of the opaque image, the Hi-Z counters' copies). Not in `gpu_other_ms`. Present exactly when `gpu_ms` is |
| `gpu_passes.<pass>` | GPU ms per kind of timed GPU work (render or compute passes), in the same `{ last, min, avg, max, samples }` shape, under stable names (below). Absent as a whole without timestamps |

**GPU time per pass (#835).** Each timed render or compute pass writes a GPU
timestamp at its start and end, or a stage of several passes takes one pair across
them (its first pass writes the start, its last the end: SSR's pyramids), or one
pair per part, all added into its key (`occlusion`); the renderer resolves them after the frame and reads them back without
ever stalling it (the live loop gets them a frame or two later, so a window frame
carries the newest times resolved, its own or one just before; the harness waits for
its frames anyway, so each reports its own). The passes are grouped by kind,
each kind's passes summed, under names that never change so two reports diff:

| `gpu_passes` key | What it covers |
|---|---|
| `sky_reflection` | the sky's reflection cube (#1553): the capture and GGX prefilter of the panorama or the procedural sky, on the frames the sky changed (counted in `sky_reflection_bakes`) |
| `shadows` | the sun's cascades (static bake + dynamic casters) and the point/spot shadow atlas |
| `occlusion` | GPU occlusion culling (#1963), each culling camera's whole `occlusion` stage: the select of last frame's visible set, its depth-only draw (phase one), the Hi-Z depth pyramid, the bounds test, the compaction and the per-cluster cull, each part its own pass and timestamp pair, summed here. All of it goes out in one submit with no readback inside, so it is the stage's GPU work, not its wall time; a gap between two parts (an encoder command) is in `gpu_other_ms`. No key holds a part: the bench reports them as `gpu.occlusion.<part>` (`docs/testing.md`, *Benchmark: `make bench`*) |
| `ssao_prepass` | the SSAO depth prepass (#1578): every visible solid's depth into the view's depth target, which a primed forward pass starts from |
| `ssao` | the SSAO occlusion pass, at its quality's resolution |
| `ssao_blur` | the SSAO blur at the occlusion's resolution, and the depth-aware upsample to full resolution when that is lower |
| `ssil` | screen-space indirect lighting (#1750): its gather, blur, accumulate and upsample before the base camera's opaque pass, and the source it keeps after it; the depth prepass it draws on a camera without SSAO is under `ssao_prepass` |
| `volumetric_fog` | volumetric fog (#1703): its froxels' scatter and integrate, before the base camera draws |
| `forward` | the opaque scene pass: solids, decals folded in, skybox — every camera's, render-texture cameras included |
| `ssr` | screen-space reflections (#1526): the trace over the base camera's opaque image, which writes each pixel's hit. The copy of that image before it is a command at the head of SSR's submit, not a pass, so it is in `gpu_between_submits_ms` |
| `ssr_pyramids` | SSR's hit and spread pyramids (#2028, #2158), built from the trace's targets; absent on a view whose shorter side is 8 px or less, which has one level |
| `ssr_resolve` | SSR's resolve (#2028): each pixel's hits averaged over its blur and drawn over the image |
| `transparent` | translucent solids |
| `ribbons` | trails and lines |
| `particles` | particle systems |
| `depth_of_field` | depth of field (#1445): its circle-of-confusion, prefilter, blur and composite passes |
| `taa` | temporal anti-aliasing (#1702): its resolve, one fullscreen pass over the scene, its history and the velocity target (on a view upscaling through it, a render scale below 1, the same pass reconstructing the output-size image from the scaled scene), its sharpen (#1847) and the clear of its reactive mask |
| `motion_blur` | per-object motion blur (#1436): its TileMax and NeighborMax tile passes and the gather |
| `lens_flare` | the screen-space lens flare (#1443): its flare pass (the composite adds it, under `post_fx`) |
| `post_fx` | bloom, the composite, authored effects, FXAA, and the automatic-exposure meter (#1444) |
| `outline` | the editor's selection outline (its mask, dilation and ring) and the gizmos drawn over it after post-FX: grid, the occlusion view's boxes, axis arrows, probes (editor only) |
| `ui` | world and screen UI, Mask textures and the backdrop blur |

A kind appears once a frame ran it (`samples` counts those frames). GPUs overlap
neighbouring passes — a tiled GPU starts the next pass's vertices while the last
one's fragments still run — so each pass is charged its **exclusive** time: from
where the work queued before it ended (or its own start, if later) to its own end.
The kinds therefore add up to `gpu_ms`, and a heavy pass shows as heavy instead of
smearing into the light ones behind it. Like every `_ms` key, these are wall-clock
and differ run to run, and they are this GPU's: compare them on one machine.

Keys ending in `_ms` or `_us`, and `fps`, are **wall-clock** and differ run to run;
everything else is a count and is deterministic for a deterministic run. The timings are measured *around*
the sim by the dev layer and never fed back into it, so reading them cannot change a
replay. Render counters (and GPU times) appear on drawn frames: a harness `Harness.Screenshot`,
`Harness.Render` (which draws without writing a PNG), or every frame a `dev` build's
window draws, editor or player (#2205). In the editor that is the tab on screen, Scene
or Game; the Inspector's asset preview is not counted. A headless run that never
renders reports timings and world counters only. Every harness frame is a session
frame; a window's frame joins the session only in Play (above).

Scenarios get the same table as `Harness.Stats()` and can turn it into pass/fail with
`Harness.AssertBudget{ draw_calls = 2000, fixed_update_ms = 4 }` — see *Performance
budgets* in `docs/testing.md`.

### The frame log — looking back at a minute ago (#2206)

`Debug.Stats` answers *now*; the frame log answers *then*. While a `dev` build's
window runs — the **editor, on by default**, a `player` dev build only when the
project says so — every drawn frame writes one JSON line under the project's
`saved/logs/`, beside what the console said, so "a minute ago, by the bar" can be
found after the fact with nothing armed beforehand (Unity's `Editor.log` plus the
Profiler's ring of frames, as files an agent reads). It works during Play, after
Stop, and from `session-mcp --attach`; a plain headless session reads what the
editor last wrote. A headless harness run writes none. Every window frame is
written, a frame the window could not show (a locked screen, an occluded window)
included, since the sim and a bake still run then; `Debug.Stats` keeps no entry
for such a frame, so its `ms` is in the log alone.

```lua
local log = Debug.FrameLog{ last_seconds = 60 }      -- the whole last minute, every channel
local slow = Debug.FrameLog{ over_ms = 33 }          -- frames past 33 ms, newest 600
print(slow.truncated)                                -- at least how many older ones the 600 left out
for _, f in ipairs(slow.records) do
  print(f.tick, f.ms, f.heaviest_pass, f.heaviest_system, f.cam[1], f.cam[3])
end
Debug.FrameLog{ channel = "console", last_seconds = 10 }   -- what was logged
Debug.SetLogChannels{ audio = false }                -- kept in project.rusty, when a project is open
```

**Records.** Each line has `ch` (its channel), `t` (wall-clock ms since the Unix
epoch) and `tick` (the Play tick, `0` in edit mode), written first. A `frames`
record also has `frame`, the renderer's count of drawn frames (this frame's index
when it drew, the last drawn one's when it drew nothing; it restarts with the
window), places itself — `play`, `scene` (the open scene's path), `cam` and `fwd`
(the game camera's position and facing) — and says what the frame cost: `ms` (its
wall time), `fps`, `ticks` (sim ticks it ran) and `sim_ms` (their CPU ms), and,
when it drew, `cpu_ms` (the renderer's), `gpu_ms`, `draws`, `tris`, `visible` and
`culled`.

**A line is whole: its own times.** `ms` is the frame's own wall time, from its
start to the next frame's, so it holds its sim, its draw and any wait before the
next; `gpu_ms` and `gpu_passes` are its own GPU times. Both arrive after the frame
(its wall time when the next starts, its GPU times a frame or two after it drew),
so the writer holds a frame's line, and the console, load and audio lines logged
with it, until they are in, or for at most 4 later frames; whatever is missing
then is named in `unresolved` (`"ms"`, `"gpu"`) instead of borrowing a
neighbour's. An adapter without timestamps writes no GPU field and waits on none.
The frames still held are the newest of the log, and `Debug.FrameLog` returns
them from memory as they stand (the newest saying `unresolved = { "ms", "gpu" }`);
when the window closes they are written as they stand. The files read the same
with `jq` or `grep` as through `Debug.FrameLog`.

| Channel | What it writes |
|---|---|
| `frames` | one record per drawn frame (above) |
| `gpu_passes` | `gpu_passes = { <pass> = ms }` on a frame record, where the adapter has timestamps |
| `systems` | `systems = { <name> = ms }` on a frame record: the systems whose CPU time over the frame's ticks reached 0.25 ms |
| `console` | a record per console line: `level` (`info`, `warn`, `error`) and `msg`, every line as it came, past the console's 100-line buffer. A warning repeated within a frame is one line with `times` (how many); one repeated every frame is one line per frame, so it is found in any look-back |
| `loads` | a record per finished Play-mode scene load (`Scene.Load`, `Scene.LoadAsync`): `load_ms` and `steps = { <step> = ms }`. An edit-mode load (File → Open) is not timed step by step: its frame's `ms` shows it, beside its `Scene loaded from …` console line |
| `audio` | a record per audio event, the fields `Debug.AudioEvents` returns, its own `tick` (the sim tick it played on) as `event_tick` |

**The read.** `last_seconds` keeps the records from the last that many seconds of
the wall clock; `over_ms` keeps the frame records slower than that and names each
one's `heaviest_pass` and `heaviest_system`; `channel` keeps one channel (`gpu_passes`
and `systems` keep the frames that carry them; `dropped`, the writer's drop notes);
`limit` keeps the newest that many (`math.huge` for all; without a `limit`, a
`last_seconds` read returns every record in its span, and a read without either
the newest 600). A read whose answer is full stops at the end of the segment that
filled it, so `truncated` is *at least* how many matching records it left out (those
in that segment). `dropped` sums the records dropped in the span of the records
returned (from the oldest one's time, or the look-back's start). `path` is the
folder, to open with `jq` or `grep`. A line that does not parse or is not UTF-8 —
the last one, cut by a power cut or still being written — is skipped and counted
in `skipped` (one older than the look-back is not). The read runs on the calling
thread (the editor's, from a session) and parses only the lines it keeps. On a
full 50 MB log (about 150 000 frames) from the page cache, in a dev build, the
default read (the newest 600) took about 40 ms, a filter nothing matches (every
segment read, no line parsed) about 0.3 s, and the worst case, a look-back that
covers the whole log and returns every line, about 2 s before its table reaches
Lua: ask for a span, or a `limit`.

**Bounded.** The log is numbered segments, `frames-<n>.jsonl`, each a tenth of the
budget (`project.rusty`, `"logs"`: `budget_mb`, default 50). Past the budget the
oldest segment is deleted; `minutes` (optional) also deletes a segment whose newest
record is older (`0`, like leaving it out, bounds the log by size alone). A restart numbers on, keeping the earlier session's segments under
the same budget. `editor` and `player` say which hosts write it; `budget_mb = 0`
writes none. The budget is read when the window opens; a channel switch through
`Debug.SetLogChannels` or the checkbox takes effect the next frame (and is kept in
`project.rusty` when a project is open; a host with none keeps it for the session), a hand edit of
`project.rusty` at the next reread of the file. One process writes a project's log:
a second (another editor, or a dev `player` with its log on) finds the folder's
`.lock` held, warns once and writes nothing. The Build Settings window (File → Build Settings) has a checkbox per
channel. A frame never waits on the disk: it hands its lines to a writer thread, and
when that falls behind the lines are dropped and a `{ ch = "dropped", tick, records = n }`
line says how many (`tick` is the newest dropped frame's); a frame that logs more
than 1024 console lines drops the rest the same way.

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
  "components": ["Collider", "Material", "Mesh"],   // optional-component inventory
  "transform": { "pos": [x,y,z], "rot": [x,y,z], "scale": [x,y,z] },  // rot = Euler°
  "bounds": { "min": [x,y,z], "max": [x,y,z] },     // world-space AABB, or null
  "scripts": ["assets/scripts/foo.lua"],
  "mesh":      { "primitive_type": "Box", "asset_ref": "models/crates.glb::Barrel" },
                                // + "subdivisions" when not 1, "cast_shadows" when not "On",
                                // "max_draw_distance" / "min_screen_size" when set
                                //   "scale_in_lightmap" when not 1
  "material":  { "color": [r,g,b], "metallic": .., "roughness": .., "texture": "..",
                 "metallic_map": null, "roughness_map": null },
  "light":     { "type": "Point", "color": [r,g,b], "intensity": .., "range": ..,
                 "inner_cone": .., "outer_cone": .. },
  "collider":  { "active": true, "is_trigger": false,
                 "shape": { "kind": "Box", "size": [x,y,z] },   // or Capsule: radius, height, axis
                 "material": { "friction": 0.5, "bounciness": 0.0,   // what the solver uses:
                               "friction_combine": "Average", "bounce_combine": "Average" },
                 "material_asset": null },  // or "assets/materials/wood.physmat" (#1878),
                                            // whose values `material` then shows
  "rigidbody": { "active": true, "is_kinematic": false, "mass": .., "velocity": [x,y,z],
                 "angular_velocity": [x,y,z], "use_gravity": true,
                 "collision_detection": "Discrete", "drag": 0.0, "angular_drag": 0.05 },
  "camera":    { "active": true, "fov": .., "near": .., "far": .., "culling_mask": ..,
                 "render_order": 0, "projection": "Perspective", "fov_axis": "Vertical",
                 "target_texture": null },                      // or "rt:<name>" (#430)
  "nav_agent": { "active": true, "radius": .., "target": [x,y,z], "speed": .., .. },
  "ai_perception": { "sight_range": 20.0, "lose_sight_range": 25.0, "sight_angle": 60.0,
                     "eye_height": 1.6, "sight_mask": .., "blocking_mask": 1,
                     "hearing_range": 30.0, "sight_max_age": .., "hearing_max_age": ..,
                     "damage_max_age": .., "known": [2], "in_sight": [2] },  // #1900, runtime
  "particles": { "active": true, "texture": null, "rate": .., "lifetime": .., .. },
  "animator":  { "clip": "Idle", "time": .., "speed": .., "playing": true,
                 "loop": false, "paused": false,
                 "parameters": { "Jump": { "Trigger": false }, "speed": { "Float": 4.2 } },
                 "graph": "guard.animgraph", "graph_enabled": true, "node": "Locomotion",
                 "layers": [ { "name": "UpperBody", "weight": 1.0, "node": "Aim" } ],
                 "apply_root_motion": false, "root_bone": "",         // #1745
                 "root_motion": { "position": [x,y,z], "yaw": .. },   // last step, degrees
                 "montages": [ { "slot": "UpperBody", "montage": "Reload",  // #1894, runtime
                                 "section": "Default", "time": .., "weight": ..,
                                 "blending_out": false } ] },
  "audio":     { "clip": "music/theme.ogg", "volume": .., "loop": false,
                 "play_on_start": false, "is_time_scaled": true,
                 "spatial_blend": 0.0, "initial_distance": .., "final_distance": ..,
                 "output_group": "", "occlusion_enabled": true, "doppler_level": 1.0 },
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

Every first-class component has its key, keyed by its saved name (the ones not
shown above too: `post_processing`, `joint`, `trail`, …, each as the component's own
fields); it holds a value only when the entity carries that component (absent ones
serialize as `null`), and `components` lists the carried ones in the registry's
order, then `Script`; `bounds` comes from the mesh when geometry is
present, else the collider, else `null`. `ui_rect` is the element's computed UI
layout (see `UI.GetRect`) — `null` unless it is a canvas or laid out under one —
so the agent can reason about the UI without a screenshot. The shape is shared with the harness's
`Harness.Snapshot` (the play-testing path), so the read is identical wherever it's
taken.
