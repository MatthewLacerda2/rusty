## `Lighting`

**`Lighting.Generate()`** (#832) is Unity 5's *Generate Lighting* button: every bake a
scene has, in order, with the scene's own settings, and the result **written** — no
separate save. It bakes the lightmaps (`Lighting.BakeLightmaps`, below), then the light
and reflection probes (`Lighting.Bake`), so walls and actors are always lit by the same
bake, then the occlusion culling's PVS (*Occlusion culling*, below). The editor's **"Generate Lighting"** button (the scene inspector) is this exact
path. The call returns when the bake is done (Unity's `Lightmapping.Bake`); the button
bakes the same probes a step a frame — one reflection probe or a few light probes —
so the editor and the MCP keep answering (Unity's `BakeAsync`, #1602). Both say each
bounce pass and reflection probe in the console (`Baking reflection probe 3 of 12`).
The two individual verbs stay for a script that wants only one of them. On a
`Shadowmask` scene (#1695) the lightmap bake records each masked light's channel and
only a probe bake writes the probes' occlusion on it, reading those channels from the
record and never assigning its own: so `Lighting.BakeLightmaps` warns once in the
console that the probes' occlusion is stale until `Probe.Bake()` or Generate
Lighting, and `Probe.Bake()` warns once when the record holds no channels (the
probes then carry no light's occlusion). Generate Lighting runs both, lightmaps
first.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.Generate` *(dev-only)* | `()` | `{ lightmaps, lightProbes, reflectionProbes, occlusionCells, occlusionObjects, occlusionBytes, occlusionNeverCulled, saved }` (the occlusion fields `0` when nothing occludes, or when the level is too large to cull) |
| `Lighting.BakeOcclusion` *(dev-only)* | `()` | `{ cells, objects, occluders, neverCulled, bytes, tooLarge }` — only Generate Lighting's occlusion step (*Occlusion culling*, below), written beside the scene; the counts `0` when nothing occludes, and `tooLarge` (a boolean) `true`, with `cells` and `bytes` `0` and no PVS kept, when the level is too large to cull |
| `Lighting.GetSettings` *(dev-only)* | `()` | `{ texelsPerUnit, samples, bounces, seed, directional, maxLightmapSize, denoiser, backfaceTolerance, samplesPerPass, prioritizeView, diffuseModel, mixedLightingMode, reflectionResolution, reflectionBudgetMB, reflectionMaxProbes }` |
| `Lighting.SetSettings` *(dev-only)* | `({ texelsPerUnit?, samples?, bounces?, seed?, directional?, maxLightmapSize?, denoiser?, backfaceTolerance?, samplesPerPass?, prioritizeView?, diffuseModel?, mixedLightingMode?, reflectionResolution?, reflectionBudgetMB?, reflectionMaxProbes? })` | — |
| `Lighting.Bake` *(dev-only)* | `([probeSpacing], [reflectionRegion])` | `true` if at least one GPU bake ran, `false` if both skipped (no adapter) |
| `Lighting.BakeLightmaps` *(dev-only)* | `([texelsPerUnit], [samples], [bounces], [seed], [directional])` | number of lightmaps written |
| `Lighting.ClearLightmaps` *(dev-only)* | `()` | — |
| `Lighting.GetBakeReport` *(dev-only)* | `(kind)` | the last report the `kind` bake (`"lightmaps"`, `"probes"`, `"reflections"` or `"occlusion"`) wrote beside the open scene, as a table (*Bake reports*, below); `nil` when the scene is unsaved or that bake never wrote one; an unknown kind errors |

### Generate Lighting

- **Settings live with the scene** (Unity's Lighting Settings): texels per unit, samples,
  bounces, seed, Max Lightmap Size, the denoiser, Backface Tolerance, the directional mode and
  the progressive bake's samples per pass and Prioritize View are saved in the scene file's
  `lighting_settings` block, so a rebake never asks for them again.
  `Lighting.SetSettings{ samples = 256 }` changes only the fields it names (each clamped
  to the editor's range: `0.25`–`64` texels per unit, `1`–`4096` samples, `1`–`16`
  bounces, `0`–`1` Backface Tolerance, Max Lightmap Size below; a NaN or infinite density takes
  the default `8` (a NaN or infinite tolerance its default `0.9`), and
  a file's out-of-range value is held the same way when the scene loads); the editor's card edits the
  same fields under the button. The reflection probes' defaults sit beside them:
  `reflectionResolution` (default `512`, a power of two in 16–2048) and
  `reflectionBudgetMB` (default `256`, `8`–`2048`) and `reflectionMaxProbes` (default
  `512`, `1`–`4096`, the most probes auto-placement makes), edited in the scene Inspector's
  *Reflection Probes* section (see `Reflection.md`). A scene whose settings are all
  at their defaults leaves the block out of its file. `Lighting.BakeLightmaps` takes the
  same settings for every argument it is not given.
- **The denoiser** (`denoiser`, #1884, Unity's Lightmapping *Denoiser* and *Filter*) cleans the baked
  bounce: `"OIDN"` (the default), `"Filter"` or `"None"`, read in any case; an unknown
  name is an error and changes nothing. The editor's card picks it under the bake knobs.
  See *Denoising* under the lightmaps below for what each does and what OIDN needs.
- **The diffuse model** (`diffuseModel`, #1698) is how a lamp's direct light falls off
  across a surface, scene-wide: `"Lambert"` (the default, the physical `max(N·L, 0)`)
  or `"HalfLambert"` (the retro squared wrap `((N·L + 1) / 2)²`: a quarter of the light
  at the terminator, none only straight behind, and about a third more diffuse
  overall). The name is read in any case; an unknown one is an error, and a
  `SetSettings` call that raises changes nothing. The editor's card edits it under the
  bake knobs, greyed out while a bake runs. It is left out of the file at `Lambert`.
  - **Where it applies:** wherever a lamp lights a surface. Realtime shading, lightmap
    texels, the light probes' baked lamps (and `Probe.BakeAnalytic`'s sun), the surfaces
    a bake's bounce rays hit and those a probe capture renders. So global illumination
    gets brighter under `HalfLambert` too; only the transfer from one surface to the
    next, and the sky, stay Lambert. Live lights follow a change at once, baked light at
    the next bake.
  - **Baked vs Realtime:** one lamp lights the side of a surface facing it the same
    either way. Past the terminator, under `HalfLambert`, they agree only when the live
    lamp casts shadows: a bake always traces a shadow ray, and from a point facing away
    from the lamp that ray hits the surface's own mesh, as a shadow-casting live lamp's
    shadow map does. A live point or spot lamp without shadows (`Light.SetCastShadows`,
    off by default; the sun always casts them) keeps the wrap's back-side light (fading
    from a quarter at the terminator to none straight behind); a baked or shadowed
    one cuts it to dark at N·L = 0, a hard step where the wrap still gives a
    quarter. A dynamic object lit through the probes beside it keeps the back-side
    light: a probe stores the lamp unshadowed at its own position. This step is a known
    limit of the opt-in retro mode; under `Lambert` there is nothing past the
    terminator to lose, and every path agrees.
- **The Mixed Lighting Mode** (`mixedLightingMode`, #1695) is Unity's scene-level
  *Lighting Mode*: what a `Mixed` light bakes. `"BakedIndirect"` (the default, left
  out of the file) bakes its bounce only; `"Shadowmask"` also bakes its static
  occlusion, into a shadowmask page beside each lightmap page
  (`<scene>.lightmaps/lightmap_mask_<page>_<tag>.png`, RGBA8, one channel per masked
  light) and into the light probes, so its static shadows hold past the shadow
  distance (see [`Light`](Light.md), *Shadowmask*). The scene file's `lightmaps` block
  records each masked light's channel (`channels`) and the lights that fell back to
  `Baked` (`fell_back`). The name is read in any case and an unknown one is an
  error; the editor's card edits it beside the diffuse model. A change takes effect
  at the next bake. Whether static casters also draw a masked light's realtime shadow
  is the quality preset's `shadowmask_mode` ([`Graphics`](Graphics.md#quality-presets)).
- **The result is written at once**, as Unity writes its lighting data asset: the
  lightmap pages, the probes' baked SH (`<scene>.lighting.json`), and the scene file's
  lighting fields (probe layout, reflection probes, lightmap set, settings). Only those
  fields of the file are rewritten: unsaved edits elsewhere in the scene **stay unsaved**,
  in the editor and on disk. A scene file loading would rewrite (a hand-edited one,
  say) is left untouched instead: `saved` is `false`, the editor marks the scene dirty,
  and saving the scene keeps the lighting. A lightmapped mesh that exists only in the
  unsaved edits keeps its lightmap once the scene is saved; until then the file names it
  but has no such entity, and the entry is ignored.
- **Refused during Play**, as `Scene.Save` is (#1340): it writes the scene file, and the
  World then holds the play state. The editor's button is greyed out while playing.
- **Needs a saved scene** (the bakes write into folders named after its file,
  `<scene>.lightmaps/` and `<scene>.reflections/`); errors otherwise. The
  probe step needs a GPU/software adapter and skips gracefully without one, as
  `Lighting.Bake` does.
- **In the editor** the lightmap step runs in the background (#808) with a progress
  bar and **Cancel**, and **progressively** (#1970, *Progressive bake*, below): the
  whole scene sharpens in place pass by pass. A cancelled run writes nothing and keeps
  the old lighting. When
  the lightmaps land they are applied (the probes are captured under them), then the
  probe step runs a step a frame and the console logs one summary line at the end.
  The probe step's placement is planned whole in the frame it starts: about 0.75 s
  in a release build on a Dust2-sized level (10 × 10 rooms of 20 m and a 40 m tower:
  0.41 s for the occupancy grid and floor plan, 0.05 s for the reflection probes,
  0.29 s for 7,638 light probes), so the editor stalls that long once.
- **From a script it logs its progress** (#1667), as Unity's batch-mode bake does: the
  verb blocks its caller until it returns, so a `play` scenario or a `session-mcp` call
  cannot poll, and instead the bake prints a line to **stderr** at each step and every
  5 s within one. `Lighting.Generate()` first names its steps (#1897):
  `[Lighting] steps: lightmaps → lightmap pages → light probes → reflection probes → occlusion`.
  Each line then starts with the time since the bake began (#1891), gives an ETA from
  the step's rate over its last 2 minutes (#1890: a texel's cost varies with the lights
  around it, so a rate since the step's start would drift) and, under `Generate()`,
  the steps still to come:
  `[Lighting] 26:02 lightmaps 41% (612/1501 meshes, 3.1M/7.6M texels), ~38 min left (then: lightmap pages, light probes, reflection probes, occlusion)`,
  then a line per lightmap page encoded (`lightmap pages 50% (6/12 pages encoded), ~30 s left`),
  `light probes, bounce 2 of at most 5: …, ~1 min left in this bounce` and
  `reflection probes 19% (3/16 probes), ~4 min left`. At the end (#1891) it prints what
  each step took (`[Lighting] lightmaps took 1 h 4 min (7.6M texels)`, one line per
  step and per light-probe bounce, plus the work between them: `writing lightmaps`,
  `probe placement` and `saving`; every second is in one line, so they add up to the
  total), the 15 lightmap
  meshes whose texels cost the most worker time, summed over every thread
  (`[Lighting]   Hall_Ceiling: 210.4k texels, 312.5 s, 673 texels/s`), and
  `[Lighting] done in 1 h 9 min`.
  The lightmap step's first line and the total also go to the console (so `play`'s
  `console.log` and the MCP reply keep them), beside the probe step's own `Baking …`
  lines; the lines in between, the step plan and the summary go to stderr only. A probe
  step with no probes of its kind prints nothing. `Lighting.Bake()`,
  `Lighting.BakeLightmaps()`, `Probe.Bake()` and `Reflection.Bake()` log the same way,
  without the step plan. A log never changes what is baked: the lightmaps are
  byte-identical with or without one.
- **The probes bake from a snapshot and apply at the end** (Unity's shape, #1602): the
  light and reflection probes are copied when the probe step starts, and their SH and
  cubemaps are applied in one step when it finishes. Adding, removing or moving a probe
  (or changing a reflection probe's resolution, clip planes or shadows) while it runs
  drops the bake with a console line: the probes keep their old lighting and the
  already-applied lightmaps are kept, with the scene marked unsaved; a probe step that
  fails for another reason (a cubemap that cannot be written) marks it unsaved too.
  Saving this one elsewhere drops it the same way; opening another scene drops it with
  nothing applied to the new one.
- **One bake at a time:** `Lighting.Generate()`, `Lighting.BakeOcclusion()`,
  `Lighting.Bake()`, `Lighting.BakeLightmaps()`, `Probe.Bake()` and
  `Reflection.Bake()` error while the editor's Generate Lighting or Bake Occlusion
  is running.

### Light and reflection probes

`Lighting.Bake()` (#246) auto-places light probes and reflection probes from the static
scene, then runs **both** probe bakes (`Probe.Bake` + `Reflection.Bake`) in one call: the
probe half of Generate Lighting, and the orchestration over the `Probe` and `Reflection`
namespaces, which stay available for manual placement. The placement is planned into
a copy: the scene keeps its old probes and their lighting until both bakes finish, and
then the new sets replace them in one step (as Unity's bake applies only when it
completes). A bake that is refused (no saved scene to write cubemaps beside), dropped
(the probes edited midway), fails (a cubemap that cannot be written) or has no GPU
adapter changes nothing.

**Auto-placement** (deterministic — a pure function of the static scene, no RNG/clock,
so the same level always yields the same layout). Both sets are planned from the same
two pictures of the level (#1737):

- the **floor plan**: where actors stand. With a baked **nav surface** it holds only
  **reachable** floor: the **main floor**, always, which is the largest area joined
  by walking that is not an **elevated top** (an area lying over at least half of
  another area at least a quarter its size: a roof over its room, so an indoor level
  whose roof is larger than its room still probes the room, while a small sealed
  nook under the floor does not lift it); plus every area that holds a `NavMeshAgent`
  or `CharacterController` (a sentry on a roof adds the roof, never removes the
  floor); and what off-mesh links (drops, jumps) lead to from those. So wall tops,
  roof tops and crate tops no one reaches get no probe. The rule still misreads a
  main floor over a basement at least a quarter its size as a top; an actor start
  on that floor adds it back.
  Without a nav surface the plan is rasterized from the static colliders' bounds
  (each cell's lowest top) and has no connectivity to read, so every floor in it counts.
  Either way, **a liquid is not floor**: under a static, flat, `Transparent` mesh with
  no solid collider (a pool's water plane), the first floor below its surface is
  dropped when it is at least 0.3 m down, so no probe captures from at or under the
  water. A floor further down (the storey under a rooftop pool) stays, and so does the
  floor under a puddle or grime quad laid just over it. A collider-less glass pane laid flat reads as a liquid
  too; give a skylight a collider.
- an **occupancy grid**: the static solids voxelized at **0.5 m**, one bit a cell: the
  active solid colliders (their triangles; convex shapes filled), and the opaque
  triangles of every static mesh whose entity has no solid collider (their surface;
  so a ceiling built as a render mesh roofs its room). Transparent runs are left out,
  so water and glass are not solid. About 1.6 MB for a Dust2-sized level
  (200 × 200 × 40 m), built in about 2 s in a debug build (3.4 s when every wall is
  also a collider-less mesh); a level past 256 M cells is gridded coarser.

- **Light probes** — after Unity's Adaptive Probe Volumes: a lattice through the air
  over every reachable floor (each storey), from 1 m up to 3 m of headroom, never past
  a ceiling, through a solid between the floor and the probe, or behind a wall from the
  floor cell it was placed over. Its coarse points, at `probeSpacing` (default `4.0`,
  held to at least `1.0`), are always kept; the points between, at half that, only
  near geometry (a horizontal ray meets a wall or a crate within half
  the spacing). A probe closer than 0.3 m to geometry is pushed out of it, and one still
  inside a solid is dropped. The layout has no regular grid, so objects blend the
  four nearest probes (`Probe.md`), found through a spatial index: about 1.5 µs a
  sample in a release build over a Dust2-sized set of 7,680 probes, against 12 µs for
  a scan of all of them. A moving Light Probe Proxy Volume samples each of its
  points, so a 32³ one costs about 50 ms on every frame it moves. With no static solid at all, a regular grid over
  the nav surface's walkable extent is placed instead.
- **Reflection probes** — **one per room** (#1584). The floor plan
  is cut into rooms where it narrows
  below **2.5 m** (a doorway, an arch, a corridor mouth) or steps by more than 0.5 m.
  Each room's probe captures from **eye height (1.6 m) over the room's middle**,
  standing on open floor at least 0.5 m clear of geometry where it can, with its box
  **fitted to the room's walls, floor and ceiling** (lowered to the first solid
  straight above the capture point). A room where most of its probes' upward rays
  (straight up and a ring at 30°) reach the sky is **open air**: it gets **one probe
  without box projection**, not a box under an invented ceiling, at importance `0`:
  its box spans the room's whole footprint, so the room probes inside it (importance
  `1`) draw over it. It captures from the clear point nearest the room's middle that
  is itself open to the sky, so a yard round a house never captures from the house's
  doorway.
  A room wider than its region is split into probes no wider than it: the region is
  `reflectionRegion` (default `12.0`, at least `2.0`: a smaller one is
  an error) for an empty room, narrowed by its **clutter** (the solid fraction of the
  air between 0.4 m and 1.8 m over its floor, column by column from each one's own
  floor, the blocked patches of up to 16 m² it encloses counted: a room a quarter
  full of crates takes probes a quarter as wide, while the solid core a corridor wraps
  is architecture, not clutter). Open floor too narrow to hold a room
  (a level of corridors narrower than a doorway, a lone ledge) is an area of its own and
  gets its probes the same way, unless it is under 9 m² (the top of a jump box or a
  crate): that joins the room around it. `reflectionMaxProbes` is a ceiling the manual
  probes count against: a layout over it is re-tiled wider, with a console warning,
  until it fits or every room is one probe, and past that the rooms with the largest
  boxes keep theirs. With the manual probes already at it, none is placed. Without a nav
  surface the floor plan is rasterized at 0.5 m, or coarser (with a warning) when the
  level would take more than 4 million cells. Triggers are not walls.
  A floor plan finds rooms on one floor per cell, so a room stacked under another (a
  tunnel under a catwalk) shares the lower floor's probes.

**Manual vs. placed** (#1737): every probe is either **manual** (`Probe.Add`,
`Probe.FillGrid`, `Reflection.Add`, the Inspector's *Add probe*, or marked with
`Probe.SetManual` / `Reflection.Set(i, { manual = true })` / the Inspector's *Manual*
box) or **placed** by this bake. A hand edit makes a probe manual: `Probe.Move`,
`Reflection.Move` / `SetBox` / `Set`, an Inspector field (except `Set{ manual = false }`
and unticking *Manual*, which say otherwise), so a tuned box survives the next bake.
Each bake **replaces the placed probes and keeps every manual one as it is** (position,
box, settings), and plans around them: a placed reflection probe whose box a manual
probe's box covers by at least half is left out (that room is the manual probe's), and
no placed light probe lands within half a step of a manual one. A manual `FillGrid`
keeps its trilinear grid through any number of bakes. So a level can mix the two and still be regenerated after its static
geometry moves. A saved probe without the flag loads as placed. `Probe.Clear()` /
`Reflection.Clear()` still remove both kinds.

Like the individual bakes, it is **dev-only** and needs a GPU/software adapter; with none
the placement still runs but the bakes skip gracefully (returns `false`, never errors —
except reflections need a saved scene path: their cubemaps go in its own
`<scene file>.reflections/` folder, and a reflection bake that runs removes that folder's
cubes past the probe count, all of them when the scene has no reflection probes). Save the scene
afterward to persist the baked SH (`<scene>.lighting.json`) and the cubemap paths, or use
`Lighting.Generate()`, which writes them.

### Lightmaps

`Lighting.BakeLightmaps()` (#438) bakes per-texel lighting for **static geometry**: bounce
light, the sky, emissive surfaces, and the direct light of `Baked` lights (see the light
**Mode** in [`Light`](Light.md)). It is the lightmap half of Generate Lighting; the
editor's button runs the same bake as its first step, in the background (#808) and
progressively (#1970): the window stays live, a progress bar counts texels done (over
every pass) and then the pages it encodes ("Encoding page k of n"), and **Cancel** stops
the bake, during the trace or the encode, and keeps the previous lightmaps. The result
is applied when the bake finishes, byte-identical to the script verb's; a bake whose
scene was swapped out meanwhile is dropped. The script verb itself stays synchronous
and one-shot, as does `Lighting.Generate()`. An argument left out takes the scene's
lighting settings (above), whose defaults are the ones listed here.

- **Progressive bake** (#1970, Unity's Progressive Lightmapper; the editor's Generate
  Lighting). The trace runs in passes over **every texel of the scene**, not mesh by
  mesh: each pass adds `samplesPerPass` paths (default `8`, `1`–`4096`; eight passes at
  the default 64 samples) to every texel, up to `samples`. After each pass but the last
  the live scene shows the lightmaps as they stand: the paths so far averaged, buried
  texels filled, the bounce cleaned by the denoiser, packed into pages. A writer
  thread beside the bake stores them as `lightmap_preview_<bake>_*` pages in the scene's
  lightmap folder (uncompressed PNGs of one level, no BC encode, while the trace goes
  on; a page a pass left unchanged is not written again); a pass still unshown when
  the next is in is skipped and its pages removed,
  and while the writer is busy the bake builds no preview at all, so a pass that
  would only be dropped costs no denoise. The card and the log say
  the passes done, the samples so far and the time left at the pace since the first
  pass (`[LightmapBake] lightmaps pass 3/8 (24/64 samples), ~3 min 5 s left`).
  **The finished bake's bytes are the one-shot bake's** at the same settings and seed:
  each texel keeps its own seeded stream and its running sums between passes, so the
  pass size never shows. Only a finished bake is written: a save during the bake
  writes the scene's previous lightmaps, never a preview, and **Cancel** puts them
  back and removes the preview pages. Edits during the bake reach those hidden
  lightmaps: a deleted entity leaves them and the preview, and
  `Lighting.ClearLightmaps()` clears them and the preview. The finished bake still
  keeps the deleted entity's new entry, so an Undo of the delete brings it back lit by
  this bake; a save writes no entry for an entity the scene no longer holds. Each shown pass
  costs a display denoise and a preview write, so a smaller `samplesPerPass`
  refreshes more often and bakes a little longer. Until the first pass is done the
  card says `first pass`. The texels' state between passes
  is held once, updated in place.
  Rebaking on edit (Unity's *Auto Generate*) is not built yet.
- **Prioritize View** (`prioritizeView`, default `true`, Unity's setting of that name).
  The texels the **Scene view's** camera sees converge first: in its frustum, facing
  it (for an orthographic view, its view direction), with nothing static the camera
  draws in between (one ray per texel, before the first pass; a `"ShadowsOnly"` mesh
  hides nothing). The whole scene gets its first pass, so it shows at once; then the
  seen texels get all their other passes, each shown, then the rest get theirs. So a
  bake whose view sees part of the scene makes more steps than passes (`pass 3/15
  (8/64 samples, 24 in view)`), and the same paths in all. Only when each texel is
  traced changes, never the bytes; off, or with a view that sees all or none of the
  scene, every pass covers every texel.

- **Who gets one.** Every active, **static**, opaque mesh with a **second UV map** (the
  lightmap UV: glTF `TEXCOORD_1`, or one generated at import by the model's **Generate
  Lightmap UVs** setting, see [`Assets`](Assets.md#generate-lightmap-uvs)). Every built-in
  primitive carries one (the Sphere, Cylinder and Capsule through that same unwrap); an imported
  mesh without one keeps probe / ambient lighting, and the bake logs once how many static
  meshes were left out that way: *"N static meshes have no lightmap UV — enable Generate
  Lightmap UVs"*.
  Every static mesh still shades and bounces light onto the others, as its Mesh
  `cast_shadows` allows ([`Light`](Light.md#cast-shadows), one rule for every bake):
  an `"Off"` mesh blocks no light but still bounces, emits and is lightmapped; a
  `"ShadowsOnly"` one blocks light but bounces, emits and receives nothing. A mesh at
  Scale In Lightmap `0` (below) gets no lightmap and reads the light probes. Dynamic objects keep
  the light probes; bake those with `Lighting.Bake()` so both read the same lights.
- **What it bakes.** A deterministic CPU path tracer: `samples` (default `64`, enough for OIDN, below) paths
  per texel, `bounces` (default `3`) surfaces deep, seeded by `seed` (default `0`) — the
  same scene and seed always write byte-identical lightmaps, however many cores ran it
  (on any machine under the `"Filter"` denoiser; on one machine under OIDN).
  Each surface a path lands on aims one shadow ray at one source picked from the lamps
  in range and the emissive surfaces near it, by how much each could light it (#1883,
  Unity's Progressive Lightmapper does the same): a level with 150 lamps bakes about as
  fast as one with a few, and a small bright emissive panel bakes smooth instead of
  speckled. Past the second bounce a dim path may end early (Russian roulette), which
  changes only the noise. What the bake converges to changes in one way: a strong
  emitter (emission past about 10) lights its surroundings brighter than before, since
  its light now arrives through those aimed samples rather than through bounce paths
  the per-path clamp (Blender's *Clamp Indirect*) cut down. Each path is still clamped,
  the texel's own sample of an emitter included, so the pool right beside a very
  bright emitter still bakes under its true value.
  `texelsPerUnit` (default `8`) is the lightmap resolution per world unit, times each
  mesh's **Scale In Lightmap** (below). A mesh's lightmap follows its surface area at that
  density, from 4 texels square up to the largest map one atlas page holds with its
  ring: `maxLightmapSize − 8` (1016 at the default 1024, #1704). A map above half the page
  leaves the rest of it to smaller maps. The bounce is then cleaned by the denoiser
  (*Denoising*, below); baked direct light never is, so a point, spot or directional light's shadows stay sharp; an Area
  light's (#1674) are soft, its rectangle integrated over `samples` or more jittered
  points per texel. A texel buried inside other geometry (floor
  under a crate) is filled from its neighbours rather than baked black (*Backface
  Tolerance*, below). No GPU is needed; it runs headless.
- **Backface Tolerance** (`backfaceTolerance`, #1949, Unity's setting of that name,
  default `0.9`, `0`–`1`). Each texel shoots 32 probe rays of its own; it is
  *valid* when at least that share of them reach a front face or escape. A texel
  below it sits inside geometry (a floor texel under a crate, one inside a wall): its
  light is dropped and it is filled from the valid texels of **its own chart** (the
  UV island it lies on), never from a chart packed beside it in the map, so a texel
  at a floor's edge never takes the dark underside next to it. The probes are a fixed,
  seeded budget apart from `samples`, so a texel is valid or not the same way at 16
  samples and at 4096. `0` keeps every texel; `1` drops any that sees a back face.
  **One-sided geometry counts as inside:** a floor under a single quad roof sees
  only the roof's back, so at `0.9` its texels are dropped and filled from the
  floor's lit part beyond the roof. The bake has no *Double Sided GI* yet, so close
  such geometry (a box, not a plane) or lower the tolerance for that scene. A
  chart with no valid texel at all (a box's face resting on a floor or against a
  neighbour, which is ordinary content) is filled from the charts beside it, as
  before, and the bake log says how many in one info line, never a warning.
- **Denoising** (#1884). Only the bounce (indirect light, and a directional bake's
  directions) is cleaned, chart by chart; the direct light stays as traced.
  - `"OIDN"`: Intel **Open Image Denoise 2**'s `RTLightmap` filter, a network trained
    on lightmaps, then the `"Filter"` Gaussian below, as Unity runs its denoiser and
    then its filter (OIDN alone leaves a faint mottling on flat walls at low
    samples, which the Gaussian takes out). At `64` samples the pair comes as close
    to a converged bake as the Gaussian alone did at `128` (within a point of mean
    relative error, measured on a primitive interior, #1884), in about 60% of the
    time, so it is the default and `64` the default `samples`. OIDN is a native library the machine must have, the way
    Blender is for a `.blend`: **macOS** `brew install open-image-denoise`, **Arch**
    `pacman -S openimagedenoise`, elsewhere the release from
    `github.com/RenderKit/oidn/releases`, unpacked anywhere with the `RUSTY_OIDN`
    environment variable naming its folder (or the library file). rusty opens it when
    a bake asks, never at build time, so a machine without it still builds, runs and
    bakes: the bake warns once, naming what to install, and only filters, which at
    `64` samples is blurrier and blotchier (raise `samples`, or pick `"Filter"` to
    silence it). Installed mid-session, the next bake finds it; no restart. Each bake
    worker denoises its own lightmaps on an OIDN device of one thread, beside the
    trace. Its output may differ in the last bits between CPUs (its SIMD path),
    never between runs on one machine.
  - `"Filter"`: an edge-aware Gaussian, 3 texels wide (Unity's indirect filter), that
    never crosses a crease. Byte-identical on every machine.
  - `"None"`: the raw trace, grain and all.
  A logged bake (`Lighting.BakeLightmaps()`, `Lighting.Generate()`) says OIDN's
  worker time and how many lightmaps it cleaned, in a line of its own in the
  summary: it runs inside the lightmaps step.
- **Max Lightmap Size** (`maxLightmapSize`, Unity's setting; the editor's
  **"Max Lightmap Size"** under the Generate Lighting button). The atlas page's edge in
  texels, a power of two from `32` to `4096`, default `1024`; any other value is held to
  the next power of two in that range, when set and when a scene file is loaded. It caps
  both the page and, through it, the largest lightmap a mesh can get. A bake that would
  take more than **256 pages** (the layers one GPU texture array holds) is refused before
  it encodes, with a message naming the page count: raise Max Lightmap Size, or lower
  `texelsPerUnit` or Scale In Lightmap. The scene keeps its previous lightmaps. A scene
  file that already holds more pages than the GPU takes loads unlit by lightmaps, with a
  warning, until it is rebaked.
- **Scale In Lightmap** (Unity's *MeshRenderer* setting, #1704): a per-mesh multiplier
  on `texelsPerUnit`, the Mesh component's `scale_in_lightmap` field (default `1`, `0`–`64`),
  written with `Component.Set(id, "Mesh", { scale_in_lightmap = 2 })`
  ([`Component`](Component.md)), read with `Component.Get`, shown in the Inspector's Mesh
  card and saved with the scene and its prefabs (a file without it reads `1`). A hero
  prop takes more texels, clutter fewer, without changing the scene's density. At `0`
  the mesh gets **no lightmap** and reads the **light probes** at runtime, as in Unity,
  while still blocking and bouncing light in the bake as its `cast_shadows` says. A
  value read from a file is held to the same range when the scene or prefab loads.
- **Directional** (#810, Unity's *Directional Mode*). By default the bake also stores,
  per texel, the **dominant direction** its light arrives from and how strongly it leans
  that way (its *directionality*: 1 when it all comes from one side, 0 when it comes
  evenly from everywhere). The bounce's directions are smoothed by the same filter as its
  colour. At runtime a lightmapped surface reshapes its baked light by how its
  **normal-mapped** normal faces that direction, so bumps, tiles and panel seams read
  under baked light as they do under realtime light (a surface without a normal map
  keeps its baked diffuse exactly as a non-directional bake gives it). Where `Baked`
  lights supply a quarter or more of a texel's light the direction is theirs (eased
  toward the mean of all its light below that), so there a normal map reshapes the
  texel's bounce by the lamp too. Each texel also stores its **lamp share**, how much
  of its light is `Baked` lights' direct light from one way, which gives a glossy
  surface one specular highlight from those lights ([`Light`](Light.md), #1695) and
  none from sky or bounce; a non-directional bake has none. `directional = false` (or the editor's
  **"Directional lightmaps"** checkbox under the Generate Lighting button) skips the direction pages
  to save their memory; the colour lightmaps are byte-identical either way.
- **At runtime** a lightmapped surface takes its ambient term from the lightmap instead
  of probes or the flat sky gradient, and skips the realtime direct light of `Baked`
  lights (it is in the map). `Mixed` lights stay realtime on it; their bounce is baked.
  Lightmapped copies of one prop still draw as one instanced call.
- **Where the files go.** The lightmaps are packed into square **atlas pages** (the
  smallest power of two that holds them all, up to Max Lightmap Size, then as many pages
  of that size as it takes; a gap in a page is filled before another opens, #1694). Each page is a
  `lightmap_<page>_<tag>.ktx2` in `<scene file>.lightmaps/` (old pages there are removed
  first): BC6H HDR light, a byte a texel, with two mips, which the renderer picks between
  by distance; each lightmap sits in a 4-texel ring of its own edge light, in a cell
  aligned to 16 texels, so no compressed block and no texel of any of the three levels
  ever mixes two lightmaps. An RGBM-encoded PNG of the same
  name sits beside it: what a GPU without BC compression loads, and a picture an agent
  can open. A directional bake writes a `lightmap_dir_<page>_<tag>.ktx2` (BC7) and
  `.png` beside each page, packed exactly like its colour page: RGB the direction
  scaled by its directionality, each component `x` stored as the byte `128 + 127·x`
  (so 0 is exact), and A the lamp share (0–1). The texture importer leaves that folder alone
  (`Assets.md`). Encoding takes time, about 2 s per million texels on four cores, after
  the trace: a 2048² page pair about 8 s, a 4096² pair about 30 s; a level whose biggest
  meshes reach the default 1016 cap, about 90 s (#1704); a 4096² page pair takes about 43 MiB of GPU memory, but filling many of them takes a density whose trace stops a bake long before memory does; the editor's Generate Lighting
  encodes the pages and compresses their PNGs on its worker and only writes the finished
  files on the frame, so the window stays live. The scene stores the page list and, per entity, its page and
  scale/offset (Unity's lightmap index + scale/offset), and the lights the bake took
  in (every active `Mixed` and `Baked` light above intensity 0, Unity's
  `Light.bakingOutput`), which
  SSIL leaves out because their bounce is baked (`Graphics.md`, *Screen-space
  indirect lighting*), so **save the scene** after
  `Lighting.BakeLightmaps()` to keep them (`Lighting.Generate()` writes them itself). Needs a saved scene path (errors otherwise). `Lighting.ClearLightmaps()`
  drops the references (the files stay until the next bake).
- **Moving or editing static geometry** after a bake leaves its lightmap stale; rebake.

**Blender export.** The lightmap UV is the mesh's **second UV map**: add one in *Object Data
Properties → UV Maps*, unwrap it with no overlapping islands (*Lightmap Pack*, or *Smart
UV Project* with some island margin), keep the first map for textures, and export glTF 2.0
with *UVs* enabled — Blender writes the second map as `TEXCOORD_1`. Or skip all that and
tick the model's **Generate Lightmap UVs** (Unity's checkbox): rusty unwraps one at import.

### Occlusion culling

Unity's *Occlusion Culling* (#1714, part 2): Generate Lighting bakes, from the static
scene, which static meshes each cell of the level may see, and a camera with
`use_occlusion_culling` on (the default; `Camera.SetUseOcclusionCulling`, the Camera
card's *Occlusion Culling* box) skips the static meshes its cell cannot see. It changes
no pixel: a mesh it skips is one no ray from the camera's cell reaches.

- **What takes part.** rusty has one static flag, so a static mesh is both of Unity's
  `StaticEditorFlags`: an opaque one (no surface shader) is an **occluder and an
  occludee**; a Transparent, Cutout or custom-shader one, one in a `LODGroup`, one
  with a max draw distance or min screen size, and a light's gizmo are **occludees
  only** (what draws of them changes, or a view passes through them). Dynamic,
  skinned, shadows-only and inactive meshes take no part and are always drawn.
- **Cells.** A uniform grid of 2 m cubes over the bounds of the static meshes it can
  cull, one cell of margin round them, coarser cubes past 16384 cells, up to 4 m (a
  Dust2-sized level, 200 × 200 m on two storeys, takes 3.9 m). A level that needs
  larger cells is **not occlusion-culled**: Generate Lighting's console line says so,
  and bakes no PVS. A camera outside every cell culls nothing. Each cell keeps what any
  point in it may see: rays from seeded points to points on each mesh. A cell larger
  than 2 m is sampled as 2 m cubes, six points each, so the eye's density is the same
  at every cell size. The points on a mesh are its extremes and a lattice laid on its
  triangles at most 1 m apart, kept at one per half-metre cube of space, so their
  count follows the mesh's surface area, not its triangles (a 2000-triangle prop gets
  about the points a box of its size does), and a long wall seen through a doorway is
  aimed at wherever the doorway shows it. A point on a one-sided triangle that faces
  away from a sampled cube is skipped. Most rays are never cast (#1714, part 3): an
  occluder face that hides a group of meshes, a mesh, or some of its points from a
  cube or from one of its points proves their rays stopped, and the faces on one
  plane that draw the same side count as one (Unity's occluder fusion, for coplanar
  faces): a floor of tiles, a wall built of pieces with its doorways left open. A
  seam fuses where it falls on the quarter-metre grid of world units, as a modular
  level's do; a slit between two faces is seen through however thin, down to float
  noise, and faces fuse only at one depth (to float noise), never a millimetre
  apart. The faces' triangles fuse whatever their order or triangulation (#1981):
  edges two faces share corner for corner join them. So a wall tessellated into
  triangles that share their vertices, as an exporter writes one mesh (a shared
  corner the same position, bit for bit, once in world space), hides what a box
  does, short of its edges: there it fuses to the quarter-metre texel, where a
  box's whole face is proved exactly. An edge welded only to float noise, or a
  T-junction, is left a seam, which fuses less and is never wrong.
  Each cube tries first the faces that last stopped one of its rays.
  These proofs are exact, so they change how fast a cell bakes, not what it keeps.
  Every cell
  then also takes its 26 neighbours' sets, and cells that see the same set share one
  row. The bake is a pure function of the scene and a fixed seed: the same bytes at
  any thread count.
- **What it can miss, and what it never culls.** A gap that shows less than about a
  metre of a mesh may hide it (points are sampled, not every ray), at every cell size
  the grid allows. A mesh too large to sample (over 4096 points, about 4000 m² of
  surface: both sides of a 600 m wall 3 m high) or with a vertex or a transform that is
  not finite is never culled: it is kept in sight of every cell, lays no cell (a
  ground slab never coarsens the grid), and Generate Lighting's console line and
  `occlusionNeverCulled` count it.
- **Cost** (release build, 4 threads, primitive levels of 10 m rooms on two
  storeys, a doorway in every wall, two crates and a 1150-triangle sphere in each):
  the bench at Kolvik's size (5 × 5 rooms, 397 static meshes, 4704 cells of 2 m) bakes
  in about 4–5 s into 277 KB; a Dust2-sized level (20 × 20 rooms, 6344 meshes, 14580
  cells of 3.9 m, each sampled as eight 2 m cubes) in about 12 minutes into 11.8 MB,
  with the process peaking under 0.6 GB, about what the occluders' BVH alone takes
  (part 2 took some 8 hours on that level). Fewer cores take longer in proportion.
  What costs is the cells under the ground floor and across a storey's slab, where
  only many faces together hide what is past them.
  A level of imported meshes costs about what one of boxes does: a 6 × 6 town of
  tessellated one-mesh buildings on bumpy ground (5120 cells of 2 m) bakes at
  about 700 cells a second, its box-built twin at about 550 (#1981; it was 78
  before tessellated faces fused). Moved and scaled off round coordinates
  (6125 cells), the town bakes at about 560 cells a second against its twin's
  650, casting 2.2 times the rays: a tessellated wall fuses to the texel at its
  edges and openings, where a box's own face is proved exactly. A cell's cost grows with the meshes the level
  holds, slowly where they crowd (5.6 times the meshes, about half the rate) and
  about in proportion to the level's floor area.
  At runtime a camera costs one cell lookup, and each static mesh
  a map lookup and a bit test; a static mesh moved or edited re-checks that mesh alone.
- **Baked alone** by `Lighting.BakeOcclusion()` or the scene inspector's **"Bake
  Occlusion"** button beside Generate Lighting (#1958; Unity bakes occlusion from its
  own window): the same step on the open scene, written beside it, with no lightmap
  or probe touched and the scene file left as it is. Refused as Generate Lighting is:
  in Play, while the editor's bake runs, and for a scene never saved. A level whose
  full Generate takes most of an hour re-bakes its PVS in what *Cost* above says:
  seconds for the primitive bench at Kolvik's size, minutes for a Dust2-sized
  level. (Kolvik itself took 20 minutes before #1981; its time since is not yet
  measured.)
- **Runs** as Generate Lighting's last step, after the lightmaps and probes are
  written into the scene, so a bake dropped or failed during it (the scene closed or
  reloaded, another opened) loses none of the lighting: in the editor on a worker
  thread, with its cells on the card's bar, so the editor and the MCP keep answering;
  `Lighting.Generate()` bakes it in line and logs `[Lighting] occlusion 62%
  (2260/3645 cells)` lines like the other steps.
- **Stored** beside the scene as `<scene>.occlusion` (a small binary file, written
  with the lighting and on every save; removed when the scene has none). A file that
  is truncated or out of range is refused on load with a warning, and nothing is
  occlusion-culled. One that is there but cannot be read (a permission) also culls
  nothing, with a warning, and the next save leaves it in place.
- **A bake older than the scene** never hides what is there: a static mesh moved,
  re-shaped (the bake hashes each mesh's triangles, corner by corner), given a
  material that occludes or faces differently (opaque or not, a surface shader, its
  Render Face), added to or taken out of a `LODGroup`, or made dynamic since is
  drawn, a mesh added since is drawn, and if an occluder changed so, or was removed
  or deactivated, nothing is occlusion-culled until the next Generate Lighting or
  Bake Occlusion. The console says so once, and again if an occluder changes after
  it said only meshes did. Only what the bake reads counts (#1958): a material
  renamed, and a re-export that stores the same triangles in another vertex order,
  keep the bake. The bake reads the live material library and the next load reads
  the file's, so a material a script defined in memory that the scene file lacks
  keeps the bake only when it reads as the default does (opaque, front faces); one
  that is two-sided, Cutout, transparent or has a surface shader does not. So
  `Lighting.Generate()`, `Lighting.BakeOcclusion()` and the editor's two buttons
  warn once, naming the keys, when the file lacks, or holds otherwise, a material
  the baked meshes draw: save the scene so its materials are written. A runtime `Scene.Load` that moves the level's
  ids out of a `DontDestroyOnLoad` entity's way moves the bake's with them.
- **Per camera.** Each perspective camera reads its own position's cell: the base
  camera, a stacked or viewmodel camera, a render-texture camera. An orthographic
  camera (a minimap) sees from a whole plane, not its eye, and a camera whose near
  plane is past 1 m (half a cell) clips walls away, so neither culls by occlusion.
  Probe and reflection bakes cull nothing. A camera whose culling mask leaves out an occluder's layer sees past
  it, so it culls nothing by occlusion. A selected mesh the editor's outline rings is
  always drawn.
- **Per light** (#1714, part 4): a shadowed point or spot light reads the cell its
  own position stands in, and leaves out of its shadow maps the static casters that
  cell cannot see. The camera's view plays no part (a caster the camera cannot see
  may still cast into view). A caster hidden from the light is behind a wall the
  same map draws, so its shadow is the same without it, with the camera's caveat
  (*What it can miss*): a caster the light sees only through a gap showing less
  than about a metre of it may be left out, and lose its shadow through that gap.
  Unlike a camera's miss, which comes back as the camera moves, a still light's
  cached tile keeps it until the light moves or a static caster changes. So a light culls nothing by occlusion while a
  baked occluder casts no shadow (`Mesh` cast mode `Off`) or has no GPU buffers, when
  it is shadowmasked under `Shadowmask` (its walls are in the mask, not the map), or
  stands outside every cell; the sun's cascades never do. It pays where a light's
  map is drawn: a moving shadowed light (a carried flashlight, a swinging lamp) every
  frame, a still one when its static bake re-bakes.
- **See it** in the editor (Unity's *Visualization*): select a camera that culls by
  occlusion, and the Scene
  view outlines the cell it stands in (yellow) and every static mesh the bake hides
  from it (red). `make editor-capture ARGS="--select <camera>"` renders the same.
- **Measure** with `Debug.Stats().occlusion_culled` (per frame, summed over the
  camera views) and `shadow_occlusion_culled` (over the shadow views).

### GPU occlusion culling

Unity 6's *GPU Occlusion Culling* (#1963): with no bake, wherever the camera stands
(outside the baked cells, in the editor's Scene view far from the play area), a
camera with `use_occlusion_culling` on leaves undrawn the opaque meshes that what it
drew hides. The quality preset's `gpu_occlusion_culling` turns it on (every seeded
preset; [`Graphics`](Graphics.md), *Quality*). It changes no pixel: a mesh it skips
is wholly behind depth the camera drew this frame.

- **How.** Two phases per camera, the standard way: the meshes it drew last frame
  draw their depth first; that depth is reduced into a pyramid of farthest depths;
  every opaque mesh's bounds are tested against it on the GPU, at the pyramid level
  where its screen rect spans at most 2×2 texels; the camera's passes then draw the
  ones that passed. A mesh that came out from behind a wall this frame is drawn this
  frame. A mesh whose bounds cross the camera's near plane is always drawn.
- **Beside the bake.** The baked PVS (*Occlusion culling*, above) still runs first
  where it is usable, as Unity keeps Umbra next to the GPU Resident Drawer: what its
  row hides is never tested. Where the bake is missing, older than the scene, or the
  camera stands outside every cell, the GPU test alone culls.
- **What takes part.** Opaque and Cutout meshes, static or not; a skinned mesh is
  tested with its posed bounds. Only what the SSAO depth prepass draws hides
  anything: lit solids, a Cutout texel clipped, a custom shader's own cut or moved
  vertices. A Transparent mesh hides nothing and is always drawn, as are a mesh
  whose bounds aren't known yet, a mesh particle, a selected mesh the editor's
  outline rings, and a mesh whose custom shader cuts, scrolls or moves its
  vertices (`vertex_waves` swells rise out of the mesh's own bounds, so a water
  plane behind a low rim would vanish though its crests show; it still hides
  what is behind it).
- **Which cameras.** Each camera view, its own: the base camera, a stacked or
  viewmodel camera, a render-texture camera, the editor's Scene view. Unlike the
  bake, an orthographic camera and one whose near plane is far out cull too: the test
  reads the depth the camera drew, so a wall its near plane clips away hides
  nothing. Probe and reflection bakes, and a GPU without indirect first-instance
  draws, cull nothing this way.
- **Cost.** Per camera, a depth-only draw of last frame's set, a compute reduction
  of the depth, and one compute test per opaque mesh; the draw calls stay the same
  (indirect). The meshes still go through the CPU's frustum and size culls first.
- **Per cluster** (#1968), the culling half of UE5's Nanite: a static mesh of at
  least 4096 triangles is split at import into clusters of up to 124 triangles,
  each with its bounds and normal cone, kept with the model. After its whole mesh
  passed the test above, each cluster is tested on its own, by the frustum, by
  whether every triangle in it faces away, and by the same pyramid, and only the
  ones that pass are drawn: the half of a canopy or a building shell a wall hides
  is left out. A cluster crossing the near plane is always drawn; a two-sided or
  back-faced material, or a mirrored or non-uniformly scaled instance, takes no
  facing test; a skinned mesh is never split. A mesh whose custom shader moves
  its vertices is split and reordered at import like any other, but the instance
  test never judges it, so its clusters are never tested and it keeps one
  instanced draw; mesh particles likewise. It changes no pixel either, and the preset's
  `gpu_cluster_culling` (on in every seeded preset) turns it off. A split mesh is
  drawn once per instance, not instanced, so many copies of one big mesh cost a
  draw each; past a per-camera budget (8M indices) a batch is tested whole.
- **Measure** with `Debug.Stats().hiz_tested` and `hiz_culled` (read back a frame
  or so late), and `cluster_tested` / `cluster_culled` for the clusters.

### Bake reports

Every bake writes what it measured and what it found **beside the scene** (#1980), one
JSON file per kind: `<scene>.bake/lightmaps.json`, `probes.json`, `reflections.json`,
`occlusion.json` and `navmesh.json` ([`Navigation.md`](Navigation.md)). The next bake of
that kind overwrites it, so two bakes compare by copying the file between them;
Generate Lighting writes the ones for the steps it ran. Only a bake writes one, never a
scene save; nothing reads them at runtime, and an export leaves the `.bake` folder
behind. A report that cannot be written is a console warning, never a failed bake. A bake
of an unsaved scene (`Lighting.Bake()` before the first save) writes none.
`Lighting.GetBakeReport(kind)` returns the file as a table, so a scenario can assert on a
bake: `assert(Lighting.GetBakeReport("occlusion").visibility.culledFraction >= 0.4)`.
A value the bake could not measure is `null` in the file and `nil` in the table.

Every report opens with the same fields:

| Field | What it holds |
|---|---|
| `kind` | the report's kind |
| `engine` | the engine commit that baked it (the one the project file records) |
| `timestamp` | when it was written, Unix seconds |
| `threads` | what the bake ran on: the CPU workers it used for `lightmaps` and `occlusion` (every core), `1` for `navmesh` (one thread), `"gpu"` for `probes` and `reflections` (a GPU capture) |
| `stages` | `{ name, seconds }` per stage, in order; `wallSeconds` is their total |
| `settings` | the settings the bake used |
| `inputFingerprint` | a hash of every input the bake read, hex: the same scene and settings give the same one, and a change to anything the bake reads (for the probes and reflections: the static geometry, its materials' colours, emission and textures, the lights, ambient, sky, skybox, fog, the lightmaps and the lighting settings) gives another |
| `outputBytes` | the bytes it wrote (for the light probes, the SH they carry: their sidecar is written with the scene's lighting) |

Then each kind's own:

- **`lightmaps`:** `maps`, `pageSize`, `pages` (`{ page, maps, occupancy }`, the share of
  the page's texels its maps cover), `meshes`, slowest first (`{ entity, name, page, size,
  texels, charts, seconds, texelsPerSecond, lightsInReach, requestedDensity,
  actualDensity }`: `seconds` is worker time over every thread, the densities are texels
  per metre asked for (texels per unit × Scale In Lightmap) and got), `zeroScaleInLightmap`
  (the meshes left out at Scale In Lightmap 0), `denoise` (`{ charts, seconds }`, `nil`
  without OIDN) and `lights` (`{ index, kind, position, range, bakesDirect, texelsInReach }`).
  A light is in reach of a mesh when the mesh's bounds meet the sphere of its range (a
  directional light reaches every mesh): a spot light's cone is not read.
  `invalidTexels` (the backface texels the bake filled) is `nil`: only the tracer knows it.
- **`probes`:** `count`, `manual`, `bounds` (`{ min, max }`), `bounces` (the wall time of
  each bounce pass), `sampling` (`"grid"`, trilinear, or `"nearest"`, the four nearest
  blended: there is no tetrahedral mesh, so no tetrahedra to count) and `farObjects`
  (`{ count, list }`: the active dynamic drawn objects whose centre (as the renderer takes
  it: the mesh's local bounds under its world matrix, halved) is over
  8 m from the nearest probe, farthest first, each `{ entity, name, centre, nearest }`).
- **`reflections`:** `count`, `probes` (`{ index, position, boxMin, boxMax, resolution,
  importance, manual, bytes, seconds, overlaps }`, `resolution` the face size the bake
  rendered at, `overlaps` the probes whose box this one's overlaps) and `picks`: **which
  probe each active drawn object reflects**, at its bounds' centre as the renderer takes
  it (the mesh's local bounds under its world matrix) (`{ entity, name, centre, probe, weight,
  second, secondWeight }`, `probe` `nil` for the sky). The picks rank the scene's whole
  set: a probe the memory budget leaves out at runtime is not skipped here.
- **`occlusion`:** `cells`, `cellSize`, `tooLarge`, `rows`, `dataBytes`, `objects`,
  `occluders`, `neverCulled`, `occluderTriangles`, `biggestOccluders` (`{ entity, name,
  triangles }`, at most 20) and `visibility`: `meanVisible` and `maxVisible` (objects a
  cell sees), `culledFraction` (the share of the objects a cell culls, on average),
  `alwaysVisible` (how many objects every cell sees; the never culled are among them) and
  `alwaysVisibleNames` (at most 20). `neverCulled` is a count with no names of its own:
  which objects the bake could not sample is known only inside it. `cellSeconds` (`{ mean,
  max }`) is the time a cell took to sample, and `slowestCells` the 20 slowest, slowest
  first (ties to the lower cell): `{ cell, centre, seconds, visible }`, `cell` its index in
  the grid, `centre` its middle, `seconds` its time to the microsecond and `visible` the
  objects its PVS row sees. A cell runs on one worker, so its time is that worker's, read on
  the platform's clock: the sampling itself never reads one, and a timed bake writes the
  same PVS as an untimed one. Both are `nil` when no PVS was kept.

A stage baked in the editor a step a frame (the probes) counts the frames between its
steps too. Lists of "heaviest first" keep the top 20; the counts hold them all.

### Environment

The scene's sky and ambient light (#1612), Unity's `RenderSettings.skybox`,
`ambientLight`, `ambientIntensity` and `ambientMode` with its sky, equator and ground
colours (#1647), and the procedural sky's look (#1620),
Unity's *Skybox/Procedural* tint and ground with HDRP's *Cloud Layer* coverage and
opacity. They are the scene file's `skybox_path`, `ambient_color`,
`ambient_intensity`, `ambient` and `sky` blocks, the same fields the editor's **Scene
Settings** panel (the Inspector with nothing selected) edits, through the same
write: **save the scene** to keep a change. Unlike the bakes they are in every build,
so a game script can drive them too (a day cycle, a mood change); a change made
during Play goes back on Stop, as every scene edit does.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.GetSkybox` | `()` | the panorama's path, `""` for the procedural sky |
| `Lighting.SetSkybox` | `(path)` | — |
| `Lighting.GetAmbientColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientIntensity` | `()` | number |
| `Lighting.SetAmbientIntensity` | `(intensity)` | — |
| `Lighting.GetAmbientMode` | `()` | `"Color"`, `"Gradient"` or `"Skybox"` |
| `Lighting.SetAmbientMode` | `(mode)` | — (an unknown name errors) |
| `Lighting.GetAmbientSkyColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientSkyColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientEquatorColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientEquatorColor` | `(r, g, b)` | — |
| `Lighting.GetAmbientGroundColor` | `()` | `r, g, b` |
| `Lighting.SetAmbientGroundColor` | `(r, g, b)` | — |
| `Lighting.GetSkyZenithColor` | `()` | `r, g, b`, or `nil` while it follows the ambient colour |
| `Lighting.SetSkyZenithColor` | `([r, g, b])` | — |
| `Lighting.GetSkyHorizonColor` | `()` | `r, g, b`, or `nil` while it follows the zenith |
| `Lighting.SetSkyHorizonColor` | `([r, g, b])` | — |
| `Lighting.GetSkyGroundColor` | `()` | `r, g, b`, or `nil` while it follows the zenith |
| `Lighting.SetSkyGroundColor` | `([r, g, b])` | — |
| `Lighting.GetCloudCoverage` | `()` | number, `0`–`1` |
| `Lighting.SetCloudCoverage` | `(coverage)` | — |
| `Lighting.GetCloudOpacity` | `()` | number, `0`–`1` |
| `Lighting.SetCloudOpacity` | `(opacity)` | — |

- **The skybox** is an equirectangular panorama image, its path written as the
  Scene Settings panel takes it (`"assets/textures/sky.png"`). The next frame loads it and re-captures the
  sky's reflection cube from it, so reflections follow. `""` (or `nil`) goes back to
  the procedural sky (below). A path that loads
  nothing draws the renderer's fallback texture, as it would from the scene file.
- **The ambient colour** is linear RGB, each channel clamped to `≥ 0`; a colour with
  a NaN or infinite channel is refused and changes nothing. **The intensity** scales
  it, clamped to `≥ 0` (NaN or `math.huge` reads as 0; the editor's slider stops at
  `5`, a script may go past it). Together they are the flat hemispherical ambient a
  surface takes where no baked probe or lightmap reaches. The procedural sky is
  tinted by the colour alone: the intensity dims or brightens what the ambient lights,
  never the sky itself, as Unity's `ambientIntensity` leaves the skybox. An active **Ambient**
  light entity, when the scene has one, overrides both (see [`Light`](Light.md)),
  and the mode below with them: it always lights as `Color`.
- **The ambient mode** is Unity's *Environment Lighting → Source*. `"Color"` (the
  default) is the hemisphere above: the colour on up-facing surfaces, fading to a
  quarter of it on down-facing ones. `"Gradient"` (Unity's `AmbientMode.Trilight`)
  takes three colours instead: **sky** on up-facing surfaces, **equator** on
  sideways ones and **ground** on down-facing ones, blended by how much a surface
  faces up or down. The intensity scales all three. The colours follow the ambient
  colour's rule (linear RGB, `≥ 0`, a NaN or infinite channel refused) and are kept
  in the other modes; their defaults are the default ambient's hemisphere at its top,
  middle and bottom, so switching the default scene to `"Gradient"` lights it the
  same. `"Skybox"` (Unity's `AmbientMode.Skybox`, Unity's default; rusty's stays
  `"Color"`) lights the scene from the sky itself: the panorama (`SetSkybox`) when
  one is set, else the procedural sky below, projected into three SH bands
  (Unity's *ambient probe*), so an HDRI's blue zenith and warm horizon light up-
  and sideways-facing surfaces with their own colours. The intensity scales it; the
  sky on screen stays as drawn. The projection is made once per change of the sky
  (a different panorama path, the panorama file's bytes or its import settings, a
  procedural setting) and never saved; a scripted ambient colour tinting the procedural sky re-projects
  nothing. A panorama is read as the sky pass draws it (8-bit, no sRGB decode), and
  one that cannot be read lights as the procedural sky does. A scene in `Color` mode
  with default colours leaves the `ambient` block out of its file. Lightmaps and light probes, both Generate Lighting's (`Probe.Bake`)
  and the analytic fill (`Probe.BakeAnalytic`), bake from the same ambient, so an
  open lightmapped or probe-lit surface takes the light an unbaked one facing the
  same way does: exactly in `Color` mode, in `Skybox` mode while the sky's three
  SH bands stay non-negative, and in `Gradient` up to a slight smoothing at the
  horizon (the bakes carry three SH bands, as Unity's ambient probe does). A sky
  with one dominant bright region (a sun-white top over a black ground) rings below
  zero on the far side; the bakes clamp each ray at zero, so a baked surface turned
  away from that region comes out slightly brighter than an unbaked one, which
  clamps once (straight down under that sky: about `0.07` against `0`). As in Unity, light probes and lightmaps bake against this
  ambient, never against the sky the camera draws (#1668): in `Color` and
  `Gradient` the sky reaches surfaces only through reflections (reflection probes
  capture it), so a panorama scene lights its probes and lightmaps from the flat
  ambient, not from the HDRI; in `Skybox` the ambient *is* the sky's light, so the
  HDRI lights unbaked, lightmapped and probe-lit surfaces alike (#1675). But, as
  in Unity, **a changed mode or colour reaches baked surfaces only at the next
  Generate Lighting**: dynamic objects and unbaked surfaces show it at once.
- **The procedural sky** is drawn when no panorama is set: a ground → horizon →
  zenith gradient under a cloud layer. Each colour is linear RGB (channels clamped
  to `≥ 0`; a NaN or infinite channel is refused and changes nothing) and
  **optional**: unset (the default, or a setter called with no arguments) the
  zenith is the ambient colour, the horizon the zenith lifted 60% toward white,
  and the ground a quarter of the zenith, so a scripted ambient colour still tints
  them every frame. A set colour stays put whatever the ambient does.
  **Cloud coverage** (default `0.5`) runs from `0`, a clear sky, to `1`, overcast;
  **cloud opacity** (default `0.6`) from `0`, no clouds, to `1`, solid ones. Both
  clamp to `0`–`1`; NaN or `math.huge` changes nothing. The defaults draw the sky
  rusty has always drawn, and a scene at its defaults leaves the `sky` block out of
  its file. What a surface reflects follows: the sky's reflection cube is re-baked
  once per change, so a script that changes a setting every frame re-bakes every
  frame (the ambient colour never re-bakes it), and reflection-probe bakes capture
  the sky as drawn. In `Color` and `Gradient` mode the ambient a surface takes is
  the ambient colour (or the gradient's colours), not the sky's: the sky's colours
  light nothing, as Unity's skybox lights nothing in those modes. In `Skybox` mode
  they are the light (clouds included, by their average brightness).
