## `Reflection`

Place **reflection probes** — the *specular* sibling of light probes. Each probe is a
capture position, a **box volume** for parallax correction, and a path to its baked
**cubemap** (a KTX2 file). Lit, glossy surfaces reflect the **nearest** probe whose box
contains them, with box-projected parallax correction (the standard Lagarde technique),
so the reflected environment tracks the actual room as the camera moves instead of
behaving like an infinitely-distant skybox. When no probe covers a point, the global
skybox is used as before.

Like light probes, reflection probes are a scene-level dataset (not a component),
addressed by **index** (0-based, in placement order); the whole set — positions, boxes,
and cubemap PATHS — saves into the `.scene` document (the cubemap itself is a referenced
file, never inlined, exactly like the skybox). `Bake` produces those cubemaps; until a
probe is baked it has no cubemap path and the runtime falls back to the skybox.
`Move`/`SetBox`/`SetCubemap`/`Remove` on an out-of-range index are no-ops.

| Function | Signature | Returns |
|---|---|---|
| `Reflection.Add` | `(x, y, z, halfX, halfY, halfZ)` | the new probe's `index` (box is centred on the position with the given half-extents) |
| `Reflection.Move` | `(index, x, y, z)` | — (box stays centred on the new position) |
| `Reflection.SetBox` | `(index, minX, minY, minZ, maxX, maxY, maxZ)` | — (explicit parallax box corners; normalized so min ≤ max) |
| `Reflection.SetCubemap` | `(index, path)` | — (point the probe at a baked cubemap KTX2 file) |
| `Reflection.Bake` *(dev-only)* | `()` | `true` if the bake ran, `false` if no GPU/software adapter was available (skipped) — the real prefiltered cubemap bake |
| `Reflection.Remove` | `(index)` | — |
| `Reflection.Clear` | `()` | — (removes every probe) |
| `Reflection.Count` | `()` | probe count |

`Bake` is the real reflection bake (#245): for every probe it captures the STATIC scene
to a cubemap from the probe's position (dynamic actors excluded), runs a GGX
importance-sample prefilter to build a roughness mip chain (mip ↔ roughness — a mirror
reads the sharp base level, a rough surface a blurrier one), encodes the result to a
**KTX2** file written next to the scene (`reflection_probe_<i>.ktx2`), and points the
probe at it. Glossy surfaces inside the probe's box then reflect that baked cubemap,
parallax-corrected, with roughness selecting the mip — instead of the global skybox. It
needs a GPU or software adapter (e.g. lavapipe); with none it skips gracefully and
returns `false`, never erroring. It **errors** only when the scene is unsaved (there is
nowhere to write the cubemaps). It is **dev-only** (an authoring action driving a headless
renderer), so it is absent from ship builds. Save the scene afterward to persist each
probe's `cubemap_path` into the `.scene` document.
