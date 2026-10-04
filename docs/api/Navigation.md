## `Navigation`

The navmesh is a **layered grid** (#454, the compact-heightfield half of Recast): every
XZ cell holds a list of walkable **spans**, each a floor height plus the open space above
it. Stacked floors, a catwalk over a corridor and a bridge over a road are all walkable at
once, and paths climb stairs and ramps from one floor to the next in real `y`.

The bake reads the **real geometry** of every active, static, non-trigger collider: the
same shapes physics collides with (rotated boxes, ramps, spheres, capsules, convex hulls
and triangle meshes), rasterised into each cell column. A ramp bakes as a slope, not a
plateau at its highest point: each span also keeps the plane of the surface across its
cell, so a `NavMeshAgent` on a ramp stands on the slope under its feet and climbs it
smoothly, not in one-cell steps (#781). **Where there is no collider there is no navmesh**: no
implicit ground past the level's edge, and an empty scene bakes nothing (as in Unity).
Carving `NavMeshObstacle`s cut their footprint out of it (see `NavMeshObstacle`),
`NavMeshModifierVolume`s give parts of it a navigation area with its own cost (below), and
off-mesh links join what walking can't (below).

### Keeping the navmesh current (#456)

In Play the navmesh follows the scene **every tick**, rebaking **only what changed**. Each
tick compares the static colliders (shape and world pose; added, removed, moved, resized,
toggled static or active), the carving obstacles, the area modifier volumes and the
authored off-mesh links against
what the last bake read; a link change re-snaps the links without rebaking any floor. A
change dirties the cells its old and new footprints cover; only those cells, plus the
agent-radius erosion's reach around them, are rebaked. The result is exactly the navmesh
a full bake of the scene would give, and an agent re-plans only when its path crosses a
rebaked cell. A tick where nothing changed costs one pass over the colliders and
obstacles, so geometry can move every frame (a door swinging shut) and agents see it the
same tick. Play's first frame bakes in full; so does a change of the bake settings or of
the grid's bounds.

Every query resolves the start and the target to spans with one rule: **the nearest
span below the point, favouring the floor under a character's feet** — the highest floor
at most `MaxStep` above the point, else the lowest floor above it. A chest-height target
therefore means the floor under it, not the one overhead. A point over a column with no
span (a wall, a hole) uses the nearest column with one, up to 5 cells away.

### Paths and queries (#458)

Paths are **smoothed**: the A* search over spans is string-pulled into **corners**, so a
path across an open room is one straight run (two corners: start and end) and a path
round a wall turns only where the wall is in the way. A straight leg only ever crosses
walkable spans, so the smoothed path keeps the agent-radius clearance the bake eroded
off every wall. A path is a table:

```lua
{ status = "complete" | "partial" | "invalid",
  corners = { {x=, y=, z=}, ... },   -- start first, end last; y is the floor height
  length = n }                       -- world units along the corners
```

- **`complete`** — the path reaches the target. Its ends are the query points themselves
  (on their floors) when they stand over the navmesh.
- **`partial`** — the target is unreachable (another island, a sealed room); the path ends
  on the reachable point nearest to it, as Unity's `PathPartial`.
- **`invalid`** — the start or the target has no navmesh within 5 cells; no corners.

`SamplePosition` and `Raycast` answer on the navmesh, not physics: *walkable*, not
*solid*. Every query is synchronous and deterministic.

| Function | Signature | Returns |
|---|---|---|
| `Navigation.GetNextPathStep` | `(cx, cy, cz, tx, ty, tz)` | the next corner `x, y, z` of the smoothed path (its `y` is that floor's height); with no path, the target itself |
| `Navigation.CalculatePath` | `(fx, fy, fz, tx, ty, tz [, areaMask])` | a path table (above); with `areaMask`, the path enters only those areas (see *Areas and costs*) |
| `Navigation.GetPathLength` | `(path)` | the length along a path table's `corners` (the same number as its `length`) |
| `Navigation.SamplePosition` | `(x, y, z, maxDistance [, areaMask])` | `found, x, y, z` — the walkable point nearest the given one (3D distance) within `maxDistance`, on a floor whose area `areaMask` allows; `false, 0, 0, 0` when there is none |
| `Navigation.Raycast` | `(fx, fy, fz, tx, ty, tz [, areaMask])` | `hit, x, y, z` — walks the navmesh straight from the start toward the end; `hit` is `true` when a wall, ledge, eroded edge, too-steep step or a floor outside `areaMask` stops it, and `x, y, z` is where it stopped (the end point when clear), on the floor it reached |

`Raycast` follows ramps and stairs, so a clear walk can end on another floor; the
returned `y` says which. Its start must stand on the navmesh: from off it the walk is
blocked at once, at the start (Unity's rule). `SamplePosition` with `maxDistance` of a
few metres is the usual way to turn a picked point (a callout, a click) into one an
agent can reach.

### Off-mesh links (#462)

**Off-mesh links** join walkable spans that walking can't: a drop off a ledge, a jump onto
a box or across a gap, a ladder. A path can cross them (see `NavMeshAgent`, *Off-mesh
links*, for what an agent does on one); a crossed link is one leg of the path, and the
smoothing never pulls a straight run across a link. Two sources:

- **Authored** `OffMeshLink` components (see `OffMeshLink`).
- **Generated** at bake time along every **ledge** (a span with a cardinal neighbour it
  can't walk to), looking straight out for the nearest floor it could land on. Each kind
  is off until its setting is above `0` (Unity's "Drop Height" and "Jump Distance"):
  - a **drop** (`DropHeight`): a floor more than `MaxStep` and at most `DropHeight` lower,
    just past the edge (within the agent-radius margins both sides of it, plus two cells);
  - a **jump**: up onto a floor at most `JumpHeight` higher in that same reach, or across
    a gap at most `JumpDistance` wide beyond it, landing at most `DropHeight` lower. A
    level jump needs a real gap on the way, so floor merely trimmed around a wall end is
    never jumped.

  Both ends need `AgentHeight` of room above the higher floor, and every column crossed
  must be open there; walls stop the scan. Generated links are **one-way** (a drop is
  never climbed back up; the far side generates its own jump back when one fits), and are
  thinned to one per `LinkSpacing` (default `2`) along an edge. The incremental rebake
  regenerates the links near a change, matching a full bake exactly.

`GetOffMeshLinks` lists every link in the navmesh (the generated ones by source cell, then
the authored ones by entity id) — the way to see where agents can drop and jump. A link
table is:

```lua
{ type = "manual" | "drop" | "jump",
  startPos = {x=, y=, z=}, endPos = {x=, y=, z=},  -- snapped onto the navmesh
  owner = id | nil,                                -- the OffMeshLink's entity
  bidirectional = bool }                           -- GetOffMeshLinks only
```

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetOffMeshLinks` | `()` | a list of link tables, every link in the navmesh |
| `Navigation.GetDropHeight` / `SetDropHeight` | `()` / `(metres)` | the highest generated drop (`0`: none) + re-bakes |
| `Navigation.GetJumpDistance` / `SetJumpDistance` | `()` / `(metres)` | the widest gap a generated jump crosses (`0`: none) + re-bakes |
| `Navigation.GetJumpHeight` / `SetJumpHeight` | `()` / `(metres)` | the highest ledge a generated jump climbs (`0`: none) + re-bakes |
| `Navigation.GetLinkSpacing` / `SetLinkSpacing` | `()` / `(metres)` | one generated link per this much edge + re-bakes |

### Areas and costs (#460)

Every walkable floor and every off-mesh link has a **navigation area**, an id `0`–`31`
(Unity's NavMesh areas). The scene's **area table** names each area and gives it a
**cost**: a path pays `distance × cost` across it, so agents prefer cheap areas and cross
a costly one only when nothing cheaper gets there — "same navmesh, different costs". An
agent's **area mask** (`NavMeshAgent.SetAreaMask`, and the optional `areaMask` on the
queries above; bit `i` = area `i`, `-1` = every area) says which areas it may enter at
all. Two areas are built in: **`Walkable`** (`0`, every floor's area by default) and
**`NotWalkable`** (`1`, which removes the floor a modifier volume covers). Areas are
assigned by `NavMeshModifierVolume` boxes at bake time, and by `OffMeshLink.SetArea`
(generated links are `Walkable`).

- Costs are at least `1` (lower values clamp up), so the search stays optimal.
- A step between floors of two areas pays the mean of their costs; a diagonal step pays at
  least the dearer of the two cells it cuts past, so a path never shaves the corner of an
  area it routes around.
- The smoothing never pulls a straight leg across a floor the mask excludes or one dearer
  than the stretch it smooths: a path that went around the mud stays out of it. Paths get
  a corner where the cost changes.
- A query's start floor is exempt from the mask, so an agent standing in a forbidden area
  can still walk out; a target in one gives a `partial` path.
- **Costs are read at search time, never baked.** `SetAreaCost` takes effect on the very
  next query, rebakes nothing, and makes every agent re-plan on its next tick (a cost
  change anywhere may change any path). Area *assignments* (moving a volume) rebake only
  the cells involved.

The table serializes with the scene (`nav_settings.areas`) and is editable from the scene
inspector's **Navmesh** section (editor↔API parity).

| Function | Signature | Returns / Effect |
|---|---|---|
| `Navigation.GetAreaFromName` | `(name)` | the area's id, or `-1` when no area has that name (case-sensitive) |
| `Navigation.DefineArea` | `(name [, cost])` | the id of a new area (cost default `1`), or of the existing one re-costed; errors on an empty name, a non-finite cost, or a full table (32 areas) |
| `Navigation.GetAreaCost` | `(name)` | the area's cost, or `nil` when undefined |
| `Navigation.SetAreaCost` | `(name, cost)` | sets the cost (clamped to at least `1`); no rebake, agents re-plan; errors on an unknown area or a non-finite cost |
| `Navigation.GetAreas` | `()` | a list of `{index =, name =, cost =}`, one per area in id order |

```lua
local fire = Navigation.DefineArea("Fire", 1)
NavMeshModifierVolume.SetArea(molotovZone, fire)   -- where the fire is
Navigation.SetAreaCost("Fire", 50)                 -- burning: avoid unless trapped
NavMeshAgent.SetAreaMask(bot, -1 ~ (1 << fire))    -- this bot never walks into it
```

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
