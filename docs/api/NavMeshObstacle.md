## `NavMeshObstacle`

Something navigation agents keep out of: Unity's `NavMeshObstacle` (#456). The obstacle
is a **box** (`size`, rotated and scaled with the `Transform`) or an upright **capsule**
(`radius` scaled by the larger of the `x`/`z` scale, `height` by `y`), around `center` in
the entity's local space. A closed door, a parked car, a pushed crate.

**Carving.** With `Carving` on, the obstacle **cuts its footprint out of the navmesh**:
every walkable span whose standing room (floor up to the agent height) the obstacle's
volume reaches is removed, and the agent-radius erosion then pulls the walkable surface
back around the hole like around any wall. Paths route around it; a door that fills a
doorway closes it. Only the floors the obstacle stands in are cut: a crate on the ground
leaves the deck above it alone. Removing the obstacle (or turning carving off) re-opens
the navmesh. Only the cells around the obstacle are rebaked (see `Navigation`, *Keeping
the navmesh current*).

**Carve only stationary** (on by default, as in Unity) carves only once the obstacle has
stood still for `TimeToStationary` seconds (default `0.5`). It counts as moving while any
point of its shape is more than `MoveThreshold` (default `0.1` m) from where it last
carved; a sliding crate cuts no holes until it comes to rest. With it off, a carving
obstacle re-carves at once whenever it moves past the threshold. An obstacle the play
tick has not seen yet (edit mode, the first Play frame) has not moved, so it carves where
it stands.

**Not carving** (carving off, or moving under carve-only-stationary), the obstacle is a
**moving obstacle for local avoidance**: agents steer around a disc holding its
footprint, predicting its velocity, and it never dodges back. `IsCarving` says which it
is right now. Avoidance is local: an agent walking dead at a still, non-carving obstacle
has no side to prefer and can stall against it, as in Unity. An obstacle that stays put
should carve.

Getters return a neutral default (`false`, zeros, `nil` shape) without an obstacle;
setters are then no-ops. Add one with `Scene.AddComponent(id, "NavMeshObstacle")`.
`Debug.Snapshot` reports it as `nav_obstacle`.

| Function | Signature | Returns |
|---|---|---|
| `NavMeshObstacle.GetShape` / `SetShape` | `(id)` / `(id, "Box" \| "Capsule")` | the shape (case-insensitive; an unknown name is an error) |
| `NavMeshObstacle.GetCenter` / `SetCenter` | `(id)` / `(id, x, y, z)` | the shape's centre, local to the entity |
| `NavMeshObstacle.GetSize` / `SetSize` | `(id)` / `(id, x, y, z)` | the box's full size (each side > 0) |
| `NavMeshObstacle.GetRadius` / `SetRadius` | `(id)` / `(id, metres)` | the capsule's radius (> 0) |
| `NavMeshObstacle.GetHeight` / `SetHeight` | `(id)` / `(id, metres)` | the capsule's full height (> 0) |
| `NavMeshObstacle.GetActive` / `SetActive` | `(id)` / `(id, bool)` | Unity's `enabled`: inactive neither carves nor is avoided |
| `NavMeshObstacle.GetCarving` / `SetCarving` | `(id)` / `(id, bool)` | whether it cuts the navmesh |
| `NavMeshObstacle.GetCarveOnlyStationary` / `SetCarveOnlyStationary` | `(id)` / `(id, bool)` | carve only after standing still |
| `NavMeshObstacle.GetMoveThreshold` / `SetMoveThreshold` | `(id)` / `(id, metres)` | how far it moves before it counts as moving (≥ 0) |
| `NavMeshObstacle.GetTimeToStationary` / `SetTimeToStationary` | `(id)` / `(id, seconds)` | how long still before it carves (≥ 0) |
| `NavMeshObstacle.IsCarving` | `(id)` | whether it cuts the navmesh right now |
| `NavMeshObstacle.GetVelocity` | `(id)` | `x, y, z`: its world velocity over the last play tick |
