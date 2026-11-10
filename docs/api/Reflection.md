## `Reflection`

Place **reflection probes** — the *specular* sibling of light probes. Each probe is a
capture position, a **volume** for parallax correction and blending (a box, which may
be turned, or a sphere: HDRP's two shapes), Unity's Reflection Probe settings (Baked
mode), and a path to its baked **cubemap** (a KTX2 file).

**The volume (#1995).** A box probe's box is `boxMin`..`boxMax` turned by its
`rotation` about its `position` (HDRP's oriented box; there is no scale, the corners
carry the size); a sphere probe's is the sphere of `radius` about its `position`.
The volume is both what the reflection is projected against and where the probe
applies (its influence is the volume grown by `blendDistance`).

**Blending (Unity's *Blend Probes*).** Probes are picked **per object**, at the centre
of its world bounds. A probe's *influence* there is 1 inside its volume and falls
linearly to 0 at `blendDistance` past it. Of the probes that reach the object, ranked
by `importance`, then influence, then the nearer capture point, then the lower index,
the top two share its reflection: equal importance splits it by influence (so walking
through a doorway fades from one room's probe to the next), a lower-importance second
fills only what the first leaves (a small important box is drawn over the room around
it), and the sky takes whatever the probes don't cover. Every fragment of the object
projects against each probe's volume when that probe's `boxProjection` is on (the
standard Lagarde technique, so the reflection tracks the room as the camera moves),
a fragment outside the volume too: its ray's distance to where it leaves each slab of
the box (or the sphere) is taken as a magnitude, so the reflection is continuous
across every face, and a large object reaching past one of its two probes, a floor
under a row of turned boxes in a round room, shows no straight step where a box ends.
Only a probe with `boxProjection` off reflects the cubemap as if infinitely far away. An object no baked probe reaches reflects the global skybox, or
the scene's **fallback probe** when it names one (below).

**Normalised by the baked light (#1697).** A probe's cube shows its room as lit at the
capture point, so on its own a glossy floor in a dark corner would reflect the bright
room at full strength. Each bake therefore also stores the **irradiance** at the probe,
measured as a light probe there measures it: its capture's L2 SH, with the scene's
ambient (not the drawn sky) where the view escapes, which is what the lightmap's
escaping rays see too. The direct light of `Baked` lamps is not in it (the capture
sees what they light, never the lamps), so where a probe sits beside a lamp does not
change the room's reflections. Each pixel scales a probe's reflection by *luminance
of its own baked diffuse* ÷ *luminance of the probe's stored irradiance along its
normal*, held to **0–1** (UE's lightmap mixing, Frostbite's probe normalisation): it
removes leaks and never brightens, so a floor lit brighter than the light the probe
saw keeps its reflection whole. The stored side is floored at a quarter of the
probe's mean, so an L2 SH ringing below zero on the far side of one bright lamp
never saturates the scale, and both sides add a small bias, so a pixel and a probe
that both hold no light scale by 1. The pixel's baked diffuse is its lightmap on a
lightmapped surface and its light-probe SH on a dynamic object. Where there is neither
(no bake), or the probe was baked without the irradiance, the scale is 1; the sky is
never scaled. A bake capture never normalises, so what one probe's capture sees does
not depend on another probe's stored irradiance. Pointing a probe at another cube
(`SetCubemap`, `Set{ cubemap }`) drops its stored irradiance until it is baked again.
With no ambient at all, an upward floor under an open sky still darkens its sky
reflection, since only the surfaces' sheen of the sky is stored and the lightmap holds
none of it.
Light from `Mixed` and `Realtime` lights is in neither side: it shades live.

**Specular occlusion.** The whole environment reflection (probes, sky and SSR's hit)
is weighted by Frostbite's specular occlusion from the ambient occlusion, roughness and
view angle, as HDRP's `GetSpecularOcclusionFromAmbientOcclusion` has it. The occlusion
is SSAO's times the decals'; with SSAO off and no decal it is 1, and the baked
normalisation above is what darkens a corner.

**The fallback probe** (Unity's default reflection probe, `SetFallback`): one baked
probe a scene names takes the share the sky took: everything an object's probes leave,
all of it for an object outside every box. It stays resident wherever the camera is
(it is taken first, whatever the budget, and so is the nearest probe after it, so a
large fallback never evicts the camera's own room's probe) and is always slot 0 of
the resident probes.
With none named, or the named one not baked, that share is the sky's. Naming a probe
makes it manual, and it stays manual through any edit (`Set{ manual = false }`, the
Inspector's *Manual*), so Generate Lighting keeps it; removing it, `Clear`, or loading a
scene that names none leaves the scene without one. A file naming two keeps the first.

**Settings per probe** (`Reflection.Set` / `Reflection.Get`, the scene Inspector's
*Reflection Probes* section, and the saved scene all carry every one):

| Field | Default | Meaning |
|---|---|---|
| `position` | — | `{ x, y, z }` capture point |
| `boxMin`, `boxMax` | — | `{ x, y, z }` box corners (normalized so min ≤ max), before `rotation` turns the box about `position` |
| `rotation` | `{ 0, 0, 0 }` | `{ x, y, z }` Euler degrees, as `Transform`'s: the box's orientation about `position`; a `Set` of the angles `Get` returned leaves it as it was |
| `shape` | `"Box"` | `"Box"` or `"Sphere"` (any other name is an error) |
| `radius` | `5` | the sphere's radius in metres (at least 0.01; read only by a sphere) |
| `cubemap` | `""` | the baked KTX2 file (empty until baked) |
| `boxProjection` | `true` | parallax-correct against the volume |
| `blendDistance` | `1` | metres past the volume the probe still reaches, fading out (0 = hard edge) |
| `importance` | `1` | a higher one is drawn over a lower one where they overlap |
| `resolution` | `nil` | face size, a power of two in 16–2048 (others round to the nearest); `nil` bakes at the scene default, and `Set` takes `0` to go back to it |
| `nearClip`, `farClip` | `0.05`, `1000` | the capture cameras' clip planes (far is kept past twice the near) |
| `shadows` | `true` | the capture renders shadows |
| `intensity` | `1` | brightness of the reflection, `0`–`8` |
| `fallback` | `false` | read-only in `Get`: this is the scene's fallback probe (set it with `SetFallback`) |
| `normalised` | `false` | read-only in `Get`: the bake stored this probe's irradiance, so its reflection is normalised by the baked light (re-bake a probe baked before it) |
| `manual` | `true` from `Add`, `false` when placed | a bake's auto-placement keeps a manual probe as it is and leaves the room it covers to it; it re-places the others (`Lighting.md`, #1737). A hand edit (`Move`, `SetBox`, `Set`, an Inspector field) makes a probe manual, so a tuned box survives the next bake, `local p = Reflection.Get(i); …; Reflection.Set(i, p)` included; `Set{ manual = false }` un-marks a probe that is manual already (on a placed one it is ignored, since a round trip carries the `false` it read), and unticking *Manual* in the Inspector does too |

**Scene defaults** live in the lighting settings (`Lighting.GetSettings()` /
`SetSettings{ reflectionResolution, reflectionBudgetMB, reflectionMaxProbes }`): the resolution a probe
without its own bakes at (**512**), the **memory budget** of resident cubes
(**256 MB**) and the most probes `Lighting.Generate()` places (**512**). Each frame keeps the baked probes whose volumes are nearest the camera (a
volume the camera stands in counts as distance zero; ties go to the lower index), at most
32, while the cubes stay within the budget: a cube that would pass it is skipped and
farther ones that fit are still taken; the nearest is always kept, and so is the
fallback probe (above). They share one
texture whose face is the largest resident cube's, so a 512 cube costs 2 MB there
(BC6H, a byte a texel, with its mips; 16 MB of half floats on a GPU without BC
compression, where cubes are decoded when they load) and a smaller cube rides in its
lower mips. That
texture is the only copy of a resident cube (a file is read, copied in and dropped), and
it keeps two spare cubes inside the budget for the ones that just left range, so a
256 MB budget shades with 32 512 cubes, the cap (14 on a GPU without BC), and pacing back and forth across its
edge reads nothing from disk. A cube is judged by
its file header before it is read, and once the budget is full no file is read at
all. A cubemap whose size is not a power of two up to 2048 is skipped with
a console warning (repeated after the next bake), and its objects reflect the skybox;
so is a half-float or 8-bit cube baked before BC6H probes, with a warning to re-bake it.
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
| `Reflection.Add` | `(x, y, z, halfX, halfY, halfZ)` | the new probe's `index` (box is centred on the position with the given half-extents; the probe is manual) |
| `Reflection.Move` | `(index, x, y, z)` | — (box stays centred on the new position) |
| `Reflection.SetBox` | `(index, minX, minY, minZ, maxX, maxY, maxZ)` | — (explicit parallax box corners; normalized so min ≤ max) |
| `Reflection.SetFallback` | `(index \| nil)` | `true`, or `false` for an index past the set (nothing changes) — name the fallback probe, or none with `nil`; any other loses the flag |
| `Reflection.GetFallback` | `()` | the fallback probe's `index`, or `nil` |
| `Reflection.SetCubemap` | `(index, path)` | — (point the probe at a baked cubemap KTX2 file) |
| `Reflection.Set` | `(index, { position?, boxMin?, boxMax?, rotation?, shape?, radius?, cubemap?, boxProjection?, blendDistance?, importance?, resolution?, nearClip?, farClip?, shadows?, intensity?, manual? })` | — (changes only the fields it names, each clamped to its range) |
| `Reflection.Get` | `(index)` | every field above as a table, or `nil` |
| `Reflection.Bake` *(dev-only)* | `()` | `true` if the bake ran, `false` if no GPU/software adapter was available (skipped, nothing written or removed) — the real prefiltered cubemap bake. With **no probes it is not a no-op**: it deletes every cube in the scene's `<scene file>.reflections/` folder and returns `true` |
| `Reflection.Remove` | `(index)` | — |
| `Reflection.Clear` | `()` | — (removes every probe) |
| `Reflection.Count` | `()` | probe count |

`Bake` is the real reflection bake (#245): for every probe it captures the STATIC scene
to a cubemap from the probe's position (dynamic actors excluded) at its resolution,
through its clip planes, with or without shadows, in **HDR**: the radiance the forward
pass computes, with no grade, tonemap or clamp, so a lamp of radiance 40 is 40 in the
cube (#1696). It runs a GGX
importance-sample prefilter to build a roughness mip chain (mip ↔ roughness — a mirror
reads the sharp base level, a rough surface a blurrier one; each sample reads the
capture's mip its solid angle covers, so a small bright lamp blurs into a smooth glow
on rough metal instead of speckle), encodes every face and mip as BC6H unsigned float
(`BC6H_UFLOAT_BLOCK`, Unity's desktop format for an HDR cubemap, an eighth of half
floats; a 2×2 or 1×1 mip is one padded 4×4 block) into a **KTX2** file written to the scene's own folder (`<scene file>.reflections/reflection_probe_<i>.ktx2`,
as Unity keeps a scene's baked probes in a folder named after it, so two scenes in one
folder never overwrite each other's cubes; cubes there past the probe count are removed,
nothing outside it is touched), and points the
probe at it. Glossy surfaces inside the probe's box then reflect that baked cubemap,
parallax-corrected, with roughness selecting the mip — instead of the global skybox. It
needs a GPU or software adapter (e.g. lavapipe); with none it skips gracefully and
returns `false` rather than erroring. It **errors** when the scene has probes and is
unsaved (there is nowhere to write the cubemaps), when a cubemap file cannot be written
or a stale one removed, and while the
editor's Generate Lighting or Bake Occlusion runs (see `Lighting.md`). It logs its progress, probe by probe, to stderr as it goes (#1667). It is **dev-only** (an authoring action driving a headless
renderer), so it is absent from ship builds. Save the scene afterward to persist each
probe's `cubemap_path` and its stored `irradiance` into the `.scene` document.
