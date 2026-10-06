## `Lighting`

**`Lighting.Generate()`** (#832) is Unity 5's *Generate Lighting* button: every bake a
scene has, in order, with the scene's own settings, and the result **written** — no
separate save. It bakes the lightmaps (`Lighting.BakeLightmaps`, below), then the light
and reflection probes (`Lighting.Bake`), so walls and actors are always lit by the same
bake. The editor's **"Generate Lighting"** button (the scene inspector) is this exact
path. The two individual verbs stay for a script that wants only one of them.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.Generate` *(dev-only)* | `()` | `{ lightmaps, lightProbes, reflectionProbes, saved }` |
| `Lighting.GetSettings` *(dev-only)* | `()` | `{ texelsPerUnit, samples, bounces, seed, directional }` |
| `Lighting.SetSettings` *(dev-only)* | `({ texelsPerUnit?, samples?, bounces?, seed?, directional? })` | — |
| `Lighting.Bake` *(dev-only)* | `([probeSpacing], [reflectionRegion], [perAxisCap])` | `true` if at least one GPU bake ran, `false` if both skipped (no adapter) |
| `Lighting.BakeLightmaps` *(dev-only)* | `([texelsPerUnit], [samples], [bounces], [seed], [directional])` | number of lightmaps written |
| `Lighting.ClearLightmaps` *(dev-only)* | `()` | — |

### Generate Lighting

- **Settings live with the scene** (Unity's Lighting Settings): texels per unit, samples,
  bounces, seed and the directional mode are saved in the scene file's
  `lighting_settings` block, so a rebake never asks for them again.
  `Lighting.SetSettings{ samples = 256 }` changes only the fields it names (each clamped
  to a bakeable value: at least 1 sample and 1 bounce, at least `0.01` texels per unit);
  the editor's card edits the same fields under the button. A scene whose settings are all
  at their defaults leaves the block out of its file. `Lighting.BakeLightmaps` takes the
  same settings for every argument it is not given.
- **The result is written at once**, as Unity writes its lighting data asset: the
  lightmap pages, the probes' baked SH (`<scene>.lighting.json`), and the scene file's
  lighting fields (probe layout, reflection probes, lightmap set, settings). Only those
  fields of the file are rewritten: unsaved edits elsewhere in the scene **stay unsaved**,
  in the editor and on disk. A scene file loading would rewrite (a legacy or hand-edited
  one) is left untouched instead: `saved` is `false`, the editor marks the scene dirty,
  and saving the scene keeps the lighting. A lightmapped mesh that exists only in the
  unsaved edits keeps its lightmap once the scene is saved; until then the file names it
  but has no such entity, and the entry is ignored.
- **Needs a saved scene** (the bakes write beside its file); errors otherwise. The
  probe step needs a GPU/software adapter and skips gracefully without one, as
  `Lighting.Bake` does.
- **In the editor** the lightmap step runs in the background (#808) with a progress
  bar and **Cancel**; a cancelled run writes nothing and keeps the old lighting. When
  the lightmaps land, the probe step and the write follow on the same frame, and the
  console logs one summary line.

### Light and reflection probes

`Lighting.Bake()` (#246) auto-places light probes and reflection probes from the static
scene, then runs **both** probe bakes (`Probe.Bake` + `Reflection.Bake`) in one call: the
probe half of Generate Lighting, and the orchestration over the `Probe` and `Reflection`
namespaces, which stay available for manual placement.

**Auto-placement** (deterministic — a pure function of the static scene, no RNG/clock,
so the same level always yields the same layout):

- **Light probes** — a regular grid through the navigable volume. The bounds are the
  baked **nav surface**'s walkable XZ extent (raised by a few units of headroom so probes
  cover the air actors move through), intersected with the static-geometry AABB. When no
  nav surface has been baked, it **falls back to the static-geometry AABB** alone.
  `probeSpacing` (default `4.0`) is the grid cell size in world units.
- **Reflection probes** — one per coarse region, from subdividing the static AABB into
  `reflectionRegion`-sized (default `12.0`) rooms; each region yields a centroid + a
  parallax box covering it. `perAxisCap` (default `4`) bounds the per-axis region count so
  a huge level can't explode the bake.

**Manual-vs-auto rule** (decided **per set**): if the scene already carries any light
probes, their layout is **kept and only rebaked** — auto-placement runs only for a set
that is currently empty; the reflection set is judged independently the same way. So a
level with no probes gets a full auto-place + bake, while one with hand-authored probes
is a pure rebake — **manual placement is never clobbered**. To force a re-auto-place after
moving static geometry, `Probe.Clear()` / `Reflection.Clear()` first, then `Lighting.Bake()`.

Like the individual bakes, it is **dev-only** and needs a GPU/software adapter; with none
the placement still runs but the bakes skip gracefully (returns `false`, never errors —
except reflections need a saved scene path to write their cubemaps beside). Save the scene
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
  primitive carries one (the Sphere and Cylinder through that same unwrap); an imported
  mesh without one keeps probe / ambient lighting, and the bake logs once how many static
  meshes were left out that way: *"N static meshes have no lightmap UV — enable Generate
  Lightmap UVs"*.
  Every static mesh still shades and bounces light onto the others. Dynamic objects keep
  the light probes; bake those with `Lighting.Bake()` so both read the same lights.
- **What it bakes.** A deterministic CPU path tracer: `samples` (default `128`) paths
  per texel, `bounces` (default `3`) surfaces deep, seeded by `seed` (default `0`) — the
  same scene and seed always write byte-identical lightmaps, however many cores ran it.
  `texelsPerUnit` (default `8`) is the lightmap resolution per world unit (each mesh's
  lightmap is 4–512 texels square). The bounce is smoothed with a 3-texel edge-aware
  Gaussian (Unity's indirect filter) that never crosses a crease; baked direct light is
  not filtered, so its shadows stay sharp. A texel buried inside other geometry (floor
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
