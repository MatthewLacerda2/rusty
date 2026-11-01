## `Reflection`

Place **reflection probes** — the *specular* sibling of light probes. Each probe is a
capture position, a **box volume** for parallax correction and blending, Unity's
Reflection Probe settings (Baked mode), and a path to its baked **cubemap** (a KTX2
file).

**Blending (Unity's *Blend Probes*).** Probes are picked **per object**, at the centre
of its world bounds. A probe's *influence* there is 1 inside its box and falls
linearly to 0 at `blendDistance` past it. Of the probes that reach the object, ranked
by `importance`, then influence, then the nearer capture point, then the lower index,
the top two share its reflection: equal importance splits it by influence (so walking
through a doorway fades from one room's probe to the next), a lower-importance second
fills only what the first leaves (a small important box is drawn over the room around
it), and the sky takes whatever the probes don't cover. Every fragment of the object
box-projects against each probe's box when that probe's `boxProjection` is on (the
standard Lagarde technique, so the reflection tracks the room as the camera moves); a
fragment outside the box, or a probe with it off, reflects the cubemap as if
infinitely far away. An object no baked probe reaches reflects the global skybox.

**Settings per probe** (`Reflection.Set` / `Reflection.Get`, the scene Inspector's
*Reflection Probes* section, and the saved scene all carry every one):

| Field | Default | Meaning |
|---|---|---|
| `position` | — | `{ x, y, z }` capture point |
| `boxMin`, `boxMax` | — | `{ x, y, z }` box corners (normalized so min ≤ max) |
| `cubemap` | `""` | the baked KTX2 file (empty until baked) |
| `boxProjection` | `true` | parallax-correct against the box |
| `blendDistance` | `1` | metres past the box the probe still reaches, fading out (0 = hard edge) |
| `importance` | `1` | a higher one is drawn over a lower one where they overlap |
| `resolution` | `nil` | face size, a power of two in 16–2048 (others round to the nearest); `nil` bakes at the scene default, and `Set` takes `0` to go back to it |
| `nearClip`, `farClip` | `0.05`, `1000` | the capture cameras' clip planes (far is kept past twice the near) |
| `shadows` | `true` | the capture renders shadows |
| `intensity` | `1` | brightness of the reflection, `0`–`8` |

**Scene defaults** live in the lighting settings (`Lighting.GetSettings()` /
`SetSettings{ reflectionResolution, reflectionBudgetMB, reflectionMaxProbes }`): the resolution a probe
without its own bakes at (**512**), the **memory budget** of resident cubes
(**128 MB**) and the most probes `Lighting.Generate()` places (**512**). Each frame keeps the baked probes whose boxes are nearest the camera (a
box the camera stands in counts as distance zero; ties go to the lower index), at most
32, while the cubes stay within the budget: a cube that would pass it is skipped and
farther ones that fit are still taken; the nearest is always kept. They share one
texture whose face is the largest resident cube's, so a 512 cube costs 8 MB there and
a smaller cube rides in its lower mips. That texture is the only copy of a resident
cube (a file is read, copied in and dropped), and it keeps two spare cubes inside the
budget for the ones that just left range, so a 128 MB budget shades with 14 512 cubes
and pacing back and forth across its edge reads nothing from disk. A cube is judged by
its file header before it is read, and once the budget is full no file is read at
all. A cubemap whose size is not a power of two up to 2048 is skipped with
a console warning (repeated after the next bake), and its objects reflect the skybox.
Probes may share one cubemap file; each keeps its own box. An object whose probes are
not resident blends the ones that are, or reflects the skybox.

Like light probes, reflection probes are a scene-level dataset (not a component),
addressed by **index** (0-based, in placement order); the whole set saves into the
`.scene` document (the cubemap itself is a referenced file, never inlined, exactly like
the skybox). `Bake` produces those cubemaps; until a probe is baked it has no cubemap
path and the runtime falls back to the skybox. `Move`/`SetBox`/`SetCubemap`/`Set`/
`Remove` on an out-of-range index are no-ops, and `Get` returns `nil`.

| Function | Signature | Returns |
|---|---|---|
| `Reflection.Add` | `(x, y, z, halfX, halfY, halfZ)` | the new probe's `index` (box is centred on the position with the given half-extents) |
| `Reflection.Move` | `(index, x, y, z)` | — (box stays centred on the new position) |
| `Reflection.SetBox` | `(index, minX, minY, minZ, maxX, maxY, maxZ)` | — (explicit parallax box corners; normalized so min ≤ max) |
| `Reflection.SetCubemap` | `(index, path)` | — (point the probe at a baked cubemap KTX2 file) |
| `Reflection.Set` | `(index, { position?, boxMin?, boxMax?, cubemap?, boxProjection?, blendDistance?, importance?, resolution?, nearClip?, farClip?, shadows?, intensity? })` | — (changes only the fields it names, each clamped to its range) |
| `Reflection.Get` | `(index)` | every field above as a table, or `nil` |
| `Reflection.Bake` *(dev-only)* | `()` | `true` if the bake ran (or there are no probes, a no-op), `false` if no GPU/software adapter was available (skipped) — the real prefiltered cubemap bake |
| `Reflection.Remove` | `(index)` | — |
| `Reflection.Clear` | `()` | — (removes every probe) |
| `Reflection.Count` | `()` | probe count |

`Bake` is the real reflection bake (#245): for every probe it captures the STATIC scene
to a cubemap from the probe's position (dynamic actors excluded) at its resolution,
through its clip planes, with or without shadows, runs a GGX
importance-sample prefilter to build a roughness mip chain (mip ↔ roughness — a mirror
reads the sharp base level, a rough surface a blurrier one), encodes the result to a
**KTX2** file written next to the scene (`reflection_probe_<i>.ktx2`), and points the
probe at it. Glossy surfaces inside the probe's box then reflect that baked cubemap,
parallax-corrected, with roughness selecting the mip — instead of the global skybox. It
needs a GPU or software adapter (e.g. lavapipe); with none it skips gracefully and
returns `false` rather than erroring. It **errors** when the scene is unsaved (there is
nowhere to write the cubemaps), when a cubemap file cannot be written, and while the
editor's Generate Lighting runs (see `Lighting.md`). It logs its progress, probe by probe, to stderr as it goes (#1667). It is **dev-only** (an authoring action driving a headless
renderer), so it is absent from ship builds. Save the scene afterward to persist each
probe's `cubemap_path` into the `.scene` document.
