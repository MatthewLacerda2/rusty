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
| `NavMeshAgent.SetBaseOffset` / `GetBaseOffset` | `(id, offset)` / `(id)` | how far the entity's origin sits above the agent's feet (Unity's `baseOffset`, default `0`) |
| `NavMeshAgent.IsAtTarget` | `(id)` | `bool` — its feet are within `StoppingDistance` of where its path ends (the target projected onto the navmesh) |
| `NavMeshAgent.GetVelocity` | `(id)` | `x, y, z` |
| `NavMeshAgent.SetActive` | `(id, active)` | — |
| `NavMeshAgent.SetAvoidancePriority` | `(id, priority)` | — (0–99, clamped; lower = more important) |
| `NavMeshAgent.SetAvoidanceEnabled` | `(id, enabled)` | — |
| `NavMeshAgent.HasPath` | `(id)` | `bool` — the agent holds a planned path |
| `NavMeshAgent.RemainingDistance` | `(id)` | distance left along the path from where it stands; `math.huge` with no path |
| `NavMeshAgent.GetPathStatus` | `(id)` | `"complete"`, `"partial"` or `"invalid"` (no path planned) |
| `NavMeshAgent.GetPath` | `(id)` | a path table (see `Navigation`): the agent's position, then the corners still ahead |
| `NavMeshAgent.Warp` | `(id, x, y, z)` | `bool` — teleports its feet onto the navmesh point nearest `x, y, z` (within 1 unit); `false` and no move when there is none |
| `NavMeshAgent.ResetPath` | `(id)` | — stops following: the target becomes where it stands |
| `NavMeshAgent.IsOnOffMeshLink` | `(id)` | `bool` — the agent is on an off-mesh link (see below) |
| `NavMeshAgent.GetCurrentOffMeshLink` | `(id)` | the link it is on, as a link table (see `Navigation`), or `nil` |
| `NavMeshAgent.CompleteOffMeshLink` | `(id)` | `bool` — puts the agent on the link's end and resumes its path; `false` when it is on no link |
| `NavMeshAgent.GetAutoTraverseOffMeshLink` / `SetAutoTraverseOffMeshLink` | `(id)` / `(id, bool)` | whether the engine crosses links for it (default `true`) |
| `NavMeshAgent.GetAreaMask` / `SetAreaMask` | `(id)` / `(id, mask)` | the areas it may enter (Unity's `areaMask`): bit `i` allows area `i`; `-1` (the default) is every area. A new mask re-plans on the next tick |

### Areas (#460)

The agent's path is planned with its **area mask** and the scene's area **costs** (see
`Navigation`, *Areas and costs*): it never steps onto a floor or crosses a link whose area
the mask excludes, and it prefers cheap areas, crossing a costly one only when that is
still the cheapest way. A target in an excluded area gives a `partial` path. A runtime
cost change (`Navigation.SetAreaCost`) makes every agent re-plan on its next tick. To
make a bot avoid a zone entirely: `NavMeshAgent.SetAreaMask(id, -1 ~ (1 << area))`.

### Following a path (#458)

An active agent plans a **smoothed path** to its target (the same one
`Navigation.CalculatePath` returns) and steers corner to corner, so it cuts straight
across open ground instead of zig-zagging over grid cells. It keeps that path, and only
plans again when the target moves, a rebake changes cells its remaining path crosses
(#456; a rebake elsewhere leaves it be, and a partial path re-plans on any rebake, since
the way may have opened), the path ages out (60 fixed frames), or a corner ahead stops
being walkable. When the target is unreachable the path
is **partial** and the agent stops at its end, the nearest reachable point, instead of
pressing into the wall.

The path never ends off the navmesh (#666): the target is first projected onto the
walkable point nearest it, within 8 units, and the agent walks there and stops. A target
past the floor's edge is chased to the edge; a target above the floor (a flying player, a
chest-height point) is reached on the floor under it. A target with no navmesh within 8
units plans no path (`"invalid"`), and the agent stays where it is. Arrival is measured
from the agent's **feet** — its Transform minus `BaseOffset` — to that projected end, so
an agent whose body is centred on its origin sets `BaseOffset` to half its height and
stands on the floor instead of sinking into it. `Warp` drops the old path and stops the agent dead; it plans once
from the new position on its next tick. `ResetPath` is Unity's: the agent slows to a stop
where it is.

### Off-mesh links (#462)

A path can cross **off-mesh links**: authored `OffMeshLink`s (a ladder) and the drops
and jumps the navmesh generates (see `Navigation`). Such a leg is one straight corner to
corner, from the link's start to its end. When the agent reaches a link's start it stops
steering and is **on the link** (`IsOnOffMeshLink`; Unity's `isOnOffMeshLink`) — the
`Walking → OnLink → Walking` state machine:

- **Auto-traverse on** (the default, Unity's `autoTraverseOffMeshLink`): the engine moves
  it in a straight line to the link's end at its `Speed`, then it walks on.
- **Off**: it waits on the link, as long as it takes. The script reads
  `GetCurrentOffMeshLink` (`type` says `"drop"`, `"jump"` or `"manual"`; `owner` is the
  authoring entity, for a ladder's animation), plays the vault, jump or climb, moves the
  body, and calls `CompleteOffMeshLink`, which puts the agent on the link's end.

On a link the agent neither re-plans nor takes part in local avoidance. `Warp` takes it
off the link. It is poll-only: there is no callback. A one-way link (every generated one)
is never crossed backwards, so a drop is never climbed back up.

```lua
function Update()
  if NavMeshAgent.IsOnOffMeshLink(self.id) then
    local link = NavMeshAgent.GetCurrentOffMeshLink(self.id)
    -- play the climb for link.type, move toward link.endPos, then:
    NavMeshAgent.CompleteOffMeshLink(self.id)
  end
end
```

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
