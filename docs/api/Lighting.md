## `Lighting`

**`Lighting.Generate()`** (#832) is Unity 5's *Generate Lighting* button: every bake a
scene has, in order, with the scene's own settings, and the result **written** — no
separate save. It bakes the lightmaps (`Lighting.BakeLightmaps`, below), then the light
and reflection probes (`Lighting.Bake`), so walls and actors are always lit by the same
bake. The editor's **"Generate Lighting"** button (the scene inspector) is this exact
path. The call returns when the bake is done (Unity's `Lightmapping.Bake`); the button
bakes the same probes a step a frame — one reflection probe or a few light probes —
so the editor and the MCP keep answering (Unity's `BakeAsync`, #1602). Both say each
bounce pass and reflection probe in the console (`Baking reflection probe 3 of 12`).
The two individual verbs stay for a script that wants only one of them.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.Generate` *(dev-only)* | `()` | `{ lightmaps, lightProbes, reflectionProbes, saved }` |
| `Lighting.GetSettings` *(dev-only)* | `()` | `{ texelsPerUnit, samples, bounces, seed, directional, diffuseModel, reflectionResolution, reflectionBudgetMB, reflectionMaxProbes }` |
| `Lighting.SetSettings` *(dev-only)* | `({ texelsPerUnit?, samples?, bounces?, seed?, directional?, diffuseModel?, reflectionResolution?, reflectionBudgetMB?, reflectionMaxProbes? })` | — |
| `Lighting.Bake` *(dev-only)* | `([probeSpacing], [reflectionRegion])` | `true` if at least one GPU bake ran, `false` if both skipped (no adapter) |
| `Lighting.BakeLightmaps` *(dev-only)* | `([texelsPerUnit], [samples], [bounces], [seed], [directional])` | number of lightmaps written |
| `Lighting.ClearLightmaps` *(dev-only)* | `()` | — |

### Generate Lighting

- **Settings live with the scene** (Unity's Lighting Settings): texels per unit, samples,
  bounces, seed and the directional mode are saved in the scene file's
  `lighting_settings` block, so a rebake never asks for them again.
  `Lighting.SetSettings{ samples = 256 }` changes only the fields it names (each clamped
  to the editor's range: `0.25`–`64` texels per unit, `1`–`4096` samples, `1`–`16`
  bounces; a NaN or infinite density takes the default `8`); the editor's card edits the
  same fields under the button. The reflection probes' defaults sit beside them:
  `reflectionResolution` (default `512`, a power of two in 16–2048) and
  `reflectionBudgetMB` (default `128`, `8`–`2048`) and `reflectionMaxProbes` (default
  `512`, `1`–`4096`, the most probes auto-placement makes), edited in the scene Inspector's
  *Reflection Probes* section (see `Reflection.md`). A scene whose settings are all
  at their defaults leaves the block out of its file. `Lighting.BakeLightmaps` takes the
  same settings for every argument it is not given.
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
  bar and **Cancel**; a cancelled run writes nothing and keeps the old lighting. When
  the lightmaps land they are applied (the probes are captured under them), then the
  probe step runs a step a frame and the console logs one summary line at the end.
  The probe step's placement is planned whole in the frame it starts: about 0.75 s
  in a release build on a Dust2-sized level (10 × 10 rooms of 20 m and a 40 m tower:
  0.41 s for the occupancy grid and floor plan, 0.05 s for the reflection probes,
  0.29 s for 7,638 light probes), so the editor stalls that long once.
- **From a script it logs its progress** (#1667), as Unity's batch-mode bake does: the
  verb blocks its caller until it returns, so a `play` scenario or a `session-mcp` call
  cannot poll, and instead the bake prints a line to **stderr** at each step and every
  5 s within one, with an ETA from the step's own rate, then the total time:
  `[Lighting] lightmaps 41% (612/1501 meshes, 3.1M/7.6M texels), ~38 min left`, then
  `light probes, bounce 2 of at most 5: …, ~1 min left in this bounce` and
  `reflection probes 19% (3/16 probes), ~4 min left`, and `[Lighting] done in 1 h 4 min`.
  The lightmap step's first line and the total also go to the console (so `play`'s
  `console.log` and the MCP reply keep them), beside the probe step's own `Baking …`
  lines; the lines in between go to stderr only. A probe step with no probes of its
  kind prints nothing. `Lighting.Bake()`,
  `Lighting.BakeLightmaps()`, `Probe.Bake()` and `Reflection.Bake()` log the same way. A
  log never changes what is baked: the lightmaps are byte-identical with or without one.
- **The probes bake from a snapshot and apply at the end** (Unity's shape, #1602): the
  light and reflection probes are copied when the probe step starts, and their SH and
  cubemaps are applied in one step when it finishes. Adding, removing or moving a probe
  (or changing a reflection probe's resolution, clip planes or shadows) while it runs
  drops the bake with a console line: the probes keep their old lighting and the
  already-applied lightmaps are kept, with the scene marked unsaved; a probe step that
  fails for another reason (a cubemap that cannot be written) marks it unsaved too.
  Saving this one elsewhere drops it the same way; opening another scene drops it with
  nothing applied to the new one.
- **One bake at a time:** `Lighting.Generate()`, `Lighting.Bake()`,
  `Lighting.BakeLightmaps()`, `Probe.Bake()` and `Reflection.Bake()` error while the
  editor's Generate Lighting is running.

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
editor's button runs this exact path as its first step, in the background (#808): the
window stays live, a progress bar counts texels done, and **Cancel** stops the bake and
keeps the previous lightmaps. The result is applied when the bake finishes,
byte-identical to the script verb's; a bake whose scene was swapped out meanwhile is
dropped. The script verb itself stays synchronous. An argument left out takes the
scene's lighting settings (above), whose defaults are the ones listed here.

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
  `"ShadowsOnly"` one blocks light but bounces, emits and receives nothing. Dynamic objects keep
  the light probes; bake those with `Lighting.Bake()` so both read the same lights.
- **What it bakes.** A deterministic CPU path tracer: `samples` (default `128`) paths
  per texel, `bounces` (default `3`) surfaces deep, seeded by `seed` (default `0`) — the
  same scene and seed always write byte-identical lightmaps, however many cores ran it.
  `texelsPerUnit` (default `8`) is the lightmap resolution per world unit (each mesh's
  lightmap is 4–512 texels square). The bounce is smoothed with a 3-texel edge-aware
  Gaussian (Unity's indirect filter) that never crosses a crease; baked direct light is
  not filtered, so a point, spot or directional light's shadows stay sharp; an Area
  light's (#1674) are soft, its rectangle integrated over `samples` or more jittered
  points per texel. A texel buried inside other geometry (floor
  under a crate) is filled from its neighbours rather than baked black. No GPU is needed;
  it runs headless.
- **Directional** (#810, Unity's *Directional Mode*). By default the bake also stores,
  per texel, the **dominant direction** its light arrives from and how strongly it leans
  that way (its *directionality*: 1 when it all comes from one side, 0 when it comes
  evenly from everywhere). The bounce's directions are smoothed by the same filter as its
  colour. At runtime a lightmapped surface reshapes its baked light by how its
  **normal-mapped** normal faces that direction, so bumps, tiles and panel seams read
  under baked light as they do under realtime light; a surface without a normal map
  shades exactly as a non-directional bake would. `directional = false` (or the editor's
  **"Directional lightmaps"** checkbox under the Generate Lighting button) skips the direction pages
  to save their memory; the colour lightmaps are byte-identical either way.
- **At runtime** a lightmapped surface takes its ambient term from the lightmap instead
  of probes or the flat sky gradient, and skips the realtime direct light of `Baked`
  lights (it is in the map). `Mixed` lights stay realtime on it; their bounce is baked.
  Lightmapped copies of one prop still draw as one instanced call.
- **Where the files go.** The lightmaps are packed into square **atlas pages** (the
  smallest power of two that holds them all, up to 1024², then as many 1024² pages as it
  takes), each an RGBM-encoded PNG in `<scene file>.lightmaps/` (old pages there are
  removed first). A directional bake writes a `lightmap_dir_<page>_*.png` beside each
  page: plain RGBA8, RGB the direction mapped to 0–1 and A the directionality, packed
  exactly like its colour page. The scene stores the page list and, per entity, its page and
  scale/offset (Unity's lightmap index + scale/offset), so **save the scene** after
  `Lighting.BakeLightmaps()` to keep them (`Lighting.Generate()` writes them itself). Needs a saved scene path (errors otherwise). `Lighting.ClearLightmaps()`
  drops the references (the files stay until the next bake).
- **Moving or editing static geometry** after a bake leaves its lightmap stale; rebake.

**Blender export.** The lightmap UV is the mesh's **second UV map**: add one in *Object Data
Properties → UV Maps*, unwrap it with no overlapping islands (*Lightmap Pack*, or *Smart
UV Project* with some island margin), keep the first map for textures, and export glTF 2.0
with *UVs* enabled — Blender writes the second map as `TEXCOORD_1`. Or skip all that and
tick the model's **Generate Lightmap UVs** (Unity's checkbox): rusty unwraps one at import.

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
