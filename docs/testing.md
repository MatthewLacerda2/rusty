# Testing conventions

| Kind | Lives in | Run with |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` in the same file | `cargo nextest run` |
| Integration tests | `tests/*.rs`, one binary rooted at `tests/main.rs` | `cargo nextest run` (or `cargo nextest run -E 'binary(integration)' <filter>`) |
| Doctests | `///` examples | `cargo test --doc` (nextest does not run them) |
| Harness scenarios | a project's `scenarios/*.lua` (dev-only); the engine's own in `tests/fixtures/project/scenarios/` | the headless `play` binary, `--project <dir>` |

## What a scenario can call

A scenario gets **the whole API** in `docs/api/`, read-write, through the same
registration the gameplay VM uses (`api::register`, #1384) — a Unity editor test
can call any runtime API, and a scenario that must not write the world simply
doesn't — plus the scenario-only `Harness` table (step, assert, log, stats,
screenshots, benches; `dev::bridge`). `Input`'s injection verbs (`Press`,
`MoveMouse`, `SetAxis`, …) are the API's own, so a scenario plays as the user.

It runs in a Lua VM of its own, beside the gameplay scripts' (which Play rebuilds
on its first tick), and steps the world between calls, never inside one. Six
things follow:

- `Scene.GetScript(id, name)` answers a live **handle** on the gameplay instance,
  not the table itself: `pc.vy` reads the field now, `pc.marker = {…}` writes it,
  both by value, copied Lua to Lua: integer keys, mixed tables, integers vs
  floats and `math.huge`/NaN all keep their type, and no gameplay metamethod
  runs. A function field reads as a proxy that **calls** the game script's
  function in the gameplay VM, at the current tick (#1580): `pc:Refill(30)` (the
  instance is `self`) or `pc.Add(2, 3)`, with arguments and returns copied the
  same way (a function or userdata argument is an error; one returned is an
  error raised after the call ran, so its side effects stand), and a game script
  error raised as the scenario's, with its traceback. That is how a test reaches
  a state (refill, give a weapon, start wave N) without playing there. Each read is a **copy**, so assign a
  whole field through the handle: `pc.marker.hits = 5` changes the scenario's
  copy only, while `local m = pc.marker; m.hits = 5; pc.marker = m` writes it.
  Fields are read raw, so one the instance inherits through its metatable (a
  class's method or default) reads `nil`, and the handle has no `==`, `#` or
  `pairs`. It is `nil` before Play loads the
  script, and errors once that instance is gone, even when a new one (a restarted
  Play) has the same entity id and script name: look it up again.
- `Damage.Apply` / `ApplyPoint` / `ApplyRadial` run **in the gameplay VM**
  (#1893), arguments and returns copied the same way, because the `OnTakeDamage`
  handlers they call synchronously live there: a scenario's shot reaches the game
  script's handler, and a handler's error is raised as the scenario's.
- `CharacterController.Move` runs in the gameplay VM the same way (#1879), so
  the move's `OnControllerColliderHit` handlers hear it. Before the first
  `Harness.Step` there is no gameplay VM: the scenario's own `Move` runs, moving
  the edit-mode controller unobstructed and calling no handler.
- A Lua function a scenario passes `Timer.Invoke` / `InvokeRepeating` /
  `StartCoroutine` or a tween's `on_complete` is refused with an error: it belongs
  to the scenario's VM, which the gameplay tick never calls. Drive time with
  `Harness.Step`; an invoke by *name* (`Timer.Invoke(id, "Fire", 1)`) resolves on
  the gameplay scripts and works.
- The world enters Play on the first `Harness.Step`, not before: a scenario's
  opening lines run in edit mode, which is when `Harness.AttachPlayerBot` swaps
  the Player's script. Entering Play clears the timers and restarts `Random`, so
  `Timer.Invoke` / `InvokeRepeating` / `StartCoroutine`, `Tween.To` / `Sequence`
  and `Random.SetSeed` are refused with an error until then: call
  `Harness.Step(1)` first.
- The scenario VM is built from the gameplay VM's sandbox (#1408): no `os` or
  `io` (write results through `Harness.*`), and `math.random` draws from (and
  `math.randomseed` reseeds) the world's one seeded `Random`, so a scenario's
  draws move the game's stream as `Random.*` calls do. `require`, `dofile` and
  `loadfile` are the gameplay VM's too (#1717): `require("lib.game")` searches
  the project's `assets/scripts`, then the engine's bundled scripts, then
  `package.path`; a relative `dofile` path is taken from the project root; a
  file outside the project still loads but warns once in the console. So a game
  script's `require`s of its shared modules resolve in a scenario as in Play.
  Only the file access is shared: the scenario VM's `print` is Lua's own and
  writes to the `play` process's stdout, not to the console log a gameplay
  script's `print` reaches.
- A scenario is a test, so the paid provider verbs (`Sfx.Generate`, `Speech.*`)
  are refused for the whole run, before its first `Harness.Step` too (CLAUDE.md,
  *Provider calls are authoring, never gameplay*).

**Asserting a clean run** (#2054). `Harness.Expect(cond, message)` records a
pass or fail in `results.json`; `Harness.Warnings([after])` returns the run's
console warnings and errors so far, oldest first, as `{ level = "warning" |
"error", text }` tables, skipping the first `after` (default 0). It reads the
console `console.log` is written from, engine warnings included (they join it
after every tick, and when `Warnings` is called), and keeps every one of the run,
past the 100 lines the console buffer holds; a repeat is one line, `text (×N)`, as
the console folds it. So a scenario ends with

```lua
local w = Harness.Warnings()
Harness.Expect(#w == 0, "no warnings: " .. (w[1] and w[1].text or "none"))
```

and one that provokes a warning on purpose counts first and reads past it
(`local n = #Harness.Warnings()` … `Harness.Warnings(n)`). Nothing fails a run on
a warning by itself: the scenario decides. `games/smoke` holds it.

**Hearing a run** (#1579). `Harness.RecordAudio([path])` starts recording the
game's final audio mix to a 48 kHz float WAV (`audio.wav` by default; a relative
`path` is under the run's output folder) and returns its path; `Harness.StopAudio()`
closes it and returns the path again, and `play --record-audio <path>` records the
whole run. The harness mixes offline, one fixed tick at a time, so the capture
needs no sound card and two runs of one scenario write byte-identical files; a
capture still running when the scenario ends is closed with the results. A run
started with `--record-audio` is already capturing, so the scenario's own
`Harness.RecordAudio()` errors (one capture at a time). Measure it
with `Sound.Level(path, { sections = seconds })`. What a capture taps and what going
offline changes: `docs/api/Audio.md`, *Capturing the mix*.

## Demos: every Play session, replayed (#1581)

A dev build records **every Play session** (the editor's Play, every `play` run,
the dev player) as a demo in the project's `saved/demos/<scene>-<UTC time>.demo`,
keeping the newest `demos.keep` (`project.rusty`, default 20; `0` records none).
Replay one headless, on the scene and settings it recorded:

```sh
cargo run --bin play --features dev -- --project <game> --demo <game>/saved/demos/<file>.demo [scenario.lua] <out_dir>
```

Without a scenario the whole demo plays; with one, the scenario drives it (its
`Harness.Step`s advance the replay), so screenshots, `Harness.Bench` and every
other harness verb work on a human's playtest. `results.json` says how the
replay went: `demo: replayed N of M ticks`, and an expectation `the demo replays
bit for bit` that fails at the first digest that differs, and fails a replay
that stopped before the demo's end (`stopped at tick N of M`) unless a scenario
stepped it there. **A scenario on a demo should only observe** (screenshots,
reads, `Harness.Bench`): a call that changes the world runs on top of the
recorded acts, and the replay diverges from there.
The digests are xxh3 (`core::hash::content`) since #1326: a demo recorded
before that no longer verifies, and is simply recorded again (*Backward
compatibility is not a concern*).

What a demo holds (`dev::demo`, the format in `dev/demo/format.rs`): the scene,
the engine commit, the seed, the project settings, the save data (`Storage`, the
player's options with it), `Time.timeScale`, `Time.paused` and the queued
`Time.Step`s Play started from, and a digest of the scene file and of the
project's `.lua` files, read right
before Play is entered (whether by a tick, a paused frame or `Editor.EnterPlay`),
the Game view size and the display scale (`Video.GetDisplayScale`, #1715) at
each change, and per tick the input the tick published
(`InputFrame`), as a delta from the tick before, and the tick's `dt`: the editor
and the dev player tick on the fixed tick in Play (#1877) and on wall-clock time
outside it, and the replay runs each tick on the `dt` it was recorded with. The
`Scene.LoadAsync` operations the tick read ready (#2123) are recorded with it too,
and the pending operation's progress where it moved: a window's load worker and GPU
warm-up decide both, so a replay reads the recorded progress and makes the
operations ready on the recorded tick, waiting for its own loader there if it is
behind. Everything else that reaches a playing world is
recorded as a **host act** before its tick: a console, MCP or command-channel
line (the one evaluator, `ScriptManager::eval`), a scenario's API calls with their
plain-data arguments, and the `Harness` verbs that change the world without a
tick (`Harness.AttachPlayerBot`, `Harness.LoadStress`). A call the scenario
runs in the gameplay VM (`Damage.*`, `CharacterController.Move`) replays in the
gameplay VM too, so the handlers it calls fire again; made before the game is up,
it replays as it was made (`Move` in the caller's own VM, `Damage.*` refused), by
the one rule both sides read (`dev::demo::forwards`, #1929). Acts made before Play is
entered (a scenario's set-up, a console line in edit mode) replay before
entering it, on an edit-mode runtime the replay brings up, acts made after it
after. `Editor.EnterPlay()` and `Editor.ExitPlay()` are not acts: the transition
they ask for is the demo's start and end. A demo still being written is never
pruned (its recorder holds a lock on the file). An act it cannot carry **taints** the demo from that tick: an edit made
by hand in the editor during Play (the Inspector, the hierarchy, a gizmo), a call
passing or returning a function or a userdata, a value written into a script or
passed to its function through a `Scene.GetScript` handle that is not plain data
(a function, a table with mixed keys), Play started on unsaved scene edits. The
handle itself is no taint: its field writes and function calls are recorded
(`ScriptRef.Set` / `ScriptRef.Call` in the file, `dev::demo::script_ref`, #2120)
and made again on the replay. The replay warns from there on.
Script and asset changes on disk wait for Stop (the editor's watch holds them
through Play), so they never reach a Play session. `play --size` and `--scene`
are refused with `--demo`: the replay plays the scene, the Game view size and the
display scale the demo recorded (the sim lays the UI out on the size, every frame
renders at it, and a `"match_display"` render scale resolves on the scale).

**Open one in the editor.** `Editor.OpenDemo(path)` ([`api/Editor.md`](api/Editor.md),
*The demo timeline*) plays a demo on the Game view with a timeline: play/pause,
scrub by tick (the same seek as `Harness.SeekDemo`) and the index's marks on the bar.

**Bit for bit is checked, not promised.** Every 120 ticks, and once right before
Stop (or when a `play` run ends), the demo stores a digest of the world: every
entity's id, name, active flag and transform, every rigidbody's velocities, the
play frame, `Time.time` and where `Random` stands, hashed straight from the
world. Script state (a Lua table's fields) is not in it: a script that diverges
shows once it moves something. The replay compares each digest and reports the
first that differs, with the last tick that still matched; a demo shorter than
120 ticks is still checked by its end digest. A demo recorded on another engine
commit, or on a project whose scene file or scripts changed since, warns and
replays anyway. A replay streams the file from disk, so an hour
of play costs no memory; an hour of steady mouse-look writes about 10 MB, and idle
ticks write nothing.

### Finding a moment (#1597)

While it records, a demo also writes an **index**: lines of their own beside the
tick lines (`{"at":t,…}`), read by a search and skipped by a replay. Every 15
ticks (four times a second) it samples the active camera's position (a NaN or
infinite one is skipped), and at the tick it happened it lists what the engine
knows: an entity spawned or destroyed (its id and name), a scene loaded
(`Scene.Load`; the entities that come and go with the load are not listed one by
one), Play entered, paused, resumed and stopped (Stop, and whatever came after
the last tick, at the last tick), and every console line (`Debug.Log`, warnings,
errors, the engine's own; a line repeated tick after tick, which the console
folds into one `line (×N)`, is listed at every tick it came). An index line that
does not read back costs only itself: the demo warns once and replays. A game fact the
engine can't know (health, the active weapon, "took damage") the game marks
itself with **`Debug.Mark(label [, value])`** (`docs/api/Debug.md`): stored at its
tick, a no-op when no demo records. An hour of play adds well under a megabyte of
camera samples; marks and console lines cost what the game writes.

A scenario on a demo searches it with **`Harness.FindInDemo(query)`**, which
returns the matching ticks in order, each once, read from the file without
replaying anything. A query holds exactly one of:

| Query | Matches |
|---|---|
| `{ mark = "took_damage" }` | a `Debug.Mark` with that label (the whole label) |
| `{ log = "Enemy" }` | a console line holding that text |
| `{ event = "destroyed" }` | an event: `"spawned"`, `"destroyed"`, `"scene"` or `"play"` |
| `{ inside = { min = {x, y, z}, max = {x, y, z} } }` | a camera sample inside the box, edges included |

**`Harness.SeekDemo(tick)`** replays up to and including `tick`, so the world
stands as that tick left it: the recorded input and `dt`, the digests checked on
the way, nothing drawn. A tick already behind the replay, or any tick once the
scenario has stepped past the demo's end, restarts it from the demo's start
state (Play is left, the scene, camera, clock and input go back as the replay
found them, a quit the game asked for is forgotten, and the demo is prepared
again), then replays forward: the same state, bit for bit, as a straight
replay. A tick past the demo's end, or a run on no demo, is an error; so is a
demo file gone since the run began, which leaves the run where it stood.
**`Harness.DemoPath()`** answers the replayed demo's full path (`nil` on no
demo). A worked example, on a playtest where the
player took damage:

```lua no-run
-- play --demo saved/demos/main-20261030-101500.demo scenarios/look.lua out/
local hits = Harness.FindInDemo({ mark = "took_damage" })
Harness.Log("hit at ticks " .. table.concat(hits, ", "))
Harness.SeekDemo(hits[1])                  -- the world as the first hit left it
Harness.Screenshot("out/first_hit.png")
Harness.Bench(120)                         -- and how the next two seconds run
```

### Cutting a scenario (#1597)

**`Harness.CutDemo(from, to, path)`** writes a scenario that reproduces ticks
`from` to `to` of the demo the run replays, so a bug found in a playtest becomes
a regression test. It copies no world state: the scenario runs on the **same
demo**, which it names by its full path in its header and checks it runs on,
seeks to the tick before `from`, steps the range on the recorded input, and ends
with an assert stub to fill in:

```lua no-run
-- Ticks 130 to 250 of the demo /game/saved/demos/main-20261030-101500.demo (Harness.CutDemo, #1597).
-- It runs on that demo, which must stay where it is:
--   play --demo /game/saved/demos/main-20261030-101500.demo <this file> <out_dir>
assert(Harness.DemoPath() == "/game/saved/demos/main-20261030-101500.demo", "not on its demo: play --demo " .. "/game/saved/demos/main-20261030-101500.demo")
Harness.SeekDemo(129)  -- the demo up to tick 130
Harness.Step(121)  -- ticks 130 to 250, on their recorded input
Harness.Expect(true, "TODO: what should hold at tick 250")
```

Run it with `play --demo <that demo> <scenario> <out_dir>`; its results say
whether the range replayed bit for bit (`the demo replays bit for bit as far as
the scenario stepped it`). `saved/` is git-ignored and pruned to `demos.keep`, so
a cut kept as a test needs its demo moved beside it (and the path in its header,
its check and its command changed to match).

## Rules
- Test and fixture files are capped at **150 lines** by the size gate
  (`tools/lint`), except a `<x>_tests.rs` sibling of `<x>.rs` (or of a `<x>/`
  module directory), which shares the 300-line source cap; keep them focused. Prefer small unit modules over one large
  test file.
- The sim must stay deterministic (fixed timestep, seeded RNG, no wall-clock in the
  tick) so harness/scenario tests are reproducible.
- A bake (a procgen texture, a capture) is byte-identical **on one machine**, not
  across architectures: `f32` transcendentals come from each platform's libm, so
  macOS aarch64 and Linux x86_64 differ in the last bits (#1487). A test compares two
  bakes made on the same machine, never a bake against a committed hash or pixel bytes.
- Two renders of the same scene with the same renderer are byte-identical on **every**
  adapter, Metal included, so a GPU test compares them exactly, with no tolerance.
  When Metal disagrees with itself (a level on a few silhouette pixels), a shader is
  reading something undefined: #1525 was SSR taking `dpdx`/`dpdy` inside a per-pixel
  branch, which lavapipe hides and Metal does not.
- CI (`.github/workflows/ci.yml`) and `make test` run the engine suite with
  **cargo-nextest** plus `cargo test --doc`, both feature sets; the lint xtask keeps
  plain `cargo test`. On Linux, `build-test` is a matrix of three parallel jobs
  (#2264): `build-test (default, 1/1)` runs the default suite (all but the GPU group,
  which the `dev` suite runs: *Which suite runs the GPU tests*), and `build-test (dev,
  1/2)` and `(dev, 2/2)` split the `dev` one with nextest's `--partition count:k/2`;
  each prints its twenty slowest tests and where its time went, the GPU group's test-seconds (`gpu`, `gpu-macos` on macOS) over its cap against the rest's over the runner's cores (and both over the cores, the bound on lavapipe, where the GPU tests are CPU work) beside the shard's wall, per binary, and the least memory the runner had free while its tests ran (after the build) (step *Slowest tests*; the macOS shards print ten), keeps its JUnit report as a 7-day artifact named by the shard (`junit-build-test-dev-1of2`, `junit-build-test-cross-macos-26-2of2`), and each runs the GPU and
  OIDN canaries again on its own runner (step *Canaries*), since a partition puts each
  canary in one shard only. The macOS leg (`build-test-cross`) runs the nextest suite with
  `dev` only, the superset (#1976), split in two, `build-test-cross (macos-26, 1/2)` and
  `(macos-26, 2/2)`, each with the canaries. **The macOS split is by time, not count**
  (#2397): `count:` deals tests round-robin, so its balance is luck of where tests
  fall in the list: one day the longest Metal tests all landed in 1/2, about 1.6
  times the GPU-seconds of 2/2, and a reordering later the two were within 7%.
  nextest has no partition that weighs time (`count:`, `hash:`, `slice:`). So the
  `shard-plan` job reads the leg's JUnit reports from `main`'s last successful runs
  once and hands both shards one plan (`.github/scripts/shard_plan.py`): `main`'s 120
  longest tests are dealt one by one, the rest by the last two letters of their name,
  each to the shard with less time, and shard 2/2 runs every test 1/2 did not take,
  so a test added since, or a plan the times got wrong, still runs exactly once. With
  no report to weigh by (a fork, expired artifacts, a renamed leg), too few timed
  tests to give each shard one, the `shard-plan` job failing (it is
  `continue-on-error`, so it never reddens the run) or the plan reader failing, both
  shards split by `count:` and the test step says so (`split: by count …`); the log's
  `split:` line names which. `gate_mirror.plan_gaps` holds the plan's `--shards` to the
  matrix's count. What the split buys is a balance that no longer moves with the test
  list, not a faster round on every run: macOS runners differ in speed by about a
  quarter (one shard ran at 0.78 of `main`'s times while the other ran at 1.03, on a
  plan that predicted 1729 s each), so the walls can still be a quarter apart. Each
  shard's *Slowest tests* step prints a `weight:` line, the test-seconds `main`'s
  times predicted for the tests it ran beside what they took: predictions apart mean
  a bad plan, ratios apart a slow runner. Linux keeps `count:`, where the GPU tests are CPU work and its two
  shards ran within about a third of each other on `main` (#2397). 2/2 also runs the `dev` doctests and the lint
  xtask's tests, and the default doctests run on Linux only (#2276). A job past three quarters of its `timeout-minutes`
  on `main` files an issue by itself (`ci_headroom.py`, `docs/linting.md`). See *The test runner* below.
- **A pull request that adds a slow test is told** (#2350). Each suite job's *Slowest
  tests* step first compares its report with the reports of the same leg and feature
  set from `main`'s last three successful runs that kept every shard's (every shard
  together, since a `count:` partition moves tests between shards), holds each test
  to its longest time over them, and names those runs, their dates and commits: a
  test merged after them reads as new. A test over 20 s (`slowest_tests.BUDGET`)
  that `main` had under it, or lacked, prints `new over 20 s: <test> <seconds> (main:
  <seconds or absent>)` and a warning annotation on the PR's checks when it is more
  than 1.3× (`NOISE`) `main`'s time at the shard's pace: how much slower than
  `main` the shard ran its heavy tests (`slowest_tests.pace`, printed as `pace:`),
  so a runner slow across the shard does not name what it slowed; one over it on both sides prints once as `known`,
  and one under it on `main` that did not grow that far as `near`
  (`slowest_tests.is_new`). With no baseline (expired, a renamed entry) it says `no baseline: <why>` and
  lists the tests over the budget. It informs, never gates, and exempts nothing: a test
  that needs the time says why in its PR; one that does not is trimmed against what it
  proves, still failing on the mutation it guards.
- A test never reads a developer's game project (#782). `Harness::new` runs in a
  workspace of its own, `<out_dir>/workspace`, seeded fresh from the test-fixture
  project's scripts and the bundled ones, and the scene's relative script paths
  resolve there; a test runs a scenario with `scenario::run_isolated`. The `play`
  binary is the one harness that runs a real project (`--project <dir>`, default
  `./project`; `Harness::in_user_workspace`): an agent play-tests its game, on the
  project's startup scene or `--scene <path>` (#1343). `Harness::new` builds the
  engine's default scene in memory instead, the fixture the engine's own tests use. Test code
  that names `in_user_workspace` or `scenario::run(` fails `make rules`
  (`test-user-workspace`, #980).
- **A run renders at a Game view size** (#1583): 1280x720 unless `play --size
  <W>x<H>` names another (`--size 2940x1388` for a Retina view). Every
  `Harness.Screenshot`, `Render` and `Bench` frame draws at it, with its targets, post
  effects and UI sized to it, and the sim lays its UI out on it as a window that size
  would. One shot can name its own size, as Unity's
  `ScreenCapture.CaptureScreenshot(path, superSize)` does:
  `Harness.Screenshot(path, { width = 2940, height = 1388 })`; the run keeps its size.
  Each side is a whole 1 to 8192 (`video::MAX_SIDE`), else an error.
- **A shot can show what the game cannot** (#1882): `Harness.Screenshot(path, {
  gizmos = true })` draws every collider's real shape (Unity's green, a trigger
  paler) and the `Debug.DrawLine` / `Debug.DrawRay` lines over the frame, as the
  editor's Game view **Gizmos** toggle does: the fastest way to see why a shot
  missed or a crate snagged. It takes a size too (`{ width =, height =, gizmos = }`).
  Without it a shot is the plain frame, byte for byte. A line drawn with no
  duration lasts one tick, so take the shot right after the step that drew it.
  The gizmos follow the main (base) camera, even under a viewmodel camera, and
  are left out while TAA upscales the view (a render scale below 1 with TAA on;
  the console says so once). The editor's Game view captures the same with
  `make editor-capture ARGS="--game --gizmo GameGizmos=on"`.
- **A shot after a camera jump needs a cut** (#2003): with TAA on, each shot blends
  into the history the shots before it left, and `Harness.Step` renders nothing, so
  a shot from a new spot ghosts the last one in. Call `Camera.ResetHistory()` after
  the jump, and add `{ settle = n }` (whole frames, 0 to 1024, default 0) for a
  converged image: the shot first draws `n` frames with no sim step, as `n`
  `Harness.Render()` calls would. It draws them only when they can change the
  image (#2092). When the shot's first frame reads no history
  (`Debug.Stats().history_passes` is `0`), that frame is the shot: one render
  whatever `n` is (two with `gizmos`), the same bytes a full settle writes. A frame
  reads history with TAA, SSIL, the volumetric fog or an authored effect on, with
  motion vectors recorded on a frame that is not a cut (whether or not the camera
  moved), with a render texture shown, or with a shadow tile resize still pending.
  It goes beside `width`, `height` and `gizmos`; `docs/api/Graphics.md`, *TAA*.
- **The test-fixture project** is `tests/fixtures/project/` (#829): the committed
  scenarios, the bot-player script and its `project.rusty`, opened like any other
  project — `cargo run --bin play --features dev -- --project tests/fixtures/project
  tests/fixtures/project/scenarios/smoke.lua out/`. What the engine seeds into it
  when opened is git-ignored. `play` opens a project without writing its
  `project.rusty`, so the fixture's recorded engine commit is stale by design; it
  records no count, so a run reads it as an engine upgrade and carries on without a
  warning (#853, #1319). In-process tests run with the checkout as their
  working directory, so anything they seed lands under the git-ignored `/assets/`.
  A test's own scripts go in that workspace too, attached by workspace-relative
  path, so its snapshot carries no machine path (#783).
- A test builds an engine struct through a constructor or `..Default::default()`,
  never a literal naming every field, unless every field is the point of the test:
  a new field then breaks only the tests that use it (#663; e.g.
  `TextureRecipe::new(res, nodes).with_seed(7)`). For the first-class components
  `make rules` holds it (`full-literal`, #976): build one from its default
  (`..CameraComponent::default()`, `..edit::authoring::default_light()`) or a
  constructor (`MeshComponent::new(primitive, vertices, indices)`), or say why every
  field is the point with `// lint: allow(full-literal) <why>`.

## The test runner: cargo-nextest
`make test` and CI run the suite with [cargo-nextest](https://nexte.st) (#484),
configured in `.config/nextest.toml`. Install it once: `cargo install --locked
cargo-nextest` (CI installs a pinned version through `.github/actions/install-tool`,
which caches it, #1973). What it buys over `cargo test`:

- **Each test is its own process**, so one test's panic, leaked global or stuck
  thread cannot take its neighbours with it.
- **Every failure, summarised at the end** (`fail-fast = false`), instead of stopping
  at the first failing binary — the list an agent reads to decide what to fix. `make
  test` goes on the same way across its four runs (nextest and doctests, both feature
  sets), and fails at the end if any did.
- **A hang is named, not waited out**: a test is flagged SLOW after 60 s and killed
  after 5 min.
- **A fixed proptest seed in the gates, a random one in exploring** (#1800, #1852).
  `ci.yml` runs its gates on one `PROPTEST_RNG_SEED`, and `make test` (so `make
  gates`) and `make test-one-process` read that same number from `ci.yml` (the
  Makefile exports it to every recipe), so a red proptest comes from the
  branch under test, never from a seed that happened to draw an old bug. An exported
  `PROPTEST_RNG_SEED` wins, to reproduce a reported seed. Exploring reports and never
  blocks: the nightly `ci.yml` run and `make test-explore` draw fresh seeds
  (`PROPTEST_CASES=<n>` for more cases). Whichever finds a case, commit its `cc` line
  with the fix, to the file proptest names when it fails: `proptest-regressions/<module
  path>.txt` beside the nearest folder holding the test's `lib.rs` (the root's for
  `src/`, e.g. `proptest-regressions/physics/watch/tests.txt`; the crate's for a moved
  module, e.g. `crates/ecs/proptest-regressions/activity_tests.txt`), which every later
  run replays first. A root file naming a module no longer under `src/` is never read,
  and `make rules` fails on it (`orphan-seeds`). A suite that pins its own seed in
  its config (`tests/proptest/world_space.rs`) doesn't explore.
- **No retries.** A retried pass is still green, and a flaky test is a bug.

`cargo test` still works and is still correct — nothing in the suite depends on the
runner — but the gate is nextest. A nightly signal holds the promise: `cargo-test.yml`
runs plain `cargo test`, both feature sets, on `main` every night, and a red run files
itself through `main-health` (#981). It is what catches a test that **leaks
process-wide state** (a moved working directory, a `static` left set): nextest's
process per test hides one, but `cargo test`, which the mutation runs use too, puts a
binary's tests in one process, so before the signal went nightly such a leak broke the
mutation baseline of every branch until a mutation run failed (#1042, #1069, #1337).
It is a signal, not a PR gate, because it would add about two minutes to every PR's
CI for a leak that only breaks the informational mutation runs (#1338).
To check a branch before it merges: `make test-one-process` runs the `dev` suite that
way locally (quiet, every binary, the failing tests named at the end), and
`gh workflow run cargo-test.yml --ref <branch>` runs the whole signal on GitHub.
**A test never holds a provider key** (#974), under any runner:
`dev::providers::environment()` drops every `*_API_KEY` and skips the `.env` unless
the binary opted in at startup (`dev::providers::read_keys`, which only the editor,
`session` and `session-mcp` call), and no test binary does. A test starts an engine
binary only through `tests/spawn.rs` (`crate::spawn::engine_bin!("session")`), which
sets `RUSTY_NO_PROVIDER_KEYS`, so a spawned binary holds no key either, under nextest
or a bare `cargo test` alike; the `engine-spawn` lint fails a `CARGO_BIN_EXE_` named
anywhere else. That lint is the primary guard; the backstop is nextest's `NEXTEST`
variable, which every test process and every child it starts inherits, and which
blanks the keys too, so a spawn the lint cannot see (`cargo run --bin`, a built path)
holds none under nextest. Doctests stay on `cargo test --doc`, which nextest
cannot run. CI's `ci` profile additionally writes JUnit, rendered as each
`build-test` job's summary.

Handy forms: `cargo nextest run -E 'test(physics_)'` (filter by name),
`cargo nextest list` (what would run), `cargo nextest show-config test-groups`
(which tests the GPU group below holds, `gpu` or `gpu-macos`).

## One integration-test binary
Cargo's default builds **each** `tests/*.rs` as its own executable, and each one links
the whole engine — wgpu/naga, egui, rapier3d, vendored Lua. With dozens of files and two
feature sets, that multiplied the slowest serial step of the build (linking) by the file
count and filled `target/` with near-identical binaries (#483). So `Cargo.toml` sets
`autotests = false` and declares a single `[[test]]` target, `integration`, rooted at
`tests/main.rs`; every file under `tests/` is a **module** of it.

- **A new test file needs a `mod` line** in `tests/main.rs` (or its folder's `mod.rs`) —
  Cargo no longer discovers it. `tests/layout.rs` fails the suite, naming the file, if
  one is forgotten, so a file can't silently never run; `make rules` (`orphan-module`)
  holds the same for every scanned `.rs` file, crates and tools included.
- **Dev-only files are gated at the declaration**, `#[cfg(feature = "dev")] mod …;`,
  not by a `#![cfg]` inside the file. `make rules` (`test-inner-cfg`) fails on one,
  and its sibling locks hold the user-workspace and GPU-entry rules below (#980).
- **Tests that render live in `tests/gpu/`**, so they share the `gpu::` module path and
  a runner can identify them by name (see the budget below).
- **Every module can share one process.** nextest gives each test its own, but
  `cargo test` runs the whole binary in one, and both must stay correct. Nothing in
  `tests/` may rely on having a process to itself: write scratch files under
  `temp::dir()`, which is unique to the process *and* the test (#709), and never
  `set_var` / `remove_var` / `set_current_dir`. Clippy bans those three crate-wide
  (`clippy.toml`'s `disallowed-methods`, #977): pass the value as a parameter
  instead. Nor may a test write a process-wide `static` (flip a flag, reset a
  counter, fill a cache): keep the state on the value under test, in a
  `thread_local!`, or in a child process; the `test-static-write` rule fails the
  write, in a test or in a test hook (see `linting.md`). Production code that really owns the process (`project::open` moving
  the working directory) opts out inline with `#[expect(…, reason = "…")]`.
  In-crate tests have the same helper,
  `rusty_test_support::temp::dir()` (#999, `crates/test-support/`); the `test-temp-dir` rule fails a bare
  `std::env::temp_dir()` in test code (see `linting.md`).
- **What is process-wide by nature gets a process of its own.** A test reading a
  process-wide measure (the peak memory, `memory::rss_peak_bytes`), asserting an
  exact value of a process-wide counter live code bumps
  (`asset::convert::conversions`), or installing state nothing else may share runs its body through
  `rusty_test_support::alone!(|| …)`, which names the test from its function:
  under nextest it runs in place, under plain `cargo test` the binary re-runs
  that one test in a child, and passes only once the child says the body ran.
  A check that the machine cannot measure goes before `alone!`, so it starts no
  child to return at once. The write log (`core::fs::written`) is process-wide
  too, and every write through `core::fs::written::write` or `note_written`, any
  test's, notes into it (a plain `std::fs::write` does not): a cache or reader
  draining it acts only on the notes naming its own keys, so its "nothing
  happened" (`ui::TextureSizeReader`'s idle refresh sweeps nothing) holds while
  other tests write, and a test of it keys by its own `temp::dir()` paths. The `log`
  logger is one a process too: a crate that names `rusty-logging` captures its
  tests' warnings through `rusty_logging::init_for_tests` and `forward`, never
  `rusty_test_support::logs` (`one-logger`), and each `init` panics when the
  other's logger is already installed.
- **A path a test expects is spelled as the engine writes it**: a plain
  `Path::join("a/b/c")` or `"a/b/c"`, `/`-separated, with a `\` read as part of a
  name, never a separator. In-crate tests take a relative root from
  `rusty_test_support::temp::relative(dir)` (#1332). Every text file checks out LF
  (`.gitattributes`), so embedded and byte-compared text reads the same whatever
  the git config.
- Filtering works by module path: `cargo nextest run -E 'binary(integration)' physics_`.

## Build and test share one dependency graph
Cargo resolves features for `cargo build` without the dev-dependencies. If a
dev-dependency switches on a feature that the normal build leaves off, `cargo test`
compiles its own copy of that crate, of everything above it, and of rusty. proptest
did this to `getrandom`'s `std` feature, and that doubled ahash, hecs, wgpu and egui
(#552). #1334 builds proptest without its default features to close that gap. Where
a normal dependency brings in a crate a test-only one widens (`scorsese-providers`'
`zip` and getrandom 0.4, under `dev`), `Cargo.toml` names that crate with the test
build's features, as a `dev`-gated pin no code uses (listed `unscoped`, with its
reason, in `tools/lint/src/layers/externs/policy.rs`).
**Don't let a dev-dependency turn on a feature the normal build lacks.** The
`feature-split` gate (`make feature-split`, `.github/scripts/feature_split.py`, #979)
compares the two resolutions on every run. Test-only crates (proptest and its
tree) appear only in the second, which is fine. A crate that appears in **both**
with a feature only the second has is the split. By hand:

    cargo tree -e normal,build --features dev -f '{p} [{f}]' --prefix none | sort -u
    cargo tree -e normal,build,dev --features dev -f '{p} [{f}]' --prefix none | sort -u

## Incremental rebuilds: the codegen-unit cap
rustc compiles a crate in codegen units (CGUs), roughly two per module. The default
dev cap is 256. Above the cap, rustc merges units by size, so an edit that changes one
function's size reshuffles the merge and a large share of the crate's units
recompile. Most members sit at the cap (`render` would split into about 890 units,
`scene` into about 1050). The CGU count is the number of `.o` files in the newest
session dir under `target/debug/incremental/<crate>-*`; the units an edit recompiled
are the ones whose inode is new in that dir.

**Raising a member's cap does not pay** (`[profile.dev.package.<crate>] codegen-units = 4096`),
because the row helps only while its own crate is edited. A raised crate recompiles 6-7
units instead of a hundred or more, but every time something *below* it changes it is
rebuilt as a dependent, and that rebuild costs more with four times the units. In a
workspace each edit rebuilds the crates above it, so the gain in one crate is spent in
the others: raising every member is a wash, and so is raising only the members more
crates depend on than they depend on (eleven of them, `scene` among them).

Measured on a 4-core cloud machine with a warm `target/`, alternating a line that
changes the size of `scene::lighting::probe::index::scan` (6-8 samples each, medians,
ranges in brackets). "scene" is `rusty-scene`'s own compile from `--timings`:

| Cap | CGUs recompiled | scene | `build --features dev` | `nextest run --features dev --no-run` |
|---|---|---|---|---|
| 256 everywhere (today) | 129 of 256 (165 in its test target) | 6.2 s, 8.0 s in the test build | 13.9 s [13.1-15.1] | 32.4 s [31.4-34.6] |
| 4096 on every member | 6-7 | 3.5 s, 4.0 s | 14.5 s [13.4-15.3] | 33.2 s [32.1-34.4] |
| 4096 on the 11 members with more crates above than below | 6-7 | 4.2 s | n/a | 31.8 s [29.7-33.1] |
| 4096 on `scene` only | 6-7 | 3.8 s | n/a | 28.9 s [27.4-29.3] |

An edit in `render::camera::texture_cameras` reads the same: 160 of 256 units and
3.6 s at the cap, 6-7 units and 2.5 s raised, and `cargo build` 9.5 s against 10.2 s,
the binaries' links eating the difference.

So the row pays only for a long stretch of work inside one crate, where it saves about
3.5 s of a 32 s edit-to-test-binaries loop. Set it for that session from the command
line, not in `Cargo.toml`:
`cargo nextest run --features dev --config 'profile.dev.package.rusty-scene.codegen-units=4096'`
(the first build after switching it rebuilds that crate and those above it).

The 13-19 s codegen spikes #495 measured did not reproduce, either with a build-only
loop or with #495's own edit-build-test-revert loop. They also cannot be CGU
invalidation. Codegen for every unit from scratch took 4.5 s on that 8-core machine
(8.3 s here), and a spike cost 3-4 times that. The likely cause is contention on the
machine: memory pressure, or another process such as rust-analyzer's check-on-save.
If they return, count the recompiled `.o` files as above. A spike with few recompiled
units points to the machine, not the crate.

## The link step (#2202)
The figures below are one machine's, the operator's, whose builds wait on its disk:
they are the reasons behind each decision here, not numbers to plan by, and another
machine measures its own. Every root binary links the whole engine statically,
about 190 MB each in the dev profile, eight of them per `cargo build --features dev`.
What links them, and what was measured against it, so it is not measured twice:

- **The linker is already `lld`.** rustc links `x86_64-unknown-linux-gnu` through
  its bundled `rust-lld` (`cc -fuse-ld=lld`, the default since 1.90), so Linux dev
  machines, cloud containers and CI's runners all use it with nothing configured
  (`rustc --print link-args` shows it). macOS links with Apple's `ld` (the one Xcode
  ships). `mold` is not set anywhere: a linker named in `.cargo/config.toml` would
  have to exist on every machine that builds. Relinking `play` alone (one link,
  warm page cache): `lld` 0.95 s and 1.05 GB peak, `gold` 4.5 s, GNU `ld` 7.7 s.
- **Debug info is not what the linker spends time on.** Dependencies already build
  with `debug = false` (`[profile.dev.package."*"]`); the members keep
  `line-tables-only` for backtraces. Dropping every debug section at link time
  (`--strip-debug`) took the same `play` link from 0.95 s to 0.89 s (178 MB to 107
  MB). `split-debuginfo = "unpacked"` shrank `play` to 153 MB but wrote as many bytes
  per touched build (rustc rewrites the `.dwo` beside each changed unit) and was no
  faster. `--compress-debug-sections=zlib` cut a touched build's writes by a
  quarter (2.07 GB to 1.59 GB) with no wall-time gain over eight interleaved pairs;
  `zstd` compresses as well but std's backtraces then lose `file:line`.
- **A link is about a second; a slow one is waiting on the disk.** Alone, a
  binary's rustc run takes 1.5 s, 1.0 s of it the link. When a touched build links
  the eight at once (four at a time on four jobs), each can take 15-17 s, stalled
  on writes: a touched `cargo build --features dev` wrote about 2.1 GB, 1.5 GB of it
  the eight binaries, and the same touch took anywhere from 15 s to 52 s over 14
  runs. What a build writes, not the linker, is the lever on such a machine.
- **The root package builds no unit-test harness.** `[lib]` and every `[[bin]]`
  say `test = false`, and `[package]` says `autobins = false` so no binary is found
  without its `[[bin]]`: the root's sources hold no tests, and each harness was one
  more rustc run and link against the whole engine on every `cargo test` / `nextest
  run`, the nine together about 2 s of a test build. The `root-tests` rule
  (`docs/linting.md`) holds it.
- **Dynamic linking was measured and not adopted.** A `dylib` crate re-exporting
  the members, with `-C prefer-dynamic`, halved what a touched build wrote (0.93
  GB) and made each binary a few hundred KB, but the shared library's own link took
  9.7 s, so the build was no faster over four samples. It would also need
  `RUSTFLAGS` (a second full build of every crate) and `LD_LIBRARY_PATH` for every
  binary run outside cargo (the launcher's `rusty`, MCP's `session-mcp`).
- **`make check` links nothing** (clippy stops at metadata), and the `make` verbs
  that run one binary already build only it (`--bin`).

## GPU tests and the headless budget
Tests that need a real device call `Renderer::new_headless`, which returns `None`
when no adapter is present — so **every GPU test skips gracefully** rather than
failing. In-crate tests acquire one through
`crate::render::test_gpu::headless_or_skip`, which carries that skip contract in one
place; the `let Some(r) = … else { return }` shape is the whole convention. A direct
`new_headless` in test code fails `make rules` (`test-gpu-entry`, #980).

Where they actually run is not uniform, and it is worth knowing before you rely on one:

| CI job | Adapter | GPU tests |
|---|---|---|
| `build-test (dev, k/2)` (ubuntu) | Mesa **lavapipe** (software Vulkan, #489) | run, on the CPU |
| `build-test (default, 1/1)` (ubuntu) | lavapipe | compiled, not run, but the canary |
| `build-test-cross` (macos) | real Metal GPU | run, against real VRAM |

### Which suite runs the GPU tests
The `dev` suite (#2352). The default-feature suite, `make test`'s first pass and CI's
`build-test (default, 1/1)`, runs nextest with `-E 'not group(gpu*)'` (the Makefile's
`DEFAULT_SUITE`, `make test-explore`'s default pass too): it **compiles every test
target** of the default build, so a default-only build break still fails, and runs
every test except the GPU group (`gpu`, `gpu-macos` on macOS). `--features dev` only adds to the default
features, so the `dev` run builds and draws each of those GPU tests too; what it
cannot see is code that exists only *without* `dev`. So the split rests on `dev`
only *adding* code, and the `gpu-default-only` lint (`docs/linting.md`) checks the
other side: in `rusty-render`, every crate its `dev` turns on and every crate with an
in-crate `gpu_` test, a `cfg!` naming `feature = "dev"` or a `cfg` whose `not(…)`
names it fails, a GPU test hidden that way included. A `dev`-only *override* (a value
or a module `path` set under `feature = "dev"`) changes what the GPU tests draw
without hiding anything; the lint cannot tell it from an addition, so it is the
reviewer's to catch. Keep the code the same under both feature sets, or add
that crate's GPU tests back to the default pass by widening the filter in both
callers, `make test`'s `DEFAULT_SUITE` and ci.yml's "Test (engine, default)", e.g.
`-E 'not group(gpu*) | (group(gpu*) & package(rusty-<crate>))'` (`make scripts` fails
when the two differ, `test_gate_mirror.py`). The adapter canary still runs in the
default entry (step *Canaries*), so a runner that lost lavapipe fails there too.

**CI requires an adapter.** Every workflow job that runs the suite (nextest,
`cargo test` other than `--doc`, `llvm-cov`, `mutants`) sets `RUSTY_REQUIRE_GPU=1`, and
`make scripts` fails a job that does not (#978). The variable turns one
canary test (`render::test_gpu::tests::gpu_adapter_present_when_required`) from a skip into
a failure when no adapter is found — so a runner image that loses its driver goes red
instead of quietly skipping every GPU test. The dev capture layer answers to the same
variable (#885): `dev::capture::no_adapter` turns a capture's "no adapter" (a
screenshot, `Debug.Preview`, an editor capture) into an `Err`, so every visual test
under `tests/gpu/` fails rather than skips, and
`dev::capture::capture_required_tests::gpu_capture_renders_when_required` fails if a
capture path wrongly reports no adapter. Without the variable (any local machine)
the skip contract above is unchanged.

**OIDN tests skip the same way** (#1884): the lightmap denoiser's tests
(`dev::bake::denoise::tests` and `bake_tests`) need Open Image Denoise installed and skip without it.
CI's `build-test` (the pinned release, where `RUSTY_OIDN` points) and the macOS leg
(Homebrew's) install it and set `RUSTY_REQUIRE_OIDN=1`, which turns the canary
`oidn_is_present_where_ci_requires_it` into a failure when OIDN does not open. The
other jobs build `--features dev` without it (rusty opens OIDN at run time, never
links it) and skip those tests. The bake's own denoiser logic is tested with mocks
(`lighting::lightmap::denoise_tests`) and runs everywhere.

**Running them on a GPU-less Linux box** (a container, a cloud session): install
lavapipe — `sudo apt-get install -y mesa-vulkan-drivers libvulkan1` — and the same
tests render there as in Linux CI. Set `RUSTY_REQUIRE_GPU=1` to be sure they did.
CI installs the same packages through `.github/actions/system-libs`, which keeps
the `.deb`s in the Actions cache per runner image and ref, so a mirror outage fails no
run on an image that ref (or `main`, whose cache every ref reads) has already served
(#2101).

**Runner image.** Every job runs on a pinned image, never `-latest` (#930): lavapipe
comes from the image's apt archive, so a moving image changes the driver under the GPU
tests with nothing in any diff. Every Ubuntu job (the gates, and the mutation, coverage
and `main`-health runs) names the image `ci.yml`'s `build-test` names, the one it is
defined by, so a signal tests on the driver the gate proves; a cache key that names an
image names that one too, so no `target/` built on another image's glibc restores. A
`runs-on` the check cannot read fails rather than passes (`make scripts`,
`.github/scripts/tests/test_runner_image.py`). The
`build-test` log prints the Mesa version it rendered with and the runner's CPU (step
*Driver version*), as the mutation Baseline's *Runner CPU* step does.
Moving to a new image is a PR of its own (the check lists every line to change), and reads #2385 first: a shadowed
spot's fog shaft went missing twice on Mesa 26's lavapipe and never on Mesa 25, its
cause not named. `ubuntu-26.04` is otherwise proven (#868): the apt list resolves
unchanged, lavapipe is Mesa 26.0.8, and a cold-cache build ran every GPU test there.

Linux CI's adapter is a software renderer, so a green run there proves the render
path is correct on a conformant driver, not that it performs or behaves identically on
real hardware — Metal on macOS is the one real GPU CI has. Pair anything load-bearing
with an adapter-free unit test on the underlying predicate so the rule is pinned
everywhere.

### Which adapters a GPU tolerance holds on

A GPU test that holds a number (a step count against a CPU reference, a ratio, a
mean) holds it on **three adapters**: **lavapipe** (Linux CI, cloud sessions),
**Metal** (CI's macOS leg) and a **real desktop GPU** (the operator's machine, a
discrete Vulkan driver). CI runs only the first two, so a number tuned there is first
seen failing on the third, by whoever runs `make test` there next.

- **A tolerance says what it was measured against**: a comment beside it with the
  adapters and the values they gave (`Measured: RTX 2060 1, lavapipe 1, Metal 1`), and
  what the bar stands for. It moves only with the reason written there; "widened until
  green" is not one. No check holds this: a tolerance is a constant like any other in
  a test's text, so the review does.
- **Find the difference before moving a number.** For a GPU path held to a CPU
  reference, nudge the reference's own inputs by a GPU's rounding (1e-7 to 1e-5) and
  see how far it moves from itself. A tolerance below that is a coin toss on some
  adapter; a fixture where it is large tests ties (a tap exactly on a texel edge, a
  seam), not the code, and wants a fixture without them, not a wider bar. For a ratio
  between two renders, take it on every adapter first: its spread is what the bar must
  sit under.
- **One adapter, one command**: `make test-gpu` runs the GPU group alone, in the `dev`
  feature set (the suite that runs it, above), on lavapipe when its Vulkan driver is installed (`lvp_icd` under
  `/usr/share/vulkan/icd.d/`: Arch's `vulkan-swrast`, Debian's `mesa-vulkan-drivers`),
  else on the machine's own; `ADAPTER=nvidia` (any part of
  an adapter's name, case ignored) picks another. Under it sits wgpu's own
  `WGPU_ADAPTER_NAME`, which every headless renderer honours
  (`render::setup::headless::ADAPTER_ENV`): set on any command, only that adapter is
  used. None of that name skips each GPU test but fails the canary, whatever
  `RUSTY_REQUIRE_GPU` says, so a mistyped name is never a green run that rendered
  nothing. A test red on a real GPU and green on lavapipe, on the same commit, is the
  driver, not the change. The variable steers every headless renderer, not only the
  tests: the probe and reflection bakes, `dev::capture`'s screenshots and
  `Debug.Preview`. The windowed renderer ignores it, so left exported in a shell, an
  editor draws its window on the real GPU and bakes on the named adapter.
- **Every run names its adapter and its values**: the canary
  (`render::test_gpu::tests::gpu_adapter_present_when_required`) prints
  `[gpu] adapter: <name> (<backend>, <driver>)`, and a test that holds a tolerance
  prints what it measured (`eprintln!`, as the prefilter and TAA shimmer tests do).
  nextest shows both after every run, passing too, CI's included, for the tests its
  `success-output` override in `.config/nextest.toml` lists; a new tolerance test adds
  itself there.
- **Before a new or moved GPU tolerance merges**, it is run on a real desktop GPU:
  `make test-gpu ADAPTER=<gpu>` on the branch's head, on the operator's machine (a
  local coder, or the orchestrator for a cloud branch), and the PR's `Gates` line names
  the adapter and the value. Lavapipe's and Metal's values are read from CI's log of
  the branch's run (`build-test`, `build-test-cross`).

**Concurrency is capped.** A `Renderer` is a device, the full pipeline set and shadow
maps, and a software adapter (lavapipe) allocates all of that in system RAM shared with rustc — so
unbounded parallel renderers exhausted memory and failed *unrelated* tests with a bare
`Queue::write_texture: Not enough memory left.` (#366). Two caps, one per runner, each
with one number per platform:

| Platform | nextest group | Cap | What measured it |
|---|---|---|---|
| Linux (lavapipe) | `gpu` | 2 | 3 on a 4-core CI runner was no faster on the lighter `dev` shard and slower on the busier one: lavapipe renders on the CPU, so a third renderer only shares the cores (#985, #2363) |
| macOS (Metal) | `gpu-macos` | 3 | on a 3-core CI runner the heavier shard is bound by the cap at 2 and by its cores at 3. Over four runs at 3 against two at 2, the slowest shard's test run fell from 589–604 s to 507–592 s (mean about 596 → 540 s, a tenth), and the round's last-ending job from 800–905 s to 735–885 s (mean about 852 → 815 s, about 4%): a small gain, within the runners' run-to-run spread for the whole job. At least 2.7 of 7 GiB stayed free (#2363) |

- **nextest (the gate):** every test is its own process, so nothing in-process can see
  its neighbours. The GPU **test group** in `.config/nextest.toml` bounds how many GPU
  tests run at once. A test group has no platform key, so each platform has its own,
  picked by a platform-keyed override with the one filter; a filter names both as
  `group(gpu*)`.
- **`cargo test`:** `MAX_CONCURRENT_HEADLESS` in `crates/render/src/setup/budget.rs` (one
  per `cfg(target_os)`) bounds
  how many renderers are alive in the one process, enforced by an RAII permit the
  renderer holds for its whole life. It applies to the normal library build, not just
  `cfg(test)`, because the screenshot integration tests reach the renderer indirectly
  through `screenshot::capture`. A test thread that holds a permit and asks for a
  second panics at once (#1844): under `cargo test` two such tests each hold one slot
  and wait forever for the other's. A test that keeps two renderers alive takes both
  slots together, `render::test_gpu::headless_pair_or_skip` (or `test_gpu::pair::headless_pair`, when
  the engine builds both, as a capture host beside a bake does). Any other thread
  waiting past 180 s panics with an explanation.

Keep each platform's two numbers equal; `make scripts` fails when they differ, and holds the
nextest settings above (no retries, `fail-fast = false`, the timeouts) in place
(`.github/scripts/tests/test_gpu_config.py`, #978). If a new GPU test makes CI run out
of memory, **lower both to 1** before weakening the test. Every suite job prints the
least memory its runner had free while its tests ran, after the build (a `memory:` line under *Slowest
tests*, `.github/scripts/memory_floor.py`): read it before raising a cap.

**On your own machine** `make test` runs at your platform's number. A machine short on
memory (check the machine: free memory while the suite runs, `free -g` or Activity
Monitor) should run fewer renderers at once: lower the group's `max-threads` in
`.config/nextest.toml` for the run and leave the edit uncommitted, or cap the whole
run, `NEXTEST_TEST_THREADS=2 make test`.

**The GPU naming rule.** A test group selects tests by name, so the name says whether
a test renders:

- an **integration** test that renders lives under `tests/gpu/` (path `gpu::…`; the
  filter takes that path in the integration binary only, `kind(test)`, because
  `rusty-render`'s own unit tests sit in its `gpu` module and share the prefix);
- an **in-crate** test that renders has a function name starting with **`gpu_`**
  (`fn gpu_resize_tracks_the_new_size`).

You cannot forget it quietly: in debug builds, a test that builds a headless renderer
without such a name panics and names this rule (`render::setup::gpu_rule`, called
from the budget's acquire). A test in that module also fails if the filter in
`.config/nextest.toml` stops matching the Rust predicate, so the two copies of the
rule cannot drift apart.

## Tests that need Blender (#1072)

rusty requires Blender 5+ to import a `.blend`, but `make test` and the suite jobs
run without one, so the tests that convert through a real Blender
(`asset::convert::blend::tests`, and exporting a `.blend` project in
`tests/export/blend.rs`) are `#[ignore]`d, each with its `skip` reason.
CI's `blender` job installs Blender and runs exactly those; locally, with Blender
on `PATH` or named by `RUSTY_BLENDER`:

    cargo nextest run --features dev --run-ignored only -E 'test(/convert::blend::|^export::blend::/)'

That filter takes in Blender's converter fingerprint
(`converter_fingerprints_hold_for_blender`, *Converter fingerprints* below), whose
rows are Linux's at the Blender CI pins: with another Blender, or on macOS, it fails
on the program or the hash, not on a bug. Add `and not test(converter_fingerprints)`
to the filter there.

Finding Blender (not found, too old, the override, a version that fails, hangs or
cannot start, a newer one put earlier on `PATH`)
is tested in the ordinary suite with fake executables, and the cache's handling of
a converter's program (its version in the stamp, none installed, Play and a missing
source never asking)
with a fake `Converter::tool`, so neither needs Blender. The `.blend` fixtures in
`tests/fixtures/models/` are made by the `make_*_blend.py` scripts beside them:
re-run one with Blender to change its fixture.

## Editor captures (#731)
`screenshot::capture` shows the game; **`shell::editor::capture`** shows the *editor* —
the egui panels around the scene viewport — rendered offscreen to a PNG with no window,
on lavapipe or any adapter. It is how an agent that changes the editor lets the operator
see the change without launching it:

```sh
make editor-capture OUT=after.png ARGS="--select Player"
# or: cargo run --features dev --bin editor-capture -- --out after.png --play --game
```

Options: `--project <dir>` (default `./project`), `--scene <path>` (a `.scene` in the
project; default: the built-in default scene, in memory), `--select
<name>`, `--select-asset <path>` (project-relative, e.g. `assets/scenes/a.scene`: open an asset's Inspector card — a `.scene`'s bake
buttons, an image, audio or prefab card; exclusive with `--select`), `--frame` (frame the selection in the Scene view, as the F key does), `--play`
(the Play-mode chrome; the sim is not stepped), `--game` (the Game tab), `--size <W>x<H>`
(default 1600x900), `--demo <file>` and `--demo-tick <n>` (a demo open on the Game
view's timeline, replayed to tick `n`, default 0; #1598), `--pointer <X>,<Y>` (the
pointer held there, in pixels, so a hover shows), `--gizmo <Name>=on|off` (a Scene-view
gizmo drawn or not, by its `Editor.SetGizmo` name, e.g. `--gizmo NavMesh=on`;
repeatable, #1909), `--game-view <Free|W:H|WxH>` (the Game view's size, as its
dropdown sets it, #2207: `--game --game-view 16:9` letterboxes it in the panel), `--preview [<mesh>]` (the selection's Inspector **Preview** tab open on `Sphere`, `Cube`, `Suzanne` or `Plane`, default the tab's own; a material entity or a model, texture or shader asset, else an error; never beside `--picker`, #2239), `--picker [new|open|mismatch|focus]` (the project picker the editor starts
on, #854, over a sample recent list dated against a fixed time; the page names show
*New Project*, *Open* and the different-engine prompt). From Rust, `capture::capture(&game,
path, &EditorCaptureOptions { .. })` takes any `GameWorld`, and `capture_into` shares a
`CaptureHost` across shots.

Each frame runs the editor's own `EditorUi::draw`, renders the scene into the viewport
rect the layout asked for, and binds it as the viewport's texture; the last frame is
painted with `egui_wgpu` and read back. A fixed size, a 1× scale, a fixed frame count
and a synthetic clock make it **deterministic**: the same scene and options give the same
bytes on the same machine. What it cannot show: open menus and popups, and a hover
anywhere but at the one `--pointer` position.

**An editor-visible branch captures before (on `main`) and after** of the panel it
changes, and its coder looks at both and says what it saw in the PR; no PNG goes to
GitHub (*Looking at a change*, below). `.gitignore` ignores `editor-capture*.png` at
the root, but a capture belongs in the session's scratchpad.

The smoke test (`tests/gpu/editor_capture/panels.rs`) checks that the default scene captures,
the image is not blank and the hierarchy, viewport and inspector sit where the editor
puts them. **No pixel goldens** of the editor: fonts and anti-aliasing differ across
platforms and adapters, so a golden would break on every one but the machine that made it.

## Looking at a change (#1483)
Pixel asserts read numbers off a frame and throw the frame away: a black frame or a
flare mirrored the wrong way can pass all of them. So a branch that changes what is
drawn (the renderer, `engine/shaders/`, materials, post effects, lighting, UI, editor
panels; lighting includes what a bake writes, since lit surfaces show its texels) is
also **looked at**: its coder renders a primitive scene before (on `main`)
and after (on the branch), the feature off and on and at the extremes of the knobs the
issue is about, opens every image (an agent's `Read` shows a PNG), and fixes what is
plainly wrong. The PR carries a **`Looked at`** line saying what scene, what was
rendered and what was seen, and what was not checked (motion, Metal, cost, taste:
those stay *For a human*). It carries no image: none is committed, attached or hosted.
The images are scratch files, named with the issue number, kept outside the checkout.
The Inspector's Preview tab is looked at the same way: `make editor-capture ARGS="--select <entity> --preview Plane"` renders the tab's image through the window's own preview call, and fails rather than writing a frame where the image never rendered.
The rule and the PR line: `.claude/skills/issue-batch/cloud-brief.md`.

A pair is one command: a short scenario run by `play` on the fixture project, with
**absolute** screenshot paths (a relative one lands inside the project):

```lua
-- look_1234.lua, in the scratchpad. The fixture's camera has no post volume, and
-- with none every Graphics.* setter is a no-op: add one, or "on" renders as "off".
Scene.AddComponent(Scene.FindEntityByName("Player"), "PostProcessing")
Harness.Step(2)
Harness.Screenshot("/abs/scratch/1234_off.png")
Graphics.SetChromaticAberrationActive(true)
Graphics.SetChromaticAberrationIntensity(1)
Harness.Step(1)                          -- a write shows on the next frame
Harness.Screenshot("/abs/scratch/1234_on.png")
```

```sh
cargo run --features dev --bin play -- --project tests/fixtures/project look_1234.lua <out_dir>
```

Log the getter after a set (`Harness.Log(tostring(Graphics.Get…()))`) when the two
images look alike: an effect that did not run and one that runs invisibly look the same.

**Comparing two builds.** A built binary reads `engine/` (shaders, fonts, bundled
scripts) at run time, from the folder found beside it or above it, else the
checkout it was built in (`core::project::engine_dir`). So a binary kept across a
`git checkout` or a pull **is not that commit's build**: it runs its own code with
the other tree's shaders. Build each side in its own tree, or pin each binary to its
own copy of `engine/`:

```sh
# On main: build, then pin the binary beside a copy of the engine content it was built with.
cargo build --features dev --bin play
mkdir -p /abs/scratch/before && cp target/debug/play /abs/scratch/before/
cp -r engine /abs/scratch/before/engine
git checkout <branch>                 # the branch's own build is `cargo run`, as above
/abs/scratch/before/play --project tests/fixtures/project look_1234.lua <before_out>
```

A binary finds the `engine/` beside it first, as a shipped game does, so the pinned
copy is the one it reads; the project, scenario and output paths are still read from
where it is run. The other way is one worktree per build (`git worktree add`), each
binary run in its own. Either way, a `dev` binary knows the engine content it was
built with (`build.rs` stamps a digest of `engine/`), **warns once at start-up when
the folder it reads holds other content**, and every run report says which content
it used: `results.json`'s `engine` (`commit`, `content_built`, `content_found`,
`content_matches`), the bench report's `engine` line and `bench.json`, and
`editor-capture`'s `wrote` line. Check `content_matches` is `true` on both sides
before trusting a pair; the bench table warns on its own when the run it compares
against had `content_matches: false`. A folder that cannot be hashed (an unreadable
file, a dangling link) is said once at start-up, and its runs' `content_found` is
`null`. A cargo build re-stamps on any change under `engine/`, so
`cargo run` and `make` never warn, and the editor's shader hot reload follows edits
made after start-up; the warning is for a binary run directly against a tree it was
not built from. In a debug build `commit` is the checkout's at run time, so after a
checkout it names the new commit: the content digest is what tells a kept binary.
A shipped `player` makes no such check: it carries no `dev` code, and its `engine/`
is the one `make export` installed beside it.

## Test games (#2029)

A **test game** is a small, complete 3D game project that a Lua recipe builds from
nothing in seconds, headless. Coders, reviewers and CI prove a change on one instead
of rebuilding a scene from cubes each time, and a PR's `Looked at` line names the
game and the shots it looked at. Each lives in a folder under `games/`, and only its
recipe and scripts are committed; the project is regenerable output.

| Game | What it is for |
|---|---|
| `arena` | The CS:GO yardstick in miniature: a two-floor deathmatch from primitives (a staircase, a ladder link, a drop, a crate to jump onto, a 10 cm wall bullets go through), snowman bots on `NavMeshAgent` + `AIPerception` + `EnvironmentQuery` (its time-sliced `Request` / `Poll`) fighting everyone, raycast guns with spread and recoil, a smoke grenade that blocks sight. Scenarios: `nav` (every spawn reaches every room and both floors), `match` (bots reach the kill limit, each scores and dies), `shot` (a wallbang hurts, a 40 cm wall stops it). The recipe bakes the navmesh and saves it with the scene, so Play restores it instead of baking on its first tick. `--param size=dust2` lays out 25 blocks, 120 m a side, for map-scale work, and `--param bake=true` adds a Generate Lighting at the showroom's lowest settings (lightmaps, probes, reflections, occlusion; about a minute and a half per generation at the default size on a software adapter); both run on request. |
| `fruit` | A slice-the-fruit arcade game in 3D: rigidbody fruit and bombs thrown in bursts on a no-collision layer, a mouse swipe hit-tested on screen against the moving bodies, a cut splitting a fruit into two halves with a juice burst, lives, score, combo, a game-over banner and zimmer sounds; scenarios `slice` (injected swipe), `bomb`, `miss`, `waves`. |
| `runner` | An endless runner: a kinematic runner at rising speed in three lanes (jump, slide, lane change), a following camera, seeded chunks built ahead and destroyed behind (the entity count stays flat), crates and barrels dropped as rigidbodies that settle the same way every load, dust, sparks and a trail, a crash on any hit, a distance counter; a dodging bot (`runner/bot.lua`) drives four scenarios and two in-run shots. |
| `showroom` | Lighting and reflections in one interior room: a polished floor (SSR from a minimum smoothness of 0.1), a mirror, a clear and a tinted glass pane, a drinking glass before the mirror, a lacquered pedestal (`ior`, `KHR_materials_specular`), an emissive sign, daylight through a window, a turned box and a sphere reflection probe, six light probes and a ball rolling through them with a proxy volume. Scenarios: `load`, `ball` (its probe light rises in the sun), `probes` (the volumes drawn as gizmos), `bake` (only with `--param bake=true`: a tiny Generate Lighting, applied on load). |
| `smoke` | The generator's own proof: a floor, a crate with a Snyder texture, a looping zimmer blip and `mover.lua` sliding it along +X, a sun, a camera; one scenario asserting the crate moved, two shots. |
| `tetris` | A 3D falling-blocks puzzle: held/tapped input with repeat timing on the fixed tick, a HUD with layout groups (stats column, 4×4 next-piece grid), many small lit cubes spawned and moved every few ticks, cleared lines bursting as rigidbodies then despawning, a pulse light, zimmer one-shots and a loop; scenarios clear a line, play to game over, and replay one seed twice for the same board. |
| `topple` | Physics at scale and its determinism: four levels of box and plank stacks, jointed box figures (powered joints that go limp into ragdolls on a hard hit) and explosive barrels (`Damage.ApplyRadial` with impulse) that the player topples with thrown balls; scenarios prove the levels settle, a throw clears a level, a click throws at what the cursor is over (`click`, through the input path), a blast reaches exactly its radius, and a replay is bit-identical. `pile=16` is a 1,498-body pile. |

```sh
make game NAME=smoke                       # → target/games/smoke/ (this worktree's own)
make game NAME=smoke ARGS="--param size=40"
make game NAME=smoke OUT=~/games/smoke     # somewhere else, e.g. to open it:
cargo run --bin rusty -- --project ~/games/smoke  # the editor on it
make game-test NAME=smoke                  # generate, prove it deterministic, play, shoot
```

`make game-test` generates the game, moves it aside, generates it again at the same
path and compares every file; then runs each `scenarios/*.lua` as `play` does, on
the project's startup scene, replays the demo each run recorded (a step of its
own, `scenario <name>'s demo replays`, results in `cache/test/<scenario>/replay/`:
one that diverges fails, since a scenario's every act on the world is recorded),
and takes the shots. It prints one `ok`/`FAIL` line per
step and exits 1 when one failed. Results go to the project's `cache/test/<scenario>/`
(`results.json`, `console.log`) and the shots to `cache/test/shots/<name>.png`; open
them with `Read`. A shot fails only when it is all one colour: rendering changes too
often for a golden image to hold, so the shots are for looking. Without a GPU or
lavapipe a shot is logged as skipped, as every GPU test skips. The same command runs
locally, in a worktree and in a cloud session: every clone has `games/`, and each
worktree writes its own `target/games/`. `gpu::test_games` runs `game --test` on
every folder under `games/` in CI, one test per game so nextest runs them side by
side (#2070).

**Each game has a budget of CPU time, counted in yardsticks** for `game --test` at its
defaults (`BUDGET` in `tests/gpu/test_games/mod.rs`). The test reaps the `game` child
itself and reads its user + system time, so a busy machine slows a game's clock but
not its count. Bare CPU seconds still move with load (a shared SMT core, all-core
clocks, a contended cache), so the test divides them by a **yardstick**: the median
CPU time of the engine's own work, 60 ticks of the default scene in a fresh headless
harness (`tests/gpu/test_games/calibrate.rs`), which a test thread samples in slices
for as long as the game runs, meeting the same load in the same window. A game's CPU
is mostly its sim's tick, so a slice of the same code meets load as the game does,
where a synthetic loop met it far harder. The sampler sleeps 1.5 s between slices of
about 0.16 s (plus an untimed 0.08 s harness build), so it costs about 14% of one core
beside the game. **The yardstick cancels about four fifths of a machine's load, not
all**: on an 8-core SMT desktop `runner` reads 238 to 249 yardsticks alone and 277 to
284 beside a full-parallel cold `cargo build` (1.15×), where its CPU time rose 1.92×
(before its long run was halved; the heaviest game is now `showroom`, 216 on CI's
ubuntu, and `BUDGET`'s comment has every count). So the bar of 380 catches a
regression of about 1.6× on an idle machine and about 1.35× under heavy load; on
macOS, where the games wait on Metal more than they compute, only a CPU-bound one.
A game over budget fails by name with its CPU time and its yardsticks, so the game
whose defaults grew is the one made cheaper. Wall time is only a hang guard: a game
still running after `HANG` (450 s) is killed and fails as hung. CPU time counts every
thread, so a game that renders on lavapipe can use more CPU than wall time. Longer
runs go behind a `--param`.

**Re-measuring the budget:** every test game's test prints its line in each log, passing
too (`<game>: game --test used … s of CPU … N yardsticks of … s (… slices …)`), so
`cargo nextest run --locked --features dev -E 'test(/test_games::/)'` locally, or any
CI log, gives each game's count. The bar sits above the heaviest game's count across
the machines measured, and under twice the lowest machine's count, so a game that
doubles its work fails on every one.

**Adding a game** is adding its folder and its line in the sorted `games!` list of
`tests/gpu/test_games/mod.rs`; a folder with no line fails that file's check, naming it:

```text
games/<name>/
  recipe.lua       builds the project: scenes, materials, textures, sounds
  scripts/         the game's behaviours, copied into assets/scripts/
  scenarios/       headless play-tests, in play's scenario format
  shots.lua        returns { { name =, position = {x, y, z}, target = {x, y, z} }, … }
```

- **The recipe is Lua on the one API surface** (`docs/api/`): `Scene.New`,
  `Scene.CreateEntity`, components, `Texture.*`, `Sound.*`, then `Scene.Save` and
  `Application.SetStartupScene`. It runs in edit mode, as one console command of a
  `session` on the freshly made, seeded project (`dev::game::generate`). Something a
  recipe cannot do is an API gap: file it, never work around it in the generator.
- **Parameters** arrive as the chunk's argument (`...`), a table of the
  `--param key=value` pairs: `true`/`false`, numbers (whole ones as integers) and
  plain words. The recipe holds the defaults, which are what CI runs, and refuses a
  key it does not know (`games/smoke/recipe.lua` shows the four lines).
- **Deterministic**: the same recipe and parameters write the same bytes. Draw from
  a `seed` parameter (`Texture`'s and `Sound`'s `seed`, `Random`), never a clock.
  The check leaves a scene's bake reports (`<scene>.bake/`) out: they record when
  and how long a bake ran, by design, while the baked data they describe is compared.
- **Every test game is 3D**, and has no lightmap bake by default: a `bake`
  parameter may offer a tiny, lowest-resolution one, off unless asked for.
- The generator clears its output folder before writing, but only one it made
  (it holds `.test-game`): `OUT` naming any other non-empty folder is refused.

## Performance budgets (frame stats, #433)
A scenario can assert on performance the same way it asserts on behaviour. The harness
records frame stats every tick (see `Debug.Stats` in `docs/api/Debug.md` for the
metric list); `Harness.Stats()` returns them and `Harness.AssertBudget` turns limits
into expectations:

```lua
Harness.Step(300)                      -- 5 s of play
Harness.Screenshot("out/arena.png")    -- render counters exist only on rendered frames
Harness.AssertBudget{ draw_calls = 2000, lights_dropped = 0, fixed_update_ms = 4 }
```

- **What is checked:** each metric's **worst frame** so far must be `<=` its limit. Each
  budget is one expectation in `results.json`, in metric-name order; a failure names
  the worst frame and the average (`budget draw_calls <= 2000 exceeded: worst frame
  2412.000, avg 2398.500 over 1 frames`).
- **A metric that was never measured fails** and lists the ones that were — a
  `draw_calls` budget in a run with no `Screenshot` is a mistake, not a pass.
- **Counts are deterministic, timings are not.** Budget counters (`draw_calls`,
  `triangles`, `lights_dropped`, `entities`, …) for a regression guard that must hold on
  every machine. A `_ms` budget measures *this* machine — CI runners are slow software
  adapters — so keep it generous or for local profiling only.
- **Where the numbers land:** `results.json` gains a `stats` block with the counts only
  (`frames`, `drawn_frames` and the count metrics), so a replay stays byte-identical; `<out_dir>/stats.json` holds everything, per-system
  timings included.
- Budgets are **opt-in per scenario**, never a CI gate of their own.
- `Harness.Render()` draws the current frame without writing a PNG — the cheap way to
  get render counters and GPU time (`gpu_ms`, `gpu_passes`, #835) every tick.
- **The live window is measured too** (#2205): in a `dev` build every frame the
  editor or the player draws records its render counters, `renderer_ms`, GPU time
  and its wall-clock `wall_ms` / `fps`, so an agent attached with `session-mcp
  --attach` profiles the frame the operator is looking at with
  `Debug.Stats{ window = 120 }` (the last 120 drawn frames, edit mode's too), the
  table the Game view's **Stats** overlay shows; `Debug.Stats()` keeps Play's frames
  alone. No second process, no scenario file.

## Benchmark: `make bench` (#835)
Measure before optimising: `make bench` runs
`tests/fixtures/project/scenarios/bench/bench.lua` in the fixture project
(`play --project tests/fixtures/project`, #829), a **shooter-shaped worst case** on
the default fy_pool_day yard, and prints a short report in the terminal. The yard
is the fixture project's startup scene, `assets/scenes/default.scene`, which `play`
loads from disk (#1343). It is gitignored and seeded through the seed manifest,
which keeps a local edit, so an edited copy changes what the bench measures. A
deleted copy is not seeded again (#1618): to measure the default again, delete it
and remove its `assets/scenes/default.scene` line from the fixture's `.seeded`
(only that line: the others record the seeded scripts and starter materials).

- **the scene** — 50 skinned soldiers (a procedural 16-bone, ~12k-triangle rig with a
  walk cycle and hitboxes) walking the navmesh with a Lua brain each (patrol + a
  line-of-sight raycast per tick, `bench/enemy.lua`); 32 realtime point and spot
  lights, 8 flickering like muzzle flashes (`bench/flicker.lua`) and 6 casting
  shadows; 256 decals; four smoke columns; the sun's shadows. The
  Player stands 3 m in from its spawn, so the camera looks down the whole yard.
- **the run** — two measured windows of one run, each stepped and rendered at
  1280x720. **The quiet window**: 60 warm-up frames, then 600 measured ones
  (`Harness.LoadStress{…}` then `Harness.Bench(600)`), the yard as it stands: what
  a level costs while nothing is spawned, destroyed, re-parented or toggled, so a
  result kept until its inputs change is measured at its best. **The firefight
  window**: then `bench/firefight.lua` starts, a script whose `Update` does what
  about four automatic weapons ask every tick through the API a game uses: 4
  prefab instances spawned (`bench/casing.prefab`, a box with a Collider and a
  Rigidbody; one under a soldier's `hand_r` bone, kinematic) and 4 destroyed after
  30 ticks, 2 of 8 muzzle-flash meshes toggled (`Scene.SetActive`), 1 decal stamped
  and its oldest past 64 removed, a pickup re-parented between two holders, and 4
  shots: a `Physics.RaycastAllNonAlloc` from each shooter's muzzle to the
  Player's chest, the first hit not on the shooter's own rig taken (as a weapon
  does: its hitboxes surround the muzzle), the decal stamped where the first
  lands. After 120 frames, when its live entity and decal counts hold still,
  600 more are measured (`Harness.Bench(600, "firefight")`): the same rows, named
  `firefight.<row>` (`firefight.cpu_ms`, `firefight.cpu.follow_bones`,
  `firefight.render.<stage>`, …), where such a result's invalidation costs.
  Nothing of the firefight exists during the quiet window, so its rows compare
  with every report kept before it. The counts are constants at the top of
  `firefight.lua`.
- **the report** — per row the average and p95: `frame_ms` (the CPU time of the
  tick, `GameWorld::tick`, as measured, from after its play transition, so
  entering Play, a load, is in no tick's time: the schedule and the tick's own work
  around it), `cpu_ms` (the same at a fixed reference CPU clock, below) and
  `cpu_clock_pct` (the CPU clock it ran at), `renderer_ms` (CPU time recording the
  frame, as measured), `gpu_ms` and `gpu.<pass>` (GPU time
  at a fixed reference clock, below; `gpu_ms` is the passes' sum, busy time only,
  and `gpu.other` untimed GPU work inside a submit, where a pass left untimed
  shows, scaled like the rows), `gpu_between_submits_ms` (the GPU waiting for the
  CPU from one submit's timed work to the next's, with a command at a submit's
  edge, **as measured**: a wait does not follow the GPU's clock; the `gpu.` rows
  and it add up to the frame's timed span), `gpu_raw_ms` (GPU time as measured) and
  `gpu_clock_pct` (the clock it ran at, % of the reference), `draw_calls`,
  `triangles`, `culled_entities`, `lights_dropped`, `cluster_lights_dropped`, `lights_culled`,
  `decals_visible`, `decal_cluster_refs`, the GPU occlusion counters `hiz_tested`,
  `hiz_culled`, `hiz_late`, `cluster_tested`, `cluster_culled` (all but the two tested
  ones are read back without waiting, so they describe a frame or so earlier, `docs/api/Debug.md`), and
  the frame's GPU calls `render_passes`, `compute_passes`, `submits`,
  `bind_groups_created`, `buffer_writes` (counted through `render::gpu::ops`; a
  render-CPU fix whose timing moves inside the spread proves itself by these); then
  where the CPU time went, by name:
  `cpu.<system>` for every system the schedule runs, named as it registers them
  (`update_scripts`, `step_physics`, … at the reference CPU clock), with
  `cpu.flush_bounds` (the collider-AABB flush after each stage, summed over the
  stages) and `cpu.other` (the tick's work outside every system: publishing
  input and finished async loads, the UI textures' sizes, the camera, shader-time
  and audio sync, an async load's start, the schedule's dispatch and timer
  overhead), together summing to
  `cpu_ms`; beside them, in no sum, `taps_ms` (the demo recorder's taps, `make
  bench` only: `bench-scenes` records no demo) and `probe_ms` (the stats' own
  bookkeeping: the world counters read at the frame's end and the tick's rows
  recorded), since `frame_ms` and `cpu_ms`
  time only what a shipped, windowed game does on its tick (`docs/api/Debug.md`,
  `frame_ms`); and `render.<stage>` for every stage of the renderer's frame, summing
  to `renderer_ms`: `setup`, `assets`, `prepare`, `extract` (the retained scene's
  update), `shadows` and `shadow_gather` (the sweeps' caster gather, cull and
  batch, a lap of its own, so absent from a frame no shadow sweep ran), `render_textures`, `ui` (its layout and its draw),
  `stack`, then each per-camera pass by its name in
  `render::draw::camera::CAMERA_PASSES` (`clusters`, `solids`: the per-camera cull
  and batch, `scene`, `transparent`, `post_fx`, …), each summed over every camera
  and render-texture camera. So `render_textures` holds only what drawing the
  render textures does around their cameras: a render-texture camera's own passes
  land in `scene`, `solids` and the other pass rows, beside the screen cameras'.
  Last, `finish` (the
  frame's counters and its GPU timestamps' resolve). Each set ends with an `other`
  row, what no part held: for `render.` and `gpu.` timer overhead, a few
  microseconds, where a stage left untimed would show; for `cpu.` the tick's own
  work above. A report from before `frame_ms` timed the whole tick (#2342) has
  no `cpu.flush_bounds` and a `cpu.other` of zero, and its `cpu_ms` and
  `frame_ms` are the systems' sum, lower by what those two rows now hold: compare
  its `cpu.<system>` rows with today's, and its `cpu_ms` with today's less
  `cpu.flush_bounds` and `cpu.other`; both `make bench` and `make bench-scenes`
  warn when the kept run they compare with is such a report. A report from
  before the stats' own bookkeeping left the tick (#2445) has no `probe_ms`
  row, and its `cpu_ms` and `cpu.other` are higher by about what `probe_ms` now
  holds; both warn of that kept run too. The stages are timed only on frames the dev layer
  captures (`Renderer::time_stages`): the window, the editor and the player never
  pay for them. Every GPU pass prints every run (0 when it did not run), so the
  rows are stable. **The Hi-Z stage is also split into its parts** (#2401), on
  frames a culling camera ran it: `gpu.occlusion.select`, `.phase_one` (the visible
  set's depth draw), `.pyramid`, `.test` (the instance test and compaction) and
  `.clusters` (the cluster cull, zero on a frame that draws nothing by cluster),
  summing to `gpu.occlusion`, with `.clusters` split again into its dispatches
  (#2406): `.clusters_test`, `.clusters_scan` and `.clusters_copy`, summing to it
  and so in no sum beside it (the split adds pass boundaries only: a timed frame
  records the same dispatches, in the same order, as an untimed one): on a frame that timed any part every part is
  sampled, zero when absent, so each is averaged over the same frames and
  viewpoints as its stage; a run that never culls (or has no GPU timing) has no
  `gpu.occlusion.<part>` row at all. And `render.occlusion.hold` (making room for
  the camera, the pyramid's allocation on a resize, and collecting last frame's
  counters), `.test` (packing the instances' bounds, and copying the verdict
  count), `.clusters` (planning, uploading and copying the cluster cull's),
  `.select` (the uploads and binds before its pass, and the pass), `.phase_one`,
  `.pyramid` and `.submit`, summing to `render.occlusion` but for its returns.
  **The UI stage is split too**: `render.ui.layout`, `.textures` (loading and
  binding the Images' textures), `.mesh` (the canvas meshes and glyph atlases),
  `.sync`, `.shades`, `.uniforms`, `.masks`, `.mask_submit`, then for the overlay
  pass `.pipelines`, `.blur` (the backdrop chain alone), `.groups`, `.encode` and
  `.submit`. A CPU part is sampled on every frame its stage ran, zero on one that
  did not lap it (`.mask_submit` on a frame that drew no Mask), so its row is its
  share of the stage and the parts sum to it. A part row sits inside its stage's row, so no sum of `gpu.` or
  `render.` rows counts it. To time the parts, a frame the GPU timer times records
  the stage in a pass per part (`GpuTimer::splits`): a culling camera's stage
  records 3 or 6 compute passes (the cluster cull's three when a batch is drawn
  by cluster) where an untimed frame (lavapipe, a shipped game) records 2, and the
  extra pass boundaries' cost is in the stage's own rows. Under the table come the run's warnings, if any (below).
- **before and after** — the report is kept in `out/bench/bench.json` (and
  `bench.txt`), both windows in one, and each run prints every row's change
  against the last one, a window's table against that window's rows; a row the
  last run did not measure (the firefight's, the first time) reads "absent on the
  last run". A run stopped before a window keeps the last run's rows of it, so
  the kept report always holds each window's last complete measurement. To
  measure an optimisation: `make bench`, change, `make bench`. The report names the
  engine that measured it (`engine <commit> content <digest>`, and the last run's
  content when it moved), and warns when the binary's `engine/` is not the one it was
  built with (*Comparing two builds*, above).

It is a **release** build (the first run compiles one, a few minutes), so the numbers
are a shipped game's. It is an **informational signal, never a gate**, and not in
`make gates`: the timings are this machine's, and CI's software GPU has no timestamps
and would make them meaningless anyway. The counts (`draw_calls`, `triangles`, …) are
deterministic — same scene, same seeded scripts, fixed dt — so they match from run to
run and machine to machine; the timings compare only on one machine. Without
timestamp queries the GPU rows are absent and the report says so.

`Harness.LoadStress([opts])` takes `enemies`, `lights`, `flickering`, `shadowed`,
`decals`, `particle_systems`, `enemy_script` and `flicker_script` (an unknown key is an
error) and must run before the first `Step`, so the spawned scripts load when play
starts. `Harness.Bench(n)` works on any scenario: it steps and renders `n` frames,
prints the report, keeps it in the run's out dir and returns `{ frames, metrics =
{ <row> = { avg, p95 } }, warnings = { <text>, … } }`. `Harness.Bench(n, window)`
measures a later window of the same run (`dev::bench::window`): `window` is a
lowercase word (a-z, 0-9, `_`), measured once a run (either is an error), and
its rows are returned and kept as `<window>.<row>` beside the first window's,
its table printed under its own heading; each window's sums hold inside it, as
the first's do. A `Harness.Bench(n)` starts a new run.

### A project's own scenes: `make bench-scenes`
`make bench` measures one synthetic yard; an optimisation also needs numbers from
real levels. `make bench-scenes PROJECT=<dir> [SCENES=a,b] [RES=WxH] [RUNS=n]
[LOAD_RUNS=n] [BEFORE=<dir>] [PROFILE=1] [IMPORT=1] [SCRIPT=1] [EDITOR=0]` benches a game project's own scenes, wherever the
project is on disk (`bench-scenes`, `dev::bench::scenes`). `SCENES` names scenes by
their path under `assets/scenes/` without the extension (`levels/a`); by default
every `.scene` there, subfolders included (a folder linked twice is walked once), is
benched, in sorted order. Each scene runs in processes of its own, so its peak
memory is its own, and each of its loads runs in a fresh process. The project is opened read-only
(`Access::Run`, as `play` and the player open one) and never seeded, so nothing is
written into it; a folder missing the project skeleton gets its empty folders, as
every binary's open makes them.

- **the viewpoints**, deterministic and unauthored, in this order: every active
  screen camera the scene holds, at its own transform, as the only screen camera;
  the player's spawn (the entity named `PlayerSpawn`, else `Player`), at eye height
  (1.7 m) over the floor under it, looking the way it faces; then points on the
  navmesh at eye height, drawn from a fixed seed, each looking along yaws 0, 90,
  180 and 270°. At most 8 positions in all: the cameras first, by entity id, then
  the spawn, then navmesh points for what is left. When the cameras fill all 8,
  the report names what that left out (the cameras past 8, the spawn, the navmesh
  points), each only when the scene has it. A scene with no
  navmesh uses its cameras and spawn alone; one with none of the three looks along
  +Z from the centre of its entities' positions. The report warns in both cases,
  naming the viewpoints it used. A pose is held
  after every sim step, whatever the scripts move.
- **the run**: the scene loads, steps once and draws once (`load_ms`), `LOAD_RUNS`
  times in all (default 3), each in a fresh process: the first `LOAD_RUNS` less one
  only load it, then the bench's own load is the last. Then each
  viewpoint warms up 60 frames and measures 300, at 1920x1080 unless `RES` says
  otherwise, and the round over every viewpoint is run `RUNS` times (default 3).
  The scene's scripts run as in Play, with no input. One GPU clock probe serves
  every viewpoint of a scene. A game that calls `Application.Quit()` ends the
  measuring: the report warns, naming the viewpoint and run, and counts the frames
  really measured (`measured` in the report), which the rows are over.
- **the editor round** (`dev::bench::scenes::editor`, driven by
  `shell::editor::capture::bench`): after the loads and before the timed run, a
  process of its own opens the scene as the editor does, in edit mode and not
  standalone, so Play takes its edit snapshot and Stop restores it. Each of `RUNS`
  runs draws 60 warm-up and 300 measured frames of the editor at the bench's size,
  headless as `make editor-capture` draws it: the Scene tab with its default gizmos
  (the light probes' among them), the scene's first screen camera selected (its
  outline and gizmo draw), the Scene view posed at the scene's first viewpoint
  (above: its first camera, or the fallback after it; its position, yaw and pitch,
  while the lens stays the Scene view's own, as in the editor). Each frame is the
  windowed editor's outside Play, waited on. Then one Play entry, one Stop and one
  save. Its rows:
  - `editor.edit_ms`: the world's edit-mode frame (`GameWorld::edit_frame`: held
    conversions, the UI's texture sizes, model writes followed, skin palettes);
  - `editor.ui_ms`: `EditorUi::draw` and egui's tessellation; it and
    `editor.edit_ms` at the reference CPU clock as `cpu_ms` is (the CPU probe timed
    right before and after both; `editor.cpu_clock_pct` is the clock they ran at); the round's clock moving
    across a run, or `editor.ui_ms` spreading, warns as a viewpoint's does, and so
    does `editor.cpu_clock_pct` moving since the last run;
  - `editor.renderer_ms`, `editor.render.<stage>`, `editor.gpu.<pass>` (the GPU
    rows at the reference GPU clock) and the counters: the viewport's frame,
    sampled as a viewpoint's is, prefixed so they never mix with the game rows
    (`editor.render.handles` is the editor's gizmos, `editor.render.outline` the
    selection's). With no GPU or no timestamp queries (lavapipe) the `editor.gpu`
    rows are absent and the report says so; with no adapter the round has no rows,
    and at a size whose Scene view the editor's panels leave no room (320x180 is
    one) it draws nothing and warns;
  - `play_enter_ms` (edit mode to the first Play tick done: the snapshot, Play's
    entry, the first `FixedUpdate`; **not** the texture refresh the editor runs
    before every Play, `asset::texture_import::refresh_all`, which compresses
    changed images into the project: the round leaves it out to write nothing
    there, so on a project whose images changed the editor's Play costs more),
    `play_stop_ms` (Stop to the first edit frame drawn: the restore, the rehydrate,
    the frame, one that probes no clock and reads back no GPU time) and `save_ms`
    (the scene saved into a folder beside the reports, deleted after, as the
    editor's Ctrl+S saves the loaded scene: its bake files are copied there first
    with the world's record of them, so the save keeps them as Ctrl+S does, where
    a Save As would rewrite them): one sample a run, wall clock as `load_ms` is (a
    disk write and an allocation do not shrink with the CPU clock), each with its
    spread; `play_rss_mb` (the resident memory, Linux `VmRSS`, macOS
    `phys_footprint`, after Play entry less before it) is the **first run's** alone,
    what one Play costs a fresh editor: Stop frees the snapshot and the Play scene,
    but the pages stay resident and the later runs' Play re-uses them.

  The round's process is its own, so the game rows' `rss_peak_mb` never holds a
  Play snapshot. Nothing is written into the project: the world's store has no
  file, and Play's refresh compresses no texture, as the harness's. It costs a
  scene about one more viewpoint's measuring, `RUNS` x 360 editor frames, plus a
  load and `RUNS` Play entries, Stops and saves; `EDITOR=0` (`--no-editor`; `make bench-ab EDITOR=0`) skips it. A round
  that fails is said, and the scene is benched without it. A last run without
  the round's rows (an `EDITOR=0` run, or one from before the round) gives them
  no change column, and the rest compares.
- **the report**: every `Harness.Bench` row (`cpu.<system>` and `render.<stage>`
  included, *Benchmark*, above) over the viewpoints (the average of their averages,
  the worst p95 and the viewpoint it came from), then over the runs: the average,
  and the **spread**, the lowest to the highest run's average. Plus:
  - `load_ms`: the scene's load to its first rendered frame, wall clock, the
    average of the later loads (the second on), with their spread; the scene's
    first load in this run is reported apart as `load_cold_ms`. It is cold in the
    page cache only when nothing read the scene's files before it: after a scene
    sharing its assets, or in `bench-ab`'s second and third legs, it is one more
    warm sample. A report from before these rows holds its one first load under
    `load_ms`, so it compares with `load_cold_ms`, not with today's `load_ms`.
    With `LOAD_RUNS=1` the one load stands as `load_ms`, the first, and
    with no spread. The import cache is as the project holds it: its converted
    models are warm once any run converted them, its compressed textures only once
    the editor, an export or `make import PROJECT=<dir>` made them, since a `play` or
    `bench-scenes` run never compresses a texture. Without them every texture
    uploads decoded, as RGBA8, and the report warns (`uploaded uncompressed: N
    textures, M MB`): the scene's `load_ms`, `gpu_mb` and `rss_peak_mb` are then
    not the shipped game's. The renderer's start-up (the device and every
    pipeline, warmed as a game's renderer is) happens before the first tick, off
    the clock;
  - `load.<step>`: the load in steps, each the warm loads' average with its
    spread, which sum to `load_ms` (they tile it; `dev::bench::scenes::load`), timed
    through `rusty_app::timed` as a play-mode `Scene.Load` times its own:
    `load.document` (the scene file read and parsed, once), `load.import` (its
    meshes built, its models through the import cache), `load.apply` (the World
    replaced, its navmesh and lighting sidecars read), `load.navmesh` (the graph
    restored from the saved navmesh or baked), `load.world` (the game world, its
    Lua VM and API, the stats, the project's settings), then the first tick's Play
    entry, `load.refresh` (Play's refresh of the project's converted sources: a
    check of each cache when warm), `load.scripts` (the Lua runtime, every
    `Awake` and `Start`), `load.physics` (the rapier world) and `load.startup` (the
    `Startup` stage), `load.tick` (the rest of that tick: the first sim step) and
    `load.frame` (the first frame: its uploads and draw);
  - `load.count.<what>`: what the load did: `models_parsed` (the model files the
    scene uses: each load is a fresh process, so each is parsed once whether its
    conversion was cached or not), `models_converted` (the sources converted,
    summed over the run's loads: the first converts what is stale, so this is
    what tells a cold import from a cached one, as after a `Converter::version`
    bump), `models_cooked` (the models whose tangents, lightmap UVs and
    clusters the load computed, `asset::cook`: zero when every model's cooked
    file is current; the bench writes none, so a project an authoring build
    never imported, by `make import` or the editor, cooks every model on every
    load, and the scene's report notes it), `textures_compressed` and `textures_decoded` (the textures resident
    after the first frame, by format, the uncompressed fallbacks a slot needs
    among the decoded) and `upload_mb` (what the first frame uploaded).
    Deterministic for a project as it stands, so no spread;
  - `import_ms`, only with `IMPORT=1`, once a run (the import is the project's,
    not a scene's: every model source is converted, with headless Blender for a
    `.blend`, and every image compressed, so it is slow, minutes for a large
    project): `make import` run on a copy of the project (made beside the reports,
    deleted after) without its `cache/`, its import cache
    (`assets/generated/imported/`: converted models and compressed textures) and
    its `.git`, then the first scene's load there: what a first import and load
    cost, model conversion and texture encoding included. A row of the first scene's report, and one
    line under the summary. A link in the project is copied as what it points to,
    so the copy stands alone and the project is only ever read; a link looping
    back, and what is neither a file nor a folder, are left out. A copy or a load
    that fails leaves the row out with a warning, and the bench goes on;
  - `rss_peak_mb`: the process's peak resident memory (Linux `VmHWM`; macOS
    `task_info(TASK_VM_INFO)`'s `ledger_phys_footprint_peak`);
  - `gpu_mb`: the renderer's GPU memory after the run, once the device has
    freed what the run dropped (the clock probe). Vulkan (Linux) and DX12 read
    it from wgpu's allocator report, the blocks it reserved from the device
    (`total_reserved_bytes`: live allocations and their free space); Metal (macOS)
    from the device's `currentAllocatedSize`, which also counts what the driver
    keeps for itself, so the two platforms' figures are close, not equal. The
    report names its source, and without one (GL) the row is absent and the report
    says so. Beside it, `gpu_allocated_mb` (what of those blocks live
    allocations use, where the allocator report tells it: not on Metal) and the
    renderer's own resident bytes by kind, which follow the scene where `gpu_mb`
    does not: `gpu_texture_mb` (image textures by path), `gpu_lightmap_mb` (the
    lightmap page arrays) and `gpu_reflection_mb` (the reflection probes' cube
    array), as of the last measured frame (`docs/api/Debug.md`, the `_bytes` keys).

  It is kept as `out/bench/scenes/<project>-<hash>/<scene>.json` (and `.txt`; the
  project folder's name and a short hash of its canonical path, so a copy and its
  original keep separate histories), with the bench's console beside it
  (`<scene>.console.log`), and printed with each row's change against that
  scene's last run, under the same engine line, clock notes and warnings as
  `make bench`: a CPU clock that moved since the last run, and a CPU clock or
  `cpu_ms` that spread across a window (once per kind, naming the worst viewpoint
  and run, and how many of the measured windows had it). **A change inside the
  spread is unresolved**: when either run has a spread and the two overlap, the
  change column says `unresolved (inside the spread)` instead of a percentage. A
  run of one (`RUNS=1`) has no spread, so every change prints as measured. A last
  run of another shape (size, frames, or viewpoints: their count, or one that
  looks from elsewhere under the same label, each kept with its position, yaw and
  camera entity; within 1 mm and 0.01° it is the same one, so a float reordered
  in transform propagation or the navmesh bake still compares) is said in one warning, and the scene gets no change column.
  Console errors during the bench (a script failing every tick, a sidecar that
  failed to load) are warned of with their count and the first one, since they
  make a scene look cheaper than the game is. A scene benches as
  it is when it lacks a bake, with a warning naming what is missing: no
  lightmaps, or lightmap pages it names that are not on disk (colour, direction
  and shadowmask pages, counted: `lightmaps (43 of 43 pages not on disk)`), light
  probes placed whose SH never loaded, or reflection probes placed with no cube on
  disk (what a scene does not place is not missing). Its
  lighting cost is then not the shipped one. The run ends with a summary table of
  one line per scene: `cpu_ms` (the tick at the reference CPU clock), `frame_ms`,
  `gpu_ms`, `load_ms`, `rss_peak_mb`, `gpu_mb`, and the run's `import_ms` under
  it when asked for.
- **a fixed baseline**: `BEFORE=<dir>` compares with the kept report in `<dir>`
  instead of the last run: a copy of a project's report folder
  (`out/bench/scenes/<project>-<hash>/`, holding `<scene>.json`), or of the whole
  `out/bench/scenes/`. Copy the folder once before a batch of optimisations, and
  every PR of the batch compares with the same numbers, where the last run moves
  with every merge. The report names the folder it compared with.
- **a quality override**: `bench-scenes --set key=value` (repeatable) overrides a
  field of the running quality preset (the project's initial one) for that run
  only, through the path `Graphics.SetQualityPreset(name, { … })` takes:
  `--set gpu_occlusion_culling=true` measures the GPU occlusion cull on without
  editing `project.rusty`, which is never written. A key is a preset's field as
  `project.rusty` names it, a nested one dotted (`ssao.high.radius`); an unknown
  key fails before anything runs, listing the keys. The report prints the
  overrides under its title, and a comparison with a run of other overrides
  warns that the change is the settings' as well as the code's. `make bench-ab`
  takes it as `SET=`.
- **a sampling profile**: `PROFILE=1` adds, after each scene's timed run, a run of
  its own (the profiler's overhead stays out of the timings) under the platform's
  sampling profiler, keeping only the samples of each viewpoint's measured frames,
  warm-up excluded. On Linux, one `perf record` covers the whole scene (user
  space only, `cycles:u`, or the `cpu-clock:u` timer where there is no cycle
  counter; every thread of the process; stamped with `CLOCK_MONOTONIC`), and its
  samples are cut to the measured windows by timestamp. Its stacks are walked by
  frame pointer, which the build forces on (the 16 KiB DWARF stack copies used
  before came with recordings that lost most of their samples). A walk through
  code built without frame pointers (libc, a GPU driver) can end a stack early,
  or add a wrong caller where that code keeps other data in the frame register:
  the self time is exact, the total time is approximate. A frame with no symbol
  is named by the image it ran in (`[libnvidia-eglcore.so.…]`, `[libc.so.6]`),
  and `[unknown]` is left for a frame with no image either; `sample`'s `???` is
  such a frame too. On macOS, `sample` is attached for each viewpoint's
  measured frames, the bench's own thread (the main one) alone: `sample` samples
  every thread, parked or not, and the others' waits would fill the table. It is
  a release build with line tables and frame pointers (the `profiling` cargo
  profile, its own target folder). That binary runs the whole `PROFILE=1` run,
  so the run's timed rows come from it too, and each report says *timed on the
  profiling build*: on a desktop GPU (an RTX 2060), its `cpu_ms`, `renderer_ms`
  and `gpu_raw_ms` measured inside the release build's spread, so a `BEFORE=`
  comparison across the two builds reads true there. Kept beside the
  report: `<scene>.profile.txt`, its header (samples, the measured seconds, and
  where the samples landed by image: the bench binary, the GPU driver, libc),
  then the top 40 functions by self time (samples where the function was
  running) and by total time (samples where it was on the stack), and
  `<scene>.folded`, the raw folded stacks (`a;b;c count`) a flame graph reads.
- **script time by verb**: `SCRIPT=1` (`--script`) times every engine verb the
  scripts call (`dev::stats::verbs`, `dev::bench::verbs`), in the timed child only
  (the loads before it and a `PROFILE=1` profile beside it play without the
  timers, and every child writes under the `script/` folder below). Each Rust
  function of every namespace (`CharacterController.Move`,
  `Transform.SetPosition`, …) runs inside a timer, installed by the dev layer
  before the first Lua VM, on the thread that armed it only: the sim reads no
  clock, and a run without the flag wraps nothing. A verb written in Lua
  (`Particles.ForEach`, `GetParticles`, `SetParticles`) is timed by its Rust
  halves, which it calls through a table the timers reach
  (`script::api::RUST_HALVES`); its Lua loop, and the script's `fn` it calls, are
  `script.lua`'s. A verb's time is **exclusive**: Lua it calls back (`Move`
  raising `OnControllerColliderHit`) and the verbs that Lua calls are counted
  where they belong, not in it, and a callback that catches its own error
  (`pcall`) still ends where it returns. The report adds
  `script.<Namespace.Verb>` (its own ms per frame, at the reference CPU clock) and
  `script.<Namespace.Verb>.calls` for the 10 costliest verbs, costliest first,
  each over every frame of the scene (a viewpoint or run that never called it
  counts as zero), the rest summed as `script.other_verbs` (and `.calls`; its p95
  and spread are the verbs' summed, an upper bound), `script.calls` (every verb's
  calls per frame), `script.overhead` (the timers' own time inside
  `cpu.update_scripts`: setting the call hook, the clock reads) and `script.lua`:
  what of `cpu.update_scripts` neither a verb nor the timers hold, the VM running
  `Update` and the calls the timers do not see (a handle's methods). Two limits:
  the call hook's own work while a verb calls back into Lua lands in that verb's
  time or that Lua's, and a coroutine a verb starts (`Timer.StartCoroutine`) that yields leaves
  the rest of that verb's call counted as Lua. The verb rows count every system's
  calls (`LateUpdate`, the timer phase, callbacks), so they and `script.lua` need
  not sum to `cpu.update_scripts`. The timers cost time, so `cpu.update_scripts`
  here is not the plain run's: the reports are kept apart, under
  `out/bench/scenes/<project>-<hash>/script/`, and compare with another `SCRIPT=1`
  run; `BEFORE=` looks in the baseline's `script/` folder first, and a plain
  report, when that is all it holds, is another shape: no change column. A
  wrapped verb takes, returns and raises what the bare one does, and the run
  plays the same frames. The timer phase is its own system in every run,
  `cpu.tick_script_timers` (due `Timer.Invoke`s, resumed coroutines), beside
  `cpu.update_scripts`. A profile (`PROFILE=1`) shows where a verb spends its
  time, by function; these rows show which verbs cost the row, how often each is
  called, and its cost per call, without reading stacks.
- **a profile refuses itself** when its recording was not sound: fewer than 20%
  of its samples land in a named function of the bench binary, or it holds
  fewer than a tenth of the samples its measured seconds should give at the
  sampling rate (lost or never taken). Then `<scene>.profile.txt` and the
  console say *this profile is not to be trusted*, with each reason, its
  numbers, what perf warned of (lost chunks) and where the samples landed, and
  no tables or `.folded` are written; the run's closing lines list each refused
  scene with the start of its first reason, and the exit status stays 0 (the
  recording ran). A software renderer refuses too (lavapipe
  spends most of a frame in its own unnamed JIT code): its profile says where a
  CPU rasteriser spends time, not the engine. No profiler on `PATH` stops the
  run before anything is benched, naming the package (Debian `linux-perf`,
  Ubuntu `linux-tools-$(uname -r)`, Fedora and Arch `perf`).
  The profiler is tried once before any scene too: `perf` refusing to record
  stops the run there, saying why, and for `kernel.perf_event_paranoid` above 2
  what to set (`sudo sysctl kernel.perf_event_paranoid=2`). A profile that fails
  later is said on its own line, the scene's timings and summary row stand, and
  the run exits non-zero.

Like `make bench`, it is a release build and a signal, never a gate: it is not in
`make gates` or CI.

### A pull request's measured rows: `make bench-ab` (#2381)
A performance PR's row is measured on the operator's desktop with
`make bench-ab PR=<n> PROJECT=<dir> [SCENES=a,b] [RUNS=n] [LOAD_RUNS=n] [RES=WxH] [SET=key=value] [EDITOR=0]`,
never by hand. It runs `bench-scenes` three times back to back: on `main`, on the
PR's head, and on `main` again, each built in a detached worktree of its own under
`.claude/worktrees/` with its own `target/`, both heads read from origin and fetched
by sha (`refs/pull/<n>/head`), so no local ref sits where another git process could
move it.
**The PR's leg is its head with the measured `main` merged in** (locally, never
pushed), so the legs differ by the PR's diff alone and a PR branched before a
speed-up merged does not show it as a regression; a conflict refuses, saying to
update the branch. The table's header names `main`'s commit, the PR's head and the
merged commit. Both trees are locked (`git worktree lock`, reason `bench-ab pid
<n>`) for the run, so `make reclaim` never removes one mid-run, and an existing
tree is locked *before* it is re-pointed: a second run that `FORCE=1` took past the
timed-run lock refuses there, leaving the first run's sources alone. A lock whose
PID no longer runs is taken over by the next run and ignored by reclaim; a run that
holds the timed-run lock takes over any `bench-ab pid` lock it finds, since a power
cut can leave one behind with its PID reused (a `FORCE=1` run without the timed-run
lock refuses instead, naming `git worktree unlock <path>`). `bench-main` is kept
warm between runs (reclaim keeps it by name: its cold build is most of a run's
wait; `git worktree remove` it by hand for the disk); `pr-<n>-head` is put back at
the PR's head after the run, and `make reclaim` judges it by the PR in its name,
removing it once that PR is merged or closed, even when a power cut left it at the
run's local merge commit. **Before building, the run checks the disk**: when the
filesystem holding `.claude/worktrees/` has less free than the two release builds
still need (a tree's warm `target/release` counts as built) plus some headroom, it
refuses, naming the free space, the need, `make reclaim` and the trees whose
`target/` holds the most; `FORCE=1` does not run past it. How much is free is the
machine's: check it (`df -h`).

**Why three legs.** Not every row repeats. On a desktop the GPU rows, the memory
rows and the op counters repeat to under 1% between two takes of one binary; the
CPU rows (`cpu_ms`, `renderer_ms`, `render.<stage>`) drift by 10 to 19% between two
takes minutes apart (a `main` against `main` leg read +15% on one scene). A change
is only shown when it is larger than one binary differs from itself, so the second
`main` leg is each row's **floor**: the table prints, per scene and row, `main`'s
average, the PR's, the change, and the floor (`main` again against `main`), and a
change no larger than its floor reads *inside the floor*, beside the change it
measured. A row only the PR reports (a new
counter or stage) is listed, *absent on main*; a row the second `main` leg lacks
shows its change with the floor *unknown*. Every row gets its own floor, so
the table shows how tightly each kind of row repeated rather than assuming it.
Legs run whole, one after another: interleaving the two binaries scene by scene
halves the drift but doubles the scene loads.

**The lead.** Each scene's table opens with the rows whose change is outside the
floor, largest change first (a row from zero first), and only those that moved by
at least 0.05 (a ms row, 0.3% of a 60 Hz frame, or a count, which moves by whole
units), 50 for a µs row, or 1 for an MB or % row: the size is absolute because a
ratio is not, and a row at 0.001 ms moving 20% is nothing. After them come the
rows of that size the second `main` leg lacks, marked *floor unknown*, and every
row only the PR has, marked *absent on main*: the lead never reads "nothing
moved" above a move the table cannot judge. The full table follows.

**A load claim** reads `load_ms` and the `load.<step>` row the PR is about, never
`load_cold_ms` alone (the page cache and what ran before it decide it). It needs a spread on both sides,
so at least two warm loads a leg: the default `LOAD_RUNS=3` gives two, and a
load step of a few percent wants `LOAD_RUNS=5` or more (`make bench-ab ...
LOAD_RUNS=5`). The change must sit outside the floor and the spreads must not
overlap. The load's counts (`load.count.<what>`) are compared too: where they
differ between legs, the table's header says so, since the load rows then
compare a different load (a cold import against a cached one), not only
different code.

**What it refuses**, before it builds (`FORCE=1` runs anyway, and the table's
header lists every refusal it ran past): another timed run (an `flock` under
`out/bench/`, dropped by the kernel when its run dies), and a `cargo`, `rustc` or
`bench-scenes` process on the machine, checked again before each leg, under
`FORCE=1` too. Un-imported textures and models (the images an import would encode
now and the models with no current cooked file, `bench-scenes --pending-imports`,
the engine's own checks) are asked of `main`'s
build and of the PR's, and refused once both are built, naming the build that
found them: when both find them, `make import PROJECT=<dir>` clears it. **When the
two builds disagree** (one finds the project imported, the other does not), the PR
changes a cache's stamp (a texture's, or `COOKER_VERSION`), a texture default or the
encoder; one `.bctex` or cooked file per source holds one stamp, so no import satisfies both and the refusal says so instead of
sending you to `make import`. Measure it with `FORCE=1`: the table's header then
says `gpu_mb` and the load rows are not comparable (one leg loaded its textures
uncompressed), and only the other rows are read. Each binary is
asked its `--help` first: one without `--pending-imports` (a `main` from before it
merged) means the check could not run, refused unless `FORCE=1`; `SET=` on a binary
without `--set` refuses outright. `SET` takes dotted keys with plain values
(`SET='gpu_occlusion_culling=false ssao.high.radius=1.5'`); a JSON object value
(`{…}`) is refused: pass one key per field.

Each leg runs in a memory-capped scope of its own on Linux (`systemd-run --user
--scope`, the cap a fraction of the memory available when the run starts, printed);
without one (macOS, no user session) the legs run plain and the run says so. The
legs' reports stay under `out/bench/ab/pr-<n>/` (`main/`, `pr/`, `main-again/`),
and the table, printed and kept as `out/bench/ab/pr-<n>/table.md`, is the PR comment
as it is. Each scene's section opens with the warnings its legs' reports carry (a
missing bake, textures drawn uncompressed, console errors), each once, a warning
not every leg wrote marked with the legs that did: the table is what gets read, so
a scene benched without its bake says so there, not only in a leg's output. `make test-gpu PR=<n> [FILTER='<nextest filter>']` runs GPU tests on a
PR's head in the same `pr-<n>-head` tree, but as pushed: no `main` merged in, no
timed-run lock and none of the refusals above. It refuses a tree a live
`make bench-ab` run holds, whose sources it would otherwise swap mid-build, and a
disk without room for the `dev` test build, the largest build a tree takes (the
same check and refusal as `make bench-ab`'s). **A green run then deletes that
tree's `target/debug`**, so each PR measured does not keep its test build; a red
run keeps it warm for the fix's re-run, and the bench's `target/release` stays.
`make reclaim` works on a full disk: it judges without a git write, deletes the
build output of the trees it removes before any git command that writes, and says a
git write refused for lack of space as a full disk, naming the largest trees.

### GPU time and the GPU clock (#862)
A laptop GPU does not run at one speed. On Apple silicon the governor re-picks the
clock many times a second from how busy the GPU has been: an M4 moves between 338
and 1470 MHz during one bench run, and a fanless one on battery in Low Power Mode
settles near 1000 MHz under sustained load. The bench's frames leave the GPU idle
while the CPU steps the sim, so the clock follows the idle time, and **a lighter
frame can run at a lower clock and read slower**: #833 cut the triangles by 71% and
its raw GPU p95 rose from ~8 to ~12 ms. The same frame drawn 600 times reads
anywhere from 4.1 to 15.9 ms; its time tracks 1/clock (correlation 0.97–1.00), and
time × clock stays within 2%.

So the bench divides the clock out. A fixed arithmetic dispatch (the probe,
`dev::bench::clock`) is timed right before and right after every frame; its
duration depends only on the clock, so each frame's GPU times are scaled by
`reference ÷ probe` to **ms at the reference clock** (an M4's top clock: on that
machine, ms at full speed). That is what `gpu_ms` and `gpu.<pass>` report, and what
to compare before and after a change. Run to run it agrees within a few percent
where the raw time swings by tens of percent; read `gpu_raw_ms` for the real time
at whatever clock the governor picked, and `gpu_clock_pct` for that clock.

What it does not do: the scaling is linear, and a frame's memory-bound part does
not slow down with the GPU clock, so a few percent of noise remain, more when two
runs ran at very different clocks. A GPU-time change smaller than ~5% is not
resolved; repeat the runs alternately (before, after, before, after) to see one.
On another GPU the reference ms are that machine's own unit: compare them only with
runs on the same machine. Pinning the clock does not work: there is no user API
for it, and keeping the GPU fully busy only holds the top clock for a second or two
before the power limit pulls it back.

### CPU time and the CPU clock (#1719)
The CPU clock moves too, and the GPU moves it: on Apple silicon the two share one
power budget, so a Retina-size harness frame that keeps the GPU at full clock pulls
the CPU's down. In infinity-shout's perf audit the same sim read 1.4 to 4.9 ms a
tick across runs at a large frame, and 1.43 to 1.48 ms at 720p.

So the bench divides the CPU clock out the same way: a fixed chain of dependent
integer steps (the CPU probe, `dev::bench::clock`, ~0.1 ms) is timed right before
and right after each sim step, and the step's times are scaled by
`reference ÷ probe`. The reference is the chain's nominal time on an M4 performance
core at its top clock (4.4 GHz); it is derived, not measured there, so
`cpu_clock_pct` may read above 100, and on any machine `cpu_ms` is that machine's
own unit. The probe runs on the thread that steps the sim, and the chain does
not touch memory, so a frame bound by memory or cache does not scale with it.

**Where to read CPU numbers from:** compare `cpu_ms` and `cpu.<system>` before and
after a change, never raw `frame_ms`, and prefer a small-frame run (the bench's
1280x720) for CPU work: the smaller the frame, the less the GPU takes from the
CPU's budget. The report warns in words when its CPU numbers need care:
- the CPU clock spread more than 15% (p5 to p95, over the median) across the
  window: the clock changed under the run, so `frame_ms` is unreliable;
- `cpu_ms` spread more than 50% across the window: compare p95 with care;
- the average CPU clock moved more than 10% since the last run: the two runs'
  `frame_ms` rows measure the clock, not the work.

## API-doc drift gate
`tests/api_doc_drift.rs` (dev-only, #280) is a **hard gate** that keeps
`docs/api/` honest against the **live Lua API surface**. It boots an
empty `Session`, walks the registered namespaces via
`ScriptManager::api_surface()` (every non-stdlib global table's function-valued
keys), and parses the doc's `##` namespace headings and table rows. It then asserts
**existence parity in both directions**: every documented `Namespace.Function` is a
registered binding, and every registered binding is documented (the failure message
lists the exact offenders per namespace). The doc is served to the agent over MCP
(#288), so a drifted doc would lie to it — this is what stops that. **Scope:**
existence only. Signatures/types are out of scope, because Lua closures are opaque at
runtime; checking them would need the deferred self-describing-binding macro. The
source of truth for existence is the live surface — reconcile by editing the doc.

The **callback half** of the surface has its own gate: `tests/callback_doc_drift.rs`
(#309, both feature sets) asserts the same existence parity between the doc's
"Script lifecycle callbacks" section and `crates/script/src/scripting/callbacks.rs` — the one
list dispatch and MonoBehaviour discovery read — so a script callback can neither
be added undocumented nor advertised when the engine never dispatches it.

Both gates read the reference **assembled**: `docs/api/` is `index.md` plus one file
per namespace (#569, so branches documenting different namespaces never collide), and
`scripting::api_doc` concatenates it at run time — index first, then the files in the
order the index links them — into the one document the gates parse and MCP serves. It
is read, not embedded, so a doc edit reruns the gates without rebuilding the crate (#931).
`tests/api_doc_layout.rs` keeps the split honest: every `docs/api/<Name>.md` opens with the `` ## `<Name>` ``
heading and is linked from the index.

Existence is not correctness: an example with a renamed argument or a stale return
shape passes both gates. `tests/doc_examples/` (dev-only, #937) **runs** every
```` ```lua ```` block in `docs/api/`, each in a fresh empty `Session`, and fails
naming `file:line` and the Lua error. Fence markers opt a block down — `no-run`
(compiled only), `ignore` (not Lua) — or give it the entities it names
(`with=gun,cam:Camera`); `docs/api/index.md` lists them. A block that can run
should: prefer `with=` or completing the example over `no-run`.

## Scene-format fingerprint
`tests/scene_format/` (#941, both feature sets) pins what a `.scene` or `.prefab`
file holds. A serde tracer walks `SceneData`'s read side (every field, every enum
variant, serde aliases included) into `tests/scene_format/scene.fingerprint`, one
`path: kind` line per place a file can hold a value. A renamed, retyped, added or
removed field or variant fails the test, which lists the changed lines. Files
already saved need no migration (CLAUDE.md, *Backward compatibility is not a concern*): update the
engine's fixtures and the default scene, say what changed in the PR, then re-bless by
name: `RUSTY_BLESS_SCENE_FORMAT=1 cargo nextest run -E
'test(scene_format)'`. What serde hides from the tracer (a `#[serde(flatten)]`
struct's own fields, a type that parses a whole `serde_json::Value`) is listed by
hand in `hidden.rs`; an unlisted flatten struct fails the test by name.

## Converter fingerprints
`asset::convert::fingerprint` (#1286) holds the rule that a converter's output never
changes without a bump of its `Converter::version`, which is what tells the import
cache to re-make its files. Each converter in `CONVERTERS` converts each fixture of
its extension in `tests/fixtures/models/` twice (the two must be byte-identical),
and the hash of the `.glb` and the files written beside it is compared with
`crates/asset/src/convert/fingerprints.txt`, one row per converter, version, fixture and
program. A changed output at the blessed version fails, naming the converter: bump
its `version` in `convert/converter.rs`, then re-bless with `make bless-converters`.
A bumped version, a different Blender or an added fixture fails until re-blessed;
re-blessing a changed output at the blessed version is refused, so the bless never
stands in for the bump. The rows are Linux's (another platform's maths library may
round a float differently): `make test` holds the engine's own converters, on Linux
only, and CI's `blender` job holds Blender's rows, at the Blender it pins. Blender's
check is `#[ignore]`d rather than Linux-only, and `make bless-converters` re-blesses
its rows whenever a Blender is on the machine, writing that Blender's version: bless
on Linux with the pinned Blender, or with none installed, or the `blender` job fails
on the rows it wrote.

## Example (unit, in-module)
See `tools/lint/src/size/` — `mod.rs` mounts a `#[cfg(test)] mod tests` (`tests.rs`)
testing the size-cap selection and path normalization.

## Coverage
Coverage is measured with [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov),
accumulated over **both feature sets** (default + `dev`) so it matches what CI
actually runs. It is an **informational signal, never a merge blocker** (CLAUDE.md's
gates-vs-signals rule), surfaced where the agent acts on it.

Targets are deliberately differentiated:

- **A per-module floor on the core sim modules** (`app`, `physics`, `navigation`;
  a deliberate subset of the layer table's sim rows) — pure logic, no GPU, the part that must be right and where
  mutation/property testing is aimed — **and on `script`**, the Lua runtime and its
  `api` surface (#350), which every game script drives: the doc drift gate proves a binding *exists*, only a
  behavioral test proves it *works* (`tests/<namespace>_api.rs`, one file per
  namespace). The floors live in `coverage-baseline.txt` at the repo root (one
  `module floor` line each) and **ratchet**: raise them as coverage improves,
  never lower them silently.
- **No floor on the platform layer** (the layer table's non-sim rows) — headless
  coverage there is low-value.

It runs **weekly** (`coverage.yml`, Mondays, plus `workflow_dispatch`), never on
a pull request and not after every merge (#750): one job re-measures each floored
module and ratchets it against `coverage-baseline.txt`, landing as a job-summary
table that flags any module below its floor. A drop never fails the run; a red
run means the instrument broke, and `main-health` files it. To measure a branch,
dispatch it there: `gh workflow run coverage.yml --ref <branch>`.

Why weekly: the ratchet moves slowly, and a batch merging ~40 pull requests a day
paid for ~40 tables nobody read in between. The per-PR diff-cover comment went
with the per-PR job; the honest question "did my new code get tested?" is
answered locally, below, or by a scoped mutation run, which asks it harder.

It measures under nextest (`cargo llvm-cov nextest`, #484), so the `gpu`
group caps them like `build-test`. Checked when it was adopted: on the default
feature set it produced the same totals as `cargo llvm-cov` to the line
(70.48% regions / 70.97% lines), from ~850 small per-process profiles (~280 MB).

Run it locally: `cargo llvm-cov nextest --summary-only` (add `--features dev` for the
dev-only surface); for the lines a branch changed, `cargo llvm-cov report --cobertura
--output-path cov.xml` then `diff-cover cov.xml --compare-branch origin/main`.

## Mutation testing
[`cargo-mutants`](https://github.com/sourcefrog/cargo-mutants) audits whether the
suite actually *catches* bugs — the headline guardrail against green-but-vacuous
agent-written tests. It mutates exactly the code someone names, usually what a
branch just wrote, and reports the **surviving** mutants (a change no test failed
on). It is **non-blocking** — mutation never gates a merge — and it runs on **no
pull request** (#750) and **on no schedule** (#2369): a run exists only because
someone asked for it, so code nobody asked about is unmeasured, not clean. Auditing
a whole module is the same ask with its glob (`make mutants-remote
SCOPE='crates/physics/src/**'`), made by hand. One way in:

| Run | Trigger | Scope | Where it lands |
|---|---|---|---|
| **On request** (`mutants-on-request.yml`) | a PR's `Mutation: diff` (or globs) line, which the merge queue dispatches on `main` after the merge, two at a time (#1934); `make mutants-remote SCOPE=...`; or a dispatch with a `scope` input | `diff` (the branch against `origin/main`, or with `commit` the merged squash commit against its parent) or path globs (separated by spaces or commas; with `commit`, as from a merged PR's line, globs keep that commit's diff to the files they name and never widen it, and globs matching no changed file plan nothing and say so, green; by hand, without `commit`, they mutate the named files whole; a brace such as `crates/{core,physics}/src/**` is expanded, since git's glob dialect has none), turned by `.github/scripts/mutants-scope.py` into one `--in-diff` per cargo workspace the scope reaches: the engine's, and each directory whose `Cargo.toml` opens its own `[workspace]` (`tools/lint`, run inside it with its own `cargo test`; a cargo-fuzz crate such as `fuzz/` is named as having nothing to run), in one report (#2201); one runner, or up to four for a plan too large for one (`mutants-request.py shards`, #2283), each with a 2 h 45 budget, merged into one report; a `diff` past what four measure runs at the cap and names what it never reached, while a whole-file ask planning more than 1.5 times that is refused in the `plan` job, red, before any shard starts, with each file's mutant count largest first (`mutants-request.py refuse`); one run per branch, a newer dispatch cancelling the older and an unmerged close of the PR cancelling what is left, while a merged PR's run finishes (`mutants-cancel.yml`, #1530) | printed in the terminal by `make mutants-remote`; the run summary and a `mutants-report` artifact (`report.md`, `survivors.diff`); `report.md` also as one comment on the branch's open PR, or its merged one (the `pr` input for the queue's runs), edited in place by later runs (#912), except that a run with no answer adds its line below the earlier report rather than replacing it (#2201) |

Why this shape: mutation is a signal, and a per-PR job held runners the merge
queue was waiting on for a report that could not change whether anything merged.
Whoever writes tricky mechanism asks for exactly that code to be mutated, on the
pull request that wrote it, where someone will act on the answer. A scheduled sweep
of the whole sim held hundreds of runner-hours a week on the runners the gating CI
needs, never finished, and no survivor follow-up ever came from its report, so it
was retired (`test_on_request_only.py`, in `make scripts`, fails on any workflow
that runs `cargo mutants` on a schedule). An on-request run goes red only when it
has no honest answer (a path with nothing to mutate, a baseline that does not build,
a part whose cargo-mutants broke, reported beside the parts that finished), and then
still says why on the pull request and in `make mutants-remote`'s output, so an
unanswered ask is visible where it was made (#2201); survivors are a result. A run
stopped at its budget says how many planned mutants it never reached and is never
called clean, however its measured mutants fared; a mutant that timed out is
unmeasured too: the report never calls a run with a timeout clean, and says a run
where most viable mutants timed out is not a result.

**What a survivor means.** A survivor is a question, not a defect: is there
behaviour here that nothing asserts? Where there is (a branch, a boundary, a sign, a
formula, a state transition), a test asserts that behaviour through the callers'
interface, named for it and beside its other tests, never for the mutant or a line,
and one test holding a whole class beats a pin per instance. Where a test could only
restate the code (a constant or default, a message's wording, bench and fixture
code, code whose only effect is speed, an equivalent mutant), the survivor is left,
with no record owed. A kind that keeps coming back is excluded at the source in
`.cargo/mutants.toml`, with its reason: fixture files in `exclude_globs`, and the
`#[serde(default = "…")]` functions in `exclude_re` lines generated by
`.github/scripts/mutants_serde_defaults.py` (their value is a design choice, and
`components::registry::default_drift` holds that a dropped key loads `Default`).
No survivor count is a goal. The triage itself: `ci-merge`, *Triaging a survivor*.

**The mutation jobs run under nextest** (`cargo mutants --test-tool nextest`, #1844),
the gate's runner, with `--fail-fast` so a caught mutant stops at its first failing
test instead of running the whole suite (the profile's `fail-fast = false` is for the
gate's summary). They once stayed on `cargo test` (#484), and the baseline broke four
times on a test the gate passed, each a test leaning on a process of its own (#1042,
#1069, #1337) or, the last, two GPU tests each holding a renderer and waiting for a
second (#1844): a runner unlike the gate's finds the next such difference in every
branch's mutation run. Plain `cargo test` stays the nightly `cargo-test.yml` signal,
where a runner difference is the finding. The mutation jobs render: they install
lavapipe and set `RUSTY_REQUIRE_GPU=1` (#785), and the nextest GPU group (`gpu`, `gpu-macos` on macOS) bounds them.
Doctests are left out: nextest doesn't run them (`make test` does), so a mutant only a
doctest would catch survives. The runs pass `--test-threads 2`: the jobs' `ulimit -v`
cap (#674) is per process, so it times the test processes running at once stays within
the runner's memory (`test_gpu_config.py` holds the product). The same `ulimit` turns
core dumps off (`-c 0`, #2230): a mutant that allocates without bound aborts at the
cap, and on GitHub's runners an abort writes a core as large as the cap while the
process keeps its memory, which outlasts the test timeout and, a few mutants in a row,
the runner; with dumps off it aborts in seconds and reads as caught
(`test_limits.py` holds every capped `ulimit` to `-c 0`).

**An on-request mutant is tested by what can see it, its own crate first** (#2283).
Each mutant once ran the whole workspace's suite in nextest's order, the root
package's integration binary first and then every crate alphabetically, so a mutant in
`rusty-shell` waited 13 minutes for `rusty-shell`'s own tests and a run measured a fifth
of its plan before its budget. `mutants-on-request.yml` now runs one cargo-mutants
invocation per mutated package `P`: tested by nextest's `-E 'rdeps(=P)'` (P and every
package depending on it, dev-dependencies included), with a tool config raising
`package(=P)`'s priority so its tests run first. Before P is mutated, `cargo nextest
list` with that filter and config must succeed and select a test: cargo-mutants counts
any failing nextest run as a caught mutant, so a filter nextest refuses would report
every mutant caught. A package that fails the check is reported as unmeasured. A plan
too large for one runner (a standalone part's mutants counted in) is dealt round-robin
over up to four shards (`mutants-request.py`), whose outcomes the `report` job merges
into one report. A shard or package whose run broke is named there, its mutants counted
as never reached and the run red, beside what the rest measured.

**A torn-down job still answers.** A job whose runner died, or that was killed at its
`timeout-minutes`, runs no step of its own, so `mutants-on-request.yml`'s `report` job,
which `needs:` the others and runs unless the run was cancelled, answers for it: for
the `plan` job, or a shard that uploaded nothing, it reads the step the job died in and
GitHub's annotation from the jobs API (`mutants-torn-down.py`). A lost plan's line is
posted below any earlier report on the pull request; a lost shard is named in its
part's section beside what the other shards measured, its mutants counted as never
reached, and the run is red. What a lost job measured died with its runner.

**The jobs run their own baseline** (`--baseline skip`, #2164). cargo-mutants builds
and tests its baseline with `--package` set to the mutated packages only, whatever
`test_workspace` says, while a mutant tests the whole workspace (or, on request, the part of it that depends on the mutated crate): a member crate's
baseline timed only that crate's tests, so every mutant overran its timeout and a
member with no `dev` feature failed the baseline outright. So each job's `Baseline`
step runs what a mutant runs (the workspace build, then its tests, timed), and the
mutants' test timeout is 3× that time (at least 60 s). `--features dev` goes to
cargo-mutants itself, not after `--`, where it reached only the test phase, which then
recompiled the workspace inside the timeout. `--in-place` mutates the job's checkout,
so the mutants reuse the build the baseline made. `.github/scripts/tests/mutants/test_baseline.py`
(`make scripts`) holds both workflows' baseline commands to their mutants' arguments
(the CLI's and `.cargo/mutants.toml`'s), the mutants' `--timeout` to the `Baseline`
step's output, and the baseline to the mutants' `ulimit -v`.

Prefer `make mutants-remote` (every mutant rebuilds the crate, which is hours of
contention on a machine building sibling worktrees). Locally, the same question: time
the workspace's tests once (built first, so the time is the tests', not the build), then
pass three times that as the timeout:
```
git diff origin/main > branch.diff
cargo nextest run --no-run --workspace --features=dev   # build first, untimed
cargo nextest run --workspace --features=dev --fail-fast --test-threads 2   # the baseline: note its time
cargo mutants --workspace --test-tool nextest --baseline skip --timeout <3x secs> --features dev \
  --in-diff branch.diff -- --fail-fast --test-threads 2   # what the branch changed
```
(`--file '<glob>'` in place of `--in-diff` mutates named files; the jobs' `--in-place`
is left out here, so your worktree is never mutated.)

## Fuzzing (local-first)
The scene-load path is a parser eating untrusted input (hand-edited or corrupt
save files), so it gets a [`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz)
target in `fuzz/`. The target (`scene_deserialize`) mirrors `load_from_file`:
arbitrary bytes → UTF-8 → `SceneData` (serde) → `apply_scene_data` (the
rehydration that rebuilds meshes/colliders/skeletons), surfacing panics, hangs,
and unguarded `unwrap`s.

Running libFuzzer is **nightly-only**, so `fuzz/` is its own workspace —
deliberately out of the root build, the determinism guard and `cargo-deny` (the
size gate and the `--rules` checks still scan it). Its build is a gate all the
same: `make fuzz-check` compiles the target on the pinned stable toolchain, into
`target/fuzz`, so an engine rename it imports through turns a PR red. It builds
against `fuzz/Cargo.lock`, which is **derived, not committed** (gitignored):
`make fuzz-lock` writes it from the root `Cargo.lock`, pruned to what `fuzz/`
needs, and `make fuzz-check` runs that first, then checks `--locked`. So the
target builds the versions the root's gates prove, and a root bump needs nothing
done here; only the fuzz-only crates (`libfuzzer-sys`, `arbitrary`) come from
outside the root lock, pinned exactly in `fuzz/Cargo.toml`. It is a second cold
engine build, so CI's `fuzz-check` job and `make gates` run it only when the diff
matches `ci.yml`'s `fuzz` filter (and print that they skipped otherwise); by name
it always runs. The fuzzing itself is **local-first / on-demand**:

```
make fuzz-lock                                      # derive fuzz/Cargo.lock first
cargo +nightly fuzz run scene_deserialize          # fuzz until you stop it
cargo +nightly fuzz run scene_deserialize -- -max_total_time=60   # time-boxed
```

The committed `fuzz/corpus/scene_deserialize/` seeds the coverage-guided mutator
with the default scene plus a couple of minimal documents; new interesting inputs
accrete there. No CI job *runs* the fuzzer — fuzzing is run for as long as the
operator wants; a short time-boxed CI smoke batch could be added later as a separate
nightly workflow if regression pressure is wanted.

## Parallel agent builds & disk

The agentic workflow runs issues concurrently in isolated git **worktrees**
(`.claude/worktrees/`), and each worktree gets its **own** Cargo `target/`. Since
#483 (one test binary) and #487 (the dev profile) that is ~2.5 GB after a dev build
and test compile and ~4.5 GB once `make gates` has built every feature set — it
used to be 12–16 GB. Disk still fails loudest, as `No space left on device` at the
**link** step (the tell-tale ENOSPC), but it is rarely what runs out now.

**Cores and memory are.** Memory now peaks mid-build, not at the final link,
while the optimised dependencies (wgpu/naga, rapier, image) compile side by side.
And one cold build already fills the cores: two side by side take as long as the
same two back to back, three as long as three. Running more at once buys no
throughput — it spreads the same CPU across more builds and spends the memory
headroom, about 3 GB per uncapped cold build. An incremental rebuild after an edit
takes seconds and about 1 GB; a fresh worktree with a warm compile cache (below)
builds in well under half the uncached time.

The policy below was set from measurements on the operator's 8-core / 15 GB Linux
machine on 2026-09-30 — #497 has the raw numbers. On a different machine,
re-measure rather than trust them.

Policy when driving parallel sub-agents:

- **At most two heavy builds at once** — a cold build of a fresh `target/` or a
  `make gates` run; incremental rebuilds don't count. Serialize the rest. (A PR that
  shares files with another in-flight PR must wait anyway — see the
  serialized-merge rule in `CLAUDE.md`.)
- **Half the cores each when two may overlap** — `CARGO_BUILD_JOBS=4` on 8 cores
  costs the pair nothing and saves ~2 GB; a build alone runs uncapped (capped, it
  takes ~25% longer).
- **Check headroom first** — before a heavy build, `free -g` should show ~5 GB
  available and `df -h` ~10 GB free; if not, wait for a sibling's build or clean
  finished worktree targets first.
- **Reclaim finished targets** — once a worktree's branch is pushed/merged,
  `rm -rf .claude/worktrees/<agent-dir>/target`.

This is an orchestration policy, not a hard gate: the disk ceiling belongs to
whichever machine the session runs on, so freeing space is the other lever. CI is
unaffected (each job runs on its own runner).

## The compile cache (sccache)

A worktree's own `target/` means every new worktree compiles all ~450 dependency
crates before it reaches a line of rusty. [sccache](https://github.com/mozilla/sccache)
removes most of that (#492). It stores each compile under a hash of its inputs —
source, flags, compiler, the outputs of its own dependencies — so a later worktree
gets the same artifact back. That is **not** a shared target dir: a hit is the same
output by construction, and nothing one worktree does can overwrite another's.

**Switching it on** is opt-in, once per machine: `cargo install --locked sccache`,
then `make setup` (or `make compile-cache` alone). It writes
`.claude/worktrees/.cargo/config.toml` in the main clone — gitignored, and read by
cargo in every worktree beneath it — naming sccache as the `rustc-wrapper` and
capping the cache at 2 GB (`make setup CACHE_SIZE=4G` to change it; sccache evicts
the oldest entries past the cap). Nothing else sees it: not the main checkout, not
the user's other projects, not CI, which keeps `Swatinem/rust-cache`. Without sccache
installed, `make setup` writes nothing. Delete the file to turn it off. The cache
lives in sccache's default directory (`~/.cache/sccache` on Linux,
`~/Library/Caches/Mozilla.sccache` on macOS) and holds a few hundred MB per
dependency set, compressed — count it when checking `df -h`.

What it does and does not cache, as measured when it was adopted (#492 has the
numbers):

- **Dependencies are cached**, and so is the C that build scripts compile through
  the `cc` crate (vendored Lua among it). A fresh worktree hit ~95% of compiles and
  its cold build took about a third of the uncached time. The misses are the crates
  whose build scripts generate code into `target/` (serde, thiserror, proc-macro2)
  and everything built on them (winit, egui-winit, gltf): that path is part of the
  hash, and every worktree's path is new.
- **rusty's own crate is never cached and stays incremental.** Cargo compiles
  workspace crates with `-C incremental`; sccache declines those and runs rustc
  untouched, so `target/debug/incremental` fills as before and a one-line edit
  rebuilds as fast as without the wrapper. This was #492's rejection condition: a
  cache that switched incremental compilation off would have slowed the edit loop,
  the loop that matters most.
- **Linked things are not cached**: build scripts, binaries and proc-macros. Linking
  rusty's own binaries is the floor under a cold build.
- **Switching the wrapper on or off does not invalidate `target/`**; cargo rebuilds
  nothing because of it.
