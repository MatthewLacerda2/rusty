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

The query runs **synchronously**: it answers in the call. Nothing surviving is
not an error. A spec is a plain Lua table, so a script can keep one as a
module-level constant or build one on the fly.

### Contexts

A spec names places by **context**. `querier` is always the caller (its feet,
when it is a `NavMeshAgent`). The `contexts` table adds the rest, each by name:

- an **entity id**: where it stands, and its facing (its world +Z). A
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
  the one its table asks for (`visible`, `inside`, `exists`).
- `purpose = "score"` scores: `score = { curve, weight }` (defaults `"linear"`,
  1). The raw values are **normalized to 0..1 over the items still standing**,
  shaped by the curve (`linear` n, `inverse` 1 − n, `square` n², `sqrt` √n) and
  multiplied by `weight`; an item's score is the sum over the tests. When every
  item has the same raw value, the test gives each its full weight.
- `purpose = "filter_score"` does both. Left out, a test with `min` / `max`
  filters, a test with `score` scores, and a yes/no test with neither filters.

| Test | Fields | Raw value |
|---|---|---|
| `distance` | `to` (context, default `"querier"`), `mode` (`"3d"` or `"2d"`) | metres from `to` to the item |
| `dot` | `from` (an entity context, default `"querier"`), `mode` | cosine between `from`'s facing and the way to the item: 1 ahead, −1 behind. "Behind the player" is `max = -0.5`. |
| `trace` | `from` (default `"target"`), `visible` (true), `from_height` (1.5), `item_height` (1.5), `mask` (layer mask; every layer) | 1 when the line from `from`'s eye to the item's is clear, else 0. `from`, the querier and their hierarchies never block it, nor do triggers. **Cover** is `visible = false`. Before Play builds a physics world, nothing blocks. |
| `view` | `inside` (true), `margin` (degrees, 0), `height` (1.5) | 1 when the item is inside the active camera's view (the `Camera` namespace's), its frustum widened by `margin` on each side, else 0. A cone test, no rays. A **flank** spot is `inside = false`. |
| `path` | `"exists"` or `"length"` (a string, not a table) | `exists`: 1 when the querier can walk to the item. `length`: its travel cost along the navmesh, metres (area costs weighed in); an item it cannot reach fails the test and is dropped. |
| `separation` | `radius` (5) | metres from the item to the nearest **other** active agent's `NavMeshAgent` target or position, capped at `radius`. An agent's target already is its claim on a spot, so a group running one query spreads out with nothing to release when one dies. |
| `near_points` | `points` (a context name or a list) | metres from the item to the nearest of the points |

**Order.** As in Unreal, the cheap tests run first whatever order the spec
writes them in: `distance`, `dot`, `view`, `separation` and `near_points`, then
`trace` (a ray per item), then `path` (one bounded navmesh flood per query), so
the dear tests only see the survivors of the cheap filters.

**Mistakes** are Lua errors naming the field: an unknown context
(`tests[2].distance.to: unknown context 'targte'`), an unknown field, a missing
`outer`, a filter on a number with no `min` or `max`.

### Picking and determinism

`best` takes the highest score; a tie goes to the earliest-generated item. The
random modes take a uniform pick among the items scoring within the top 5% or
25% of the score range (Unreal's `RandomBest5Pct` / `RandomBest25Pct`), drawn
from [`Random`](Random.md): **exactly one draw** per query that found something,
and **none** when nothing survives or the mode is `best`. Items are generated in
a fixed order, so the same seed, scene and spec always pick the same point.
