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

### Local avoidance (#463)

Moving agents steer around each other with **ORCA** (optimal reciprocal collision
avoidance, van den Berg et al. 2011): each frame an agent's preferred velocity toward its
next waypoint is bent just enough to stay clear of its 10 nearest agents (within 10 world
units) for the next 2 seconds, then the usual walkable-cell slide applies. Agents keep
their `Radius` apart. **`AvoidancePriority`** is Unity's `avoidancePriority` (default
`50`): an agent ignores agents with a *higher* number (they yield to it), splits the dodge
with equal numbers, and yields fully to lower numbers. An agent with avoidance **off**
(default on) never dodges, but the others still steer around it — as they do around an
agent that has stopped at its target. Both settings serialize with the scene and are
editable on the inspector's NavMesh Agent card. Deterministic: the result depends only on
the agents' state, never on timing.
