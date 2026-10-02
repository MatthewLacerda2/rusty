## `Navigation`

The navmesh is a **layered grid** (#454, the compact-heightfield half of Recast): every
XZ cell holds a list of walkable **spans**, each a floor height plus the open space above
it. Stacked floors, a catwalk over a corridor and a bridge over a road are all walkable at
once, and paths climb stairs and ramps from one floor to the next in real `y`.

The bake reads the **real geometry** of every active, static, non-trigger collider: the
same shapes physics collides with (rotated boxes, ramps, spheres, capsules, convex hulls
and triangle meshes), rasterised into each cell column. A ramp bakes as a slope, not a
plateau at its highest point. **Where there is no collider there is no navmesh**: no
implicit ground past the level's edge, and an empty scene bakes nothing (as in Unity).

`GetNextPathStep` resolves the start and the target to spans with one rule: **the nearest
span below the point, favouring the floor under a character's feet** — the highest floor
at most `MaxStep` above the point, else the lowest floor above it. A chest-height target
therefore means the floor under it, not the one overhead. A point over a column with no
span (a wall, a hole) uses the nearest column with one, up to 5 cells away. The returned
waypoint's `y` is the next span's floor; with no path it returns the target itself.

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
of `0` is an exact no-op (the surface hugs geometry as before). `AgentHeight` **drops
low-headroom spans** at bake time (#278, the radius's vertical companion): a span whose
open space up to the next solid above is below the height is not walkable, so the agent
can't path under a low overhang or through a crawlspace, and a move between two spans
needs that height in the gap they share. The floor *above* an overhang stays walkable.
`MaxSlope` also decides which surfaces are walkable at all: a triangle steeper than it is
not a floor.

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetAgentRadius` | `()` | agent radius (world units) |
| `Navigation.SetAgentRadius` | `(radius)` | writes `nav_settings.agent_radius` + re-bakes (erodes walkable surface by radius) |
| `Navigation.GetAgentHeight` | `()` | agent height (world units) |
| `Navigation.SetAgentHeight` | `(height)` | writes `nav_settings.agent_height` + re-bakes (drops spans with less open space above than the height) |
| `Navigation.GetMaxSlope` | `()` | max walkable grade (rise per unit travelled) |
| `Navigation.SetMaxSlope` | `(slope)` | writes `nav_settings.max_slope` + re-bakes |
| `Navigation.GetMaxStep` | `()` | max step height between spans in adjacent cells |
| `Navigation.SetMaxStep` | `(step)` | writes `nav_settings.max_step` + re-bakes |
| `Navigation.GetGridSpacing` | `()` | grid cell size (world units) |
| `Navigation.SetGridSpacing` | `(spacing)` | writes `nav_settings.grid_spacing` + re-bakes (re-shapes the grid) |

### Navmesh bounds (#452)

The grid covers an XZ rectangle resolved from the scene **on every bake**. By default it
is the extent of the static geometry the bake reads (active, static, non-trigger
colliders' world AABBs), grown by `2.0` plus the agent radius on every side so the radius erosion never
eats the ground around the outermost geometry. An **empty scene** (no static geometry)
bakes over a default `±20` box, which holds no spans since there is nothing to stand on. A scene can instead **author** its bounds (Unity's nav
volume): they serialize in `nav_settings.bounds` and are editable from the scene
inspector's **Navmesh** section (editor↔API parity). Either way the bounds snap outward to
multiples of `GridSpacing`, so cell centres stay on a fixed world lattice. Anything outside
the bounds is unnavigable; the bake is O(cells), so very large bounds cost bake time.

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetBounds` | `()` | `minX, maxX, minZ, maxZ, authored` — the bounds the last bake used (snapped), and whether they were authored |
| `Navigation.SetBounds` | `(minX, maxX, minZ, maxZ)` | writes `nav_settings.bounds` + re-bakes; errors unless finite with `min < max` |
| `Navigation.ClearBounds` | `()` | clears the authored bounds (back to derived from geometry) + re-bakes |
