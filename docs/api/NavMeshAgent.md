## `NavMeshAgent`

Per-entity navmesh agent control.

| Function | Signature | Returns |
|---|---|---|
| `NavMeshAgent.SetTarget` | `(id, x, y, z)` | — |
| `NavMeshAgent.GetTarget` | `(id)` | `x, y, z` |
| `NavMeshAgent.SetSpeed` | `(id, speed)` | — |
| `NavMeshAgent.SetAcceleration` | `(id, acceleration)` | — |
| `NavMeshAgent.SetStoppingDistance` | `(id, distance)` | — |
| `NavMeshAgent.SetRadius` | `(id, radius)` | — |
| `NavMeshAgent.IsAtTarget` | `(id)` | `bool` |
| `NavMeshAgent.GetVelocity` | `(id)` | `x, y, z` |
| `NavMeshAgent.SetActive` | `(id, active)` | — |
| `NavMeshAgent.SetAvoidancePriority` | `(id, priority)` | — (0–99, clamped; lower = more important) |
| `NavMeshAgent.SetAvoidanceEnabled` | `(id, enabled)` | — |
| `NavMeshAgent.HasPath` | `(id)` | `bool` — the agent holds a planned path |
| `NavMeshAgent.RemainingDistance` | `(id)` | distance left along the path from where it stands; `math.huge` with no path |
| `NavMeshAgent.GetPathStatus` | `(id)` | `"complete"`, `"partial"` or `"invalid"` (no path planned) |
| `NavMeshAgent.GetPath` | `(id)` | a path table (see `Navigation`): the agent's position, then the corners still ahead |
| `NavMeshAgent.Warp` | `(id, x, y, z)` | `bool` — teleports onto the navmesh point nearest `x, y, z` (within 1 unit); `false` and no move when there is none |
| `NavMeshAgent.ResetPath` | `(id)` | — stops following: the target becomes where it stands |

### Following a path (#458)

An active agent plans a **smoothed path** to its target (the same one
`Navigation.CalculatePath` returns) and steers corner to corner, so it cuts straight
across open ground instead of zig-zagging over grid cells. It keeps that path, and only
plans again when the target moves, the navmesh is rebaked, the path ages out (60 fixed
frames), or a corner ahead stops being walkable. When the target is unreachable the path
is **partial** and the agent stops at its end, the nearest reachable point, instead of
pressing into the wall. `Warp` drops the old path and stops the agent dead; it plans once
from the new position on its next tick. `ResetPath` is Unity's: the agent slows to a stop
where it is.

### Local avoidance (#463)

Moving agents steer around each other with **ORCA** (optimal reciprocal collision
avoidance, van den Berg et al. 2011): each frame an agent's preferred velocity toward its
next waypoint is bent just enough to stay clear of its 10 nearest agents (within 10 world
units) for the next 2 seconds, then the usual slide applies: each axis moves only onto a span linked to the agent's own (within the step, slope and headroom limits), and the agent stands on the floor it walked onto (#454). Agents keep
their `Radius` apart. **`AvoidancePriority`** is Unity's `avoidancePriority` (default
`50`): an agent ignores agents with a *higher* number (they yield to it), splits the dodge
with equal numbers, and yields fully to lower numbers. An agent with avoidance **off**
(default on) never dodges, but the others still steer around it — as they do around an
agent that has stopped at its target. Both settings serialize with the scene and are
editable on the inspector's NavMesh Agent card. Deterministic: the result depends only on
the agents' state, never on timing.
