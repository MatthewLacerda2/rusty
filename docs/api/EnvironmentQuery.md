## `EnvironmentQuery`

"Where should I stand?" for an agent (#1892): Unreal's **Environment Query
System** (EQS), since Unity has nothing built in. A query **generates**
candidate points around a context, **projects** each onto the navmesh, runs
**tests** that filter and score them, and picks one. Flanking, taking cover,
holding an angle, keeping a distance and surrounding the player all come down to
it; `NavMeshAgent.SetTarget` then walks there.

```lua with=self_id,player_id
local spot = {
  generator = { donut = { context = "target", inner = 4, outer = 10, rings = 2, per_ring = 16 } },
  tests = {
    { path = "exists", purpose = "filter" },
    { distance = { to = "target" }, score = { curve = "inverse", weight = 0.5 } },  -- closer is better
    { trace = { from = "target", visible = true }, purpose = "filter" },            -- can shoot from there
    { separation = { radius = 3 }, score = { weight = 1 } },                        -- don't stack on a mate
  },
}
local found, x, y, z = EnvironmentQuery.Run(self_id, spot, { target = player_id })
if found then NavMeshAgent.SetTarget(self_id, x, y, z) end
```

| Function | Signature | Returns |
|---|---|---|
| `EnvironmentQuery.Run` | `(querier, spec [, contexts [, mode]])` — `mode` is `"best"` (default), `"random_best_5"` or `"random_best_25"` | `found, x, y, z, score`; `false, 0, 0, 0, 0` when no item survives |
| `EnvironmentQuery.RunAll` | `(querier, spec [, contexts])` | every surviving item, best first: an array of `{ x, y, z, score }` (empty when none survives) |
| `EnvironmentQuery.Request` | `(querier, spec [, contexts [, mode]])`, as `Run` | a ticket (an integer) for the query, answered on a later tick: see *Time-sliced queries* |
| `EnvironmentQuery.Poll` | `(ticket)` | `"pending"` while it waits; `"done", found, x, y, z, score` once answered (`Run`'s returns), read once; `"gone"` for a ticket read, cancelled, dropped or never issued |
| `EnvironmentQuery.Cancel` | `(ticket)` | `true` when it dropped a waiting or unread request, `false` otherwise |
| `EnvironmentQuery.SetIterationsPerFrame` / `GetIterationsPerFrame` | `(n)` / `()` | the navmesh node expansions the time-sliced queries share each tick (Unreal's EQS `MaxAllowedTestingTime`, counted in expansions; at least `1`, default `2048`). It goes back to the default when Play starts and on Stop |

`Run` and `RunAll` run **synchronously**: they answer in the call. Nothing
surviving is not an error. A spec is a plain Lua table, so a script can keep one as a
module-level constant or build one on the fly.

### Contexts

A spec names places by **context**. `querier` is always the caller. The
`contexts` table adds the rest, each by name:

- an **entity id**: where it **stands**, and its facing (its world +Z). Where it
  stands is its feet, not its Transform (a character's Transform is its middle):
  a `NavMeshAgent`'s feet, else the bottom of its `CharacterController`'s
  capsule, else the bottom of its collider, else its Transform. The querier
  resolves the same way. A
  destroyed or inactive entity is a Lua error naming the field
  (`contexts.target: entity 42 is destroyed or inactive`), as is a `nil`
  querier;
- a **point** `{x, y, z}` (or `{x = .., y = .., z = ..}`): no facing;
- a **list of points** `{{x, y, z}, ...}`: for the `points` generator and the
  `near_points` test.

### Generators

`generator` holds exactly one of these, plus an optional `project` (default 2 m):
how far a point may be from the walkable floor it is moved onto. Points with no
floor that near are dropped (Unreal's *Project*). A generator may make at most
**512** points; more is a Lua error.

| Generator | Fields | Points |
|---|---|---|
| `donut` | `context` (default `"querier"`), `inner` (0), `outer` (required), `rings` (1), `per_ring` (8) | `rings` rings from `inner` to `outer` (one ring sits midway), `per_ring` points on each, the first at +X, turning toward +Z. Unreal's *Donut* / *OnCircle*. |
| `grid` | `context` (`"querier"`), `half_size` (required), `spacing` (1) | a square grid over `±half_size`, row by row from -X -Z. Unreal's *SimpleGrid*. |
| `points` | a context name, or a list of points | the given points, in order |

### Tests

Each entry of `tests` holds exactly one test kind, which measures a **raw value**
per item, and says what to do with it:

- `purpose = "filter"` drops items: a number test keeps raw values within `min` /
  `max` (either may be left out); a yes/no test keeps the items whose answer is
  the one its table asks for (`visible`, `inside`, `exists`). A yes/no test
  takes no `min` / `max`.
- `purpose = "score"` scores: `score = { curve, weight }` (defaults `"linear"`,
  1). The raw values are **normalized to 0..1 over the items still standing**,
  shaped by the curve (`linear` n, `inverse` 1 − n, `square` n², `sqrt` √n) and
  multiplied by `weight`; an item's score is the sum over the tests. When every
  item has the same raw value, the curve is skipped and the test gives each its
  full weight. A yes/no test's raw value is 1 for the answer its table asks for
  and 0 otherwise, so a scored `trace { visible = false }` favours cover and a
  scored `view { inside = false }` favours spots out of view.
- `purpose = "filter_score"` does both. Left out, a test with `min` / `max`
  filters, a test with `score` scores, and a yes/no test with neither filters.

| Test | Fields | Raw value |
|---|---|---|
| `distance` | `to` (context, default `"querier"`), `mode` (`"3d"` or `"2d"`) | metres from `to` to the item |
| `dot` | `from` (an entity context, default `"querier"`), `mode` | cosine between `from`'s facing and the way to the item: 1 ahead, −1 behind. "Behind the player" is `max = -0.5`. |
| `trace` | `from` (default `"target"`), `visible` (true), `from_height` (1.5), `item_height` (1.5), `mask` (layer mask; every layer) | 1 when whether the line from `from`'s eye (`from_height` above where it stands) to the item's is clear matches `visible`, else 0. `from`, the querier and their descendants (a character's hitboxes) never block it, nor do triggers; a parent they share with the level (an empty "Level" group) does not hide its walls. **Cover** is `visible = false`. Before Play builds a physics world, nothing blocks. |
| `view` | `inside` (true), `margin` (degrees, 0), `height` (1.5) | 1 when whether the item is inside the active camera's view (the `Camera` namespace's), its frustum widened by `margin` on each side, matches `inside`, else 0. A cone test, no rays. A **flank** spot is `inside = false`. |
| `path` | `"exists"` or `"length"` (a string, not a table) | `exists`: 1 when the querier can walk to the item. `length`: its travel cost along the navmesh, metres (area costs weighed in); an item it cannot reach fails the test and is dropped. Both read one search out from the querier toward every item at once, **bounded**: it walks at most `max(2 × d, d + 4)` metres of travel cost, `d` the straight-line distance to the farthest item still standing. So `exists` means *reachable within that bound*: a spot behind a longer detour, or across costly areas, reads as unreachable. The search heads for the items (an A\* toward the nearest one not yet found), so it costs the corridor toward them, not a disc as wide as the bound, and a spot's length is exactly what a full flood would give it. A spot on navmesh no walk or link leads to from the querier's (a crate top, a roof or an upper floor with no way up, drops off it included, of any size) is found unreachable before the search starts. A spot cut off only by the querier's area mask still runs the search to its bound: that is the dear case, so filter far spots out with a cheap `distance` test before `path`. |
| `separation` | `radius` (5) | metres from the item to the nearest **other** active agent's `NavMeshAgent` target or position, capped at `radius`. An agent's target already is its claim on a spot, so a group running one query spreads out with nothing to release when one dies. |
| `near_points` | `points` (a context name or a list) | metres from the item to the nearest of the points |

**Order.** As in Unreal, the cheap tests run first whatever order the spec
writes them in: `distance`, `dot`, `view`, `separation` and `near_points`, then
`trace` (a ray per item), then `path` (one bounded navmesh search per query), so
the dear tests only see the survivors of the cheap filters.

**Mistakes** are Lua errors naming the field: an unknown context
(`tests[2].distance.to: unknown context 'targte'`), an unknown field, a missing
`outer`, a filter on a number with no `min` or `max`, `min` / `max` on a yes/no
test, and a `purpose` a field contradicts (`purpose = "score"` with `min` or
`max`, `purpose = "filter"` with a `score`).

### Time-sliced queries

A `path` test's search is the dear part of a query: toward a donut 20 m away on
a Dust2-sized map it expands several thousand navmesh nodes, and a few such
queries in one tick can cost milliseconds. `Request` runs the same query in
slices, as Unreal's EQS manager does:

1. **On the call**, the query is generated and every test but `path` is run
   against the world as it is then (its contexts, the target included, are
   read then too). A query with no `path` test is answered right there.
2. **Each tick, after every `Update` and timer**, the engine spends
   `GetIterationsPerFrame()` node expansions on the waiting queries' `path`
   searches, oldest request first (one tick's requests in the order they were
   made), resuming the search the last tick left unfinished.
3. **On the tick its search ends**, the query is scored and its item picked
   (a random mode draws from [`Random`](Random.md) then), and `Poll` hands the
   answer to the next `Update` that asks.

So a request made in an `Update` is answered by a later `Update`: the next one
when its search fits what is left of a tick's budget, a few ticks later for a
long one (a donut 20 m away takes about 7 000 expansions, under four ticks of
the default budget). The answer is exactly what `Run` would have given in the
`Update` that made the request, the `best` pick included; a random mode draws
on the tick it is answered, from the stream as it is then.

```lua with=self_id,player_id
local spot = { generator = { donut = { context = "target", outer = 8 } }, tests = { { path = "exists" } } }
if not ticket then
  ticket = EnvironmentQuery.Request(self_id, spot, { target = player_id })
else
  local state, found, x, y, z = EnvironmentQuery.Poll(ticket)
  if state ~= "pending" then
    ticket = nil                      -- answered (or gone): ask again next time
    if found then NavMeshAgent.SetTarget(self_id, x, y, z) end
  end
end
```

A request belongs to its Play run: Stop, a `Scene.Load` and destroying the
querier drop it, and its ticket then reads as `"gone"` (never an error). The
queue is served only in Play, and entering Play starts a fresh one, so a request
made in edit mode is never answered: its ticket reads `"gone"` once Play starts.
A rebake while a search is held carries the search onto the new navmesh when
the change lies away from everything it reached (a carving obstacle moving
elsewhere on the map), and starts it over otherwise; either way the answer is
the new navmesh's. `RunAll` has no time-sliced form.

### Picking and determinism

`best` takes the highest score; a tie goes to the earliest-generated item. The
random modes take a uniform pick among the items scoring within the top 5% or
25% of the score range (Unreal's `RandomBest5Pct` / `RandomBest25Pct`), drawn
from [`Random`](Random.md): **exactly one draw** per query that found something,
and **none** when nothing survives or the mode is `best`. Items are generated in
a fixed order, so the same seed, scene and spec always pick the same point. A
time-sliced query's budget is counted in node expansions, never in time, so the
same run answers every request on the same tick.
