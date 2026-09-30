## `Navigation`

The navmesh is a height-field surface (#130): each grid cell carries a baked
surface height, so paths follow ramps, stairs, and multi-level terrain in real `y`
rather than a flat `y = 0` plane. The returned waypoint's `y` is the surface height
of the next cell — agents climb and descend instead of sliding through geometry.

| Function | Signature | Returns |
|---|---|---|
| `Navigation.GetNextPathStep` | `(cx, cy, cz, tx, ty, tz)` | next waypoint `x, y, z` on the baked surface along the A* path |

### Per-scene bake settings (#276)

The bake tunables are authored **per scene** (Unity's per-scene navmesh bake
settings) and serialize with it. Every setter writes `scene.nav_settings` and
**re-bakes** the navmesh, so the change takes effect at once — the same effect as the
scene inspector's **Navmesh** section (editor↔API parity). `AgentRadius` **erodes the
walkable surface** at bake time (#277, the standard Recast/Unity meaning): the surface is
pulled back off every wall — and off the world edge — by the radius, so passages narrower
than ~`2 * radius` close up and paths keep clearance instead of hugging geometry. A radius
of `0` is an exact no-op (the surface hugs geometry as before). `AgentHeight` **carves
low-headroom cells** at bake time (#278, the radius's vertical companion): a walkable cell
whose clearance to the lowest static geometry overhead is below the height is marked
non-walkable, so the agent can't path under a low overhang or through a crawlspace. A cell
with nothing overhead has infinite headroom and is never carved (height `0` and overhead-free
scenes are no-ops). This is single-surface only — it subtracts low cells, it does not add a
second walkable layer.

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetAgentRadius` | `()` | agent radius (world units) |
| `Navigation.SetAgentRadius` | `(radius)` | writes `nav_settings.agent_radius` + re-bakes (erodes walkable surface by radius) |
| `Navigation.GetAgentHeight` | `()` | agent height (world units) |
| `Navigation.SetAgentHeight` | `(height)` | writes `nav_settings.agent_height` + re-bakes (carves cells with overhead clearance below the height) |
| `Navigation.GetMaxSlope` | `()` | max walkable grade (rise per unit travelled) |
| `Navigation.SetMaxSlope` | `(slope)` | writes `nav_settings.max_slope` + re-bakes |
| `Navigation.GetMaxStep` | `()` | max step height between adjacent cells |
| `Navigation.SetMaxStep` | `(step)` | writes `nav_settings.max_step` + re-bakes |
| `Navigation.GetGridSpacing` | `()` | grid cell size (world units) |
| `Navigation.SetGridSpacing` | `(spacing)` | writes `nav_settings.grid_spacing` + re-bakes (re-shapes the grid) |

### Navmesh bounds (#452)

The grid covers an XZ rectangle resolved from the scene **on every bake**. By default it
is the extent of the static geometry the bake reads (active, static colliders' world
AABBs), grown by `2.0` plus the agent radius on every side so the radius erosion never
eats the ground around the outermost geometry. An **empty scene** (no static geometry)
bakes over a default `±20` box. A scene can instead **author** its bounds (Unity's nav
volume): they serialize in `nav_settings.bounds` and are editable from the scene
inspector's **Navmesh** section (editor↔API parity). Either way the bounds snap outward to
multiples of `GridSpacing`, so cell centres stay on a fixed world lattice. Anything outside
the bounds is unnavigable; the bake is O(cells), so very large bounds cost bake time.

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetBounds` | `()` | `minX, maxX, minZ, maxZ, authored` — the bounds the last bake used (snapped), and whether they were authored |
| `Navigation.SetBounds` | `(minX, maxX, minZ, maxZ)` | writes `nav_settings.bounds` + re-bakes; errors unless finite with `min < max` |
| `Navigation.ClearBounds` | `()` | clears the authored bounds (back to derived from geometry) + re-bakes |
