## `Random`

The seeded gameplay RNG — Unity's `UnityEngine.Random` subset. The sim is a pure
function of (seed, inputs, fixed dt), so this is the **only** randomness scripts
get: one per-World stream (SplitMix64), restarted from a fixed default seed every
time Play starts, never from the wall clock. Two headless runs of the same scenario
draw the same numbers.

| Function | Signature | Returns |
|---|---|---|
| `Random.Value` | `()` | float in `[0, 1)` |
| `Random.Range` | `(min, max)` — **two integers** → an integer in `[min, max)` (**max exclusive**, Unity's `Range(int, int)`; `min` when `max <= min`). **Any float argument** → a float in `[min, max)`. The overload follows Lua's number subtype: `Random.Range(1, 7)` rolls a d6, `Random.Range(1.0, 7)` is a float. | number |
| `Random.InsideUnitSphere` | `()` | `x, y, z` — uniform point inside the unit sphere |
| `Random.OnUnitSphere` | `()` | `x, y, z` — uniform unit-length direction |
| `Random.InsideUnitCircle` | `()` | `x, y` — uniform point inside the unit circle |
| `Random.SetSeed` | `(seed)` — restart the stream from integer `seed` (Unity's `Random.InitState`). Lasts until the next Play, which restarts from the default seed. | — |

### The Lua standard library in gameplay scripts

Gameplay scripts (and the console REPL, which evaluates in the same VM) get Lua
5.4's safe stdlib **minus the nondeterministic parts**:

- **`os` and `io` are not loaded** — `os` is `nil`, so `os.time()` / `os.clock()` /
  `os.date()` / `os.getenv()` fail loudly instead of leaking the wall clock or the
  machine into the sim. Use `Time` for clocks and `Storage` for persistence.
- **`math.random` / `math.randomseed` draw from `Random`**, keeping Lua's argument
  rules: `math.random()` → `Random.Value()`; `math.random(n)` → integer in `[1, n]`;
  `math.random(m, n)` → integer in `[m, n]` (**inclusive**, unlike `Random.Range`);
  `math.random(0)` → any integer. `math.randomseed(x)` is `Random.SetSeed(x)`
  (`0` when called with no argument — never a random seed).

Everything else (`string`, `table`, `math`, `utf8`, `coroutine`, `package`) is
Lua's own.
