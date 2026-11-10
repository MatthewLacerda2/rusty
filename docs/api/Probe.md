## `Probe`

Place and query **light probes** — a scene-level dataset (not a component) that
captures the bounced/ambient light arriving at a point as an L2 spherical-harmonics
(SH) irradiance signal. Dynamic (non-`is_static`) objects sample the interpolated
probe field in the shader instead of the flat ambient term (the scene's `Color`
hemisphere or `Gradient`, `Lighting.md`), so they
pick up directional bounced light. An object takes **one** sample, at the centre of its world bounds (Unity's
`Renderer.bounds.center`, so a mesh modelled in world coordinates is lit where it stands,
not at its origin), for its whole mesh; one carrying a [`LightProbeProxyVolume`](LightProbeProxyVolume.md)
samples a grid over its bounds instead and is lit per pixel from it, so a tall
object goes from a lit top to a dark foot. Probe POSITIONS save into the `.scene` document;
their baked SH saves into a `<scene>.lighting.json` sidecar (heavy, regenerable data
kept out of the diffable scene file), with each probe's shadowmask occlusion beside it
after a `Shadowmask` bake (#1695; [`Light`](Light.md), *Shadowmask*): one value per
channel, how much of that `Mixed` light reaches the probe past static occluders,
interpolated like the SH and read by the objects the probe lights.

Probes are addressed by **index** (0-based, in placement order), not by entity id.
`Move`/`Remove` on an out-of-range index are no-ops. `Remove` drops the grid layout
(indices shift), so re-`FillGrid` after manual edits if you need trilinear sampling.

A probe is **manual** or **placed** (#1737). `Add` and `FillGrid` make manual probes;
`Lighting.Bake()` / Generate Lighting make placed ones. `Move` is a hand edit, so it
makes the probe manual too: the next bake would otherwise put it back. Every bake
replaces the placed probes and keeps the manual ones where they are, placing no probe
within half a step of one. `SetManual` turns either into the other; a saved probe
without the flag loads as placed. `Clear` removes both kinds.

| Function | Signature | Returns |
|---|---|---|
| `Probe.Add` | `(x, y, z)` | the new probe's `index` (manual) |
| `Probe.SetManual` | `(index, manual)` | — (manual probes survive a bake's re-placement) |
| `Probe.IsManual` | `(index)` | `true` if the probe is manual (`false` out of range) |
| `Probe.Move` | `(index, x, y, z)` | — (the probe becomes manual) |
| `Probe.Remove` | `(index)` | — (drops the grid layout) |
| `Probe.Clear` | `()` | — (removes every probe) |
| `Probe.Count` | `()` | probe count |
| `Probe.FillGrid` | `(minX, minY, minZ, maxX, maxY, maxZ, spacing)` | — (regular grid of manual probes; replaces existing probes) |
| `Probe.BakeAnalytic` | `()` | — (deterministic stand-in fill: projects the scene's ambient sky + first directional light to SH) |
| `Probe.Bake` *(dev-only)* | `()` | `true` if the bake ran (or there are no probes, a no-op), `false` if no GPU/software adapter was available (skipped) — the real multi-bounce GI bake |
| `Probe.SampleIrradiance` | `(x, y, z, nx, ny, nz)` | `r, g, b` — interpolated probe irradiance for a surface normal at a world position (linear RGB; black when there are no probes) |

`SampleIrradiance` returns irradiance E (a uniform sky of radiance L reads πL). A
surface under the probe reflects `albedo / π · E`, the same Lambert response direct
lights and baked lightmaps give, so a dynamic object under probes and a static one
under a lightmap of the same surroundings render alike (#807). A `Baked` lamp's direct light lands in
the probes shaped by the scene's diffuse model (`diffuseModel` in `Lighting.md`, #1698):
under `HalfLambert` it reads back as the same squared wrap a live lamp gives, and so does
`BakeAnalytic`'s sun. A probe holds the lamp as seen from its own position, so a probe-lit
object keeps the wrap's back-side light that a lightmapped surface's shadow ray cuts.

`Bake` is the real multi-bounce GI bake (#241, #285): for every probe it captures the
STATIC scene to a cubemap from the probe's position (dynamic actors excluded) and
projects that rendered radiance into L2 irradiance SH — so probes pick up the actual
coloured bounce off nearby static surfaces, directional and position-dependent (a probe
by a red wall reads red; one across the room does not). The capture is HDR (#1696): a
lamp's emissive and a wall lit well above 1 reach the SH at full strength, as they reach
the lightmaps, so a dynamic object next to a bright lamp is lit as brightly as the
lightmapped wall beside it. A texel the half float cannot hold (an emissive past 65504,
a NaN) is held at 65504 or dropped at the capture, never spread through the SH. Where a capture sees no
static surface, a direction that escapes the scene, it takes the scene's **ambient**
(`Lighting.SetAmbientMode`'s `Color`, `Gradient` or `Skybox`, intensity applied), not
the sky the camera draws: Unity bakes light probes against *Environment Lighting*,
and outside `Skybox` mode (where the ambient is the sky's own SH, #1675) the
skybox reaches surfaces only through reflections (reflection probes still capture
it, see [`Reflection`](Reflection.md)), and static surfaces reflect nothing into a
light probe (Unity's light-probe bake carries no specular). So `Bake` and
`BakeAnalytic` share the same ambient term, and a probe-lit object takes the ambient an unbaked or lightmapped
surface beside it does (#1668). The first pass captures direct
light only; each later pass re-lights the static scene with the previous pass's probe
field, folding in one more indirect bounce, so colour bleeds further with each
iteration. It runs **up to 5 bounces** and stops early once a pass adds negligible
energy (the field stops changing), whichever comes first. It needs a GPU or software
adapter (e.g. lavapipe); with none it skips gracefully and returns `false` rather than
erroring. It does error while the editor's Generate Lighting or Bake Occlusion runs (see `Lighting.md`). It logs each bounce's progress to stderr as it goes (see `Lighting.md`, #1667). It is **dev-only** (an authoring action driving a headless renderer), so it is
absent from ship builds. The bake is deterministic — the same scene bakes byte-identical
SH. Save the scene afterward to persist the baked SH into the `<scene>.lighting.json`
sidecar the runtime loads.

`BakeAnalytic` is the headless, render-free stand-in: it ignores occlusion and bounce
(every probe sees the same analytic sky+sun, with a gentle height gradient so the
field varies in space) and stays deterministic, so it runs anywhere with no GPU. Use
it for quick, reproducible fills; use `Bake` for the real lighting. Its sky is the
scene's ambient by direction, `Gradient` included, the same ambient `Bake` sees
where its captures escape the scene. Sampling is
**trilinear** over a grid. For free-placed probes, the automatic placement's layout
(which has no grid: its density follows the geometry) or a point outside the grid, it
blends the **four nearest probes** by inverse squared distance less the fifth's, so a
probe's weight reaches zero just as a nearer one replaces it and an object moving
between probes fades rather than pops. The nearest probes are found through a spatial index the
set builds when it is first sampled after a change, so a sample reads the probes near
it, not the whole set.
