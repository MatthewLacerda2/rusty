## `Lighting`

The **one-button** image-based-lighting bake (#246): `Lighting.Bake()` auto-places light
probes and reflection probes from the static scene, then runs **both** existing bakes
(`Probe.Bake` + `Reflection.Bake`) in one call. It is the "place well + bake right now"
workflow — the orchestration over the `Probe` and `Reflection` namespaces, which stay
available for manual placement. The editor's **"Bake Lighting"** button (the scene
inspector) routes through this exact same path, so the button and this verb never drift.

| Function | Signature | Returns |
|---|---|---|
| `Lighting.Bake` *(dev-only)* | `([probeSpacing], [reflectionRegion], [perAxisCap])` | `true` if at least one GPU bake ran, `false` if both skipped (no adapter) |

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
afterward to persist the baked SH (`<scene>.lighting.json`) and the cubemap paths.
