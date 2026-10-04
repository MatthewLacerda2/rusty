## `Probe`

Place and query **light probes** — a scene-level dataset (not a component) that
captures the bounced/ambient light arriving at a point as an L2 spherical-harmonics
(SH) irradiance signal. Dynamic (non-`is_static`) objects sample the interpolated
probe field in the shader instead of the flat hemispherical ambient term, so they
pick up directional bounced light. Probe POSITIONS save into the `.scene` document;
their baked SH saves into a `<scene>.lighting.json` sidecar (heavy, regenerable data
kept out of the diffable scene file).

Probes are addressed by **index** (0-based, in placement order), not by entity id.
`Move`/`Remove` on an out-of-range index are no-ops. `Remove` drops the grid layout
(indices shift), so re-`FillGrid` after manual edits if you need trilinear sampling.

| Function | Signature | Returns |
|---|---|---|
| `Probe.Add` | `(x, y, z)` | the new probe's `index` |
| `Probe.Move` | `(index, x, y, z)` | — |
| `Probe.Remove` | `(index)` | — (drops the grid layout) |
| `Probe.Clear` | `()` | — (removes every probe) |
| `Probe.Count` | `()` | probe count |
| `Probe.FillGrid` | `(minX, minY, minZ, maxX, maxY, maxZ, spacing)` | — (regular grid; replaces existing probes) |
| `Probe.BakeAnalytic` | `()` | — (deterministic stand-in fill: projects the scene's ambient sky + first directional light to SH) |
| `Probe.Bake` *(dev-only)* | `()` | `true` if the bake ran, `false` if no GPU/software adapter was available (skipped) — the real multi-bounce GI bake |
| `Probe.SampleIrradiance` | `(x, y, z, nx, ny, nz)` | `r, g, b` — interpolated probe irradiance for a surface normal at a world position (linear RGB; black when there are no probes) |

`SampleIrradiance` returns irradiance E (a uniform sky of radiance L reads πL). A
surface under the probe reflects `albedo / π · E`, the same Lambert response direct
lights and baked lightmaps give, so a dynamic object under probes and a static one
under a lightmap of the same surroundings render alike (#807).

`Bake` is the real multi-bounce GI bake (#241, #285): for every probe it captures the
STATIC scene to a cubemap from the probe's position (dynamic actors excluded) and
projects that rendered radiance into L2 irradiance SH — so probes pick up the actual
coloured bounce off nearby static surfaces, directional and position-dependent (a probe
by a red wall reads red; one across the room does not). The first pass captures direct
light only; each later pass re-lights the static scene with the previous pass's probe
field, folding in one more indirect bounce, so colour bleeds further with each
iteration. It runs **up to 5 bounces** and stops early once a pass adds negligible
energy (the field stops changing), whichever comes first. It needs a GPU or software
adapter (e.g. lavapipe); with none it skips gracefully and returns `false`, never
erroring. It is **dev-only** (an authoring action driving a headless renderer), so it is
absent from ship builds. The bake is deterministic — the same scene bakes byte-identical
SH. Save the scene afterward to persist the baked SH into the `<scene>.lighting.json`
sidecar the runtime loads.

`BakeAnalytic` is the headless, render-free stand-in: it ignores occlusion and bounce
(every probe sees the same analytic sky+sun, with a gentle height gradient so the
field varies in space) and stays deterministic, so it runs anywhere with no GPU. Use
it for quick, reproducible fills; use `Bake` for the real lighting. Sampling is
**trilinear** over a grid, falling back to the nearest probe for free-placed probes or
points outside the grid.
