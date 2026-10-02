## `OffMeshLink`

An authored connection between two walkable points that walking can't join: Unity's
`OffMeshLink` / `NavMeshLink` (#462). A ladder between two floors, a window to vault, a
gap to jump, a rope down a cliff. Paths route across it like any other edge, and an
agent that reaches it hands control to its script (see `NavMeshAgent`, *Off-mesh links*).

`Start` and `End` are points **local to the entity**: the link moves, turns and scales
with its `Transform`, so place a ladder by moving its GameObject. The start defaults to
the Transform itself and the end to 2 m ahead. Each end snaps to the walkable point
nearest it within **1 m**; a link with an end that finds none connects nothing until the
link or the navmesh moves (`IsConnected` says which). A link is two-way by default; a
one-way link (a drop through a hatch) is crossed start to end only.

`Cost` is the path cost of crossing it in world units (Unity's `costOverride`): negative
(the default, `-1`) uses the link's length. A higher cost makes paths prefer walking when
walking is not much longer; the search never counts a link as cheaper than the straight
ground distance it covers.

A link is a navmesh input: in Play, an added, moved, edited or toggled link reaches the
navmesh on the next tick, without rebaking any floor (see `Navigation`, *Keeping the
navmesh current*). The navmesh also **generates** drop and jump links on its own along
ledges when the scene asks for them (see `Navigation`, *Off-mesh links*); those are not
components.

Getters return a neutral default (`false`, zeros, `-1`) without a link; setters are then
no-ops. Add one with `Scene.AddComponent(id, "OffMeshLink")` (alias `NavMeshLink`).
`Debug.Snapshot` reports it as `offmesh_link`.

| Function | Signature | Returns |
|---|---|---|
| `OffMeshLink.GetStart` / `SetStart` | `(id)` / `(id, x, y, z)` | the start point, local to the entity (non-finite is ignored) |
| `OffMeshLink.GetEnd` / `SetEnd` | `(id)` / `(id, x, y, z)` | the end point, local to the entity (non-finite is ignored) |
| `OffMeshLink.GetBidirectional` / `SetBidirectional` | `(id)` / `(id, bool)` | whether agents may cross it end to start too |
| `OffMeshLink.GetCost` / `SetCost` | `(id)` / `(id, cost)` | the crossing cost in world units; negative means the link's length (stored as `-1`) |
| `OffMeshLink.GetActive` / `SetActive` | `(id)` / `(id, bool)` | Unity's `activated`: an inactive link connects nothing |
| `OffMeshLink.IsConnected` | `(id)` | whether both ends found the navmesh at the last rebake |
