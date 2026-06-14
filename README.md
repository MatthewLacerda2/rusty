# rusty engine

A 3D game engine inspired, made entirely without me looking at the code!

The runtime model is a deliberate copy of Unity (GameObjects, components, scripts
with `Update()`). What's *new* — and the reason this README spends most of its
words below the fold — is an **agentic dev layer**: tooling so a coding agent can
drive, observe, and play-test the game headlessly, without a human watching a
window. That part is uncharted, so it's documented in detail; the Unity-shaped
parts are kept brief on purpose, because they're standard game-dev knowledge.

> **Status:** this tree is currently a **scaffold**. The folders and `.rs` files
> below exist with doc headers describing their responsibility and allowed
> dependencies, but the logic has not been migrated into them yet. The working
> engine still lives in the legacy modules (`core/`, `render/`, `physics/`,
> `scripting/`, `navigation/`, `editor/`); see *Migration* at the end.

---

## Architecture at a glance

Single crate for now (we split into a Cargo workspace later, once boundaries are
proven). The hard rule: **the simulation knows nothing about rendering** — that's
what makes headless play possible.

```
src/
├── app/          App, Schedule, Stages, Resources — the frame loop (replaces the god-loop in main.rs)
├── ecs/          hecs World wrapper, generational entities, deferred commands
├── time/         Time resource + deterministic fixed-timestep clock
├── transform/    Transform component + parent/child hierarchy
├── components/   First-class built-in components (Mesh, Collider, Rigidbody, NavMeshAgent, …)
├── scene/        Save/load + snapshot: SceneData document, single active scene, clone-on-Play
├── api/          The STABLE engine API — shared by Lua scripts, the console, and bots
├── dev/          THE AGENTIC LAYER (dev-only, compiled out of ship builds)   ← see below
│
│  (legacy, being migrated:)
├── render/  physics/  navigation/  scripting/  editor/  core/
└── main.rs       Builds the App, registers each module's systems, runs it
```

---

## The Unity-shaped parts (brief — this is known territory)

- **ECS via [`hecs`].** Entities are generational handles; components are plain
  data; systems are functions that query component sets. This replaces bare-`u32`
  IDs (and hardcoded `Player == 2` assumptions) and the `Rc<RefCell>` graph.
- **First-class components.** Built-in components ship *with* the engine and its
  systems expect them — e.g. the engine bakes a navmesh and the `NavMeshAgent`
  component interfaces with it, exactly like Unity. Custom behaviour goes in
  scripts, not new built-in components.
- **Scripts = MonoBehaviours.** A scene script is a Lua table with lifecycle hooks
  (`Start`, `Update(dt)`, `FixedUpdate`, `OnTriggerEnter`, `OnDamage`, `OnDestroy`).
- **Resources.** Global singletons (Time, Input, NavGraph, Console) live in the
  World as resources — Unity's engine statics.
- **Schedule / Stages.** Systems run in ordered stages each frame
  (`FixedUpdate → Update → LateUpdate → Render`) — Unity's execution order, and the
  source of the fixed-timestep the harness drives.
- **No event bus, no plugin trait.** Deliberately. Cross-system signals (a hit, a
  trigger) are direct typed returns, not `SendMessage`-style indirection. Modules
  wire themselves up with a plain `register(&mut app)` fn, not a `Plugin` trait.
- **Scenes.** One active scene (no multi-scene / open-world); loading replaces the
  World. The on-disk format is a serde `SceneData` document storing *references +
  component values*, never GPU buffers (rehydrated on load). **Edit-mode is
  authoritative:** Play runs on a clone and Stop restores it, so saving always
  persists your edit state. Double-click a scene in the assets browser to load it.
  There's always a checked-in default scene (`assets/scenes/default.scene`, seeded
  into `project/scenes/` on boot) — the bot-chase demo with a sun and a non-white
  skybox.

Unity → rusty quick map: `GameObject/Transform` → entity + `Transform`;
`MonoBehaviour` → Lua script; `Rigidbody/Collider/Camera/Light/Animator/NavMeshAgent`
→ components of the same name; `Time/Input/Physics/SceneManager/Debug` → API modules
in `api/`.

---

## The dev-only build profile ("won't ship")

Unity strips `Debug.*` and editor-only code from release builds. We do the same
with one concept — a **`dev` Cargo feature**:

- **Rust:** dev modules are `#[cfg(feature = "dev")]`. Build the game without the
  feature → that code never compiles in.
- **Lua:** "won't ship" means the dev API tables (`Debug`, `Harness`, writable
  `Input` for bots) aren't *registered* in a ship build, and scripts tagged
  dev-only aren't loaded. A bot/dev script in a shipped game simply has nothing to
  bind against.

The "decorator" you put on a script is just a `dev_only` flag on its
`ScriptComponent` that the loader honours. (When we later add a `#[derive(Inspect)]`
macro for Unity-style field attributes like `[Range]`/`[Tooltip]`, it lives behind
this same feature.)

---

## THE AGENTIC LAYER (`src/dev/`) — the detailed part

No Unity equivalent. This exists so an agent can **play the game and report back**
instead of you opening the app every time. Four pieces, all speaking the *same*
`api/` surface as gameplay.

### Why it can exist
The simulation is pure CPU (Lua, nav, physics) and has **zero render dependency**,
so it can be stepped with no window. Rendering is the only GPU-bound part, and it's
reached only for screenshots.

### The agent loop
```
write  project/scenarios/<name>.lua          (a scenario = a normal dev-only script)
run    the headless harness on it
read   results.json + console.log + *.png     (PNGs are viewable directly by the agent)
```
That's it: *write file → run → read files.* No live process to babysit, no sockets,
fully reproducible.

### 1. Console + live REPL — `dev/console.rs`
One type, two jobs: a **log sink** (info/warn/error) and a **Lua REPL** that
evaluates a line against the *live* runtime. The same evaluator backs the in-editor
terminal panel (type while the game plays) and the headless harness, so they can
never drift. "Call the API live" literally means feeding a string to this evaluator.

### 2. Harness — `dev/harness.rs` (the agent's hands)
Builds an App with **no render systems** and runs the fixed clock as fast as the CPU
allows. Control surface:

| Call | Meaning |
|---|---|
| `Harness.Step(n)` | advance exactly `n` fixed ticks (default `dt = 1/60`) |
| `Harness.StepUntil(pred, max)` | run until a Lua predicate is true or `max` ticks — the **"skip ticks"** primitive |
| `Harness.Snapshot()` | dump world state as JSON (id/name/pos/rot/health/clip, camera, play-state) |
| `Harness.Log(msg)` / `Harness.Expect(cond, msg)` | record observations + pass/fail asserts |

**Token efficiency:** `StepUntil` is the key idea — run an entire match at full
speed and observe **once** at the end, spending one observation instead of hundreds
of per-frame reads. Ten seconds of game time computes in milliseconds.

**Determinism contract:** fixed timestep + frame-count-based logic only (no
wall-clock `Instant` reads inside the tick). A scenario therefore replays
identically every run — which is what makes an agent-reported bug reproducible by
you.

A run writes `<out_dir>/results.json` + `<out_dir>/console.log` + any screenshots.

### 3. Screenshot — `dev/screenshot.rs` (the agent's eyes)
The only place the dev layer touches the renderer. Creates a wgpu device with no
surface, renders one frame to an offscreen texture, copies it back, and writes a
PNG. Lets the agent actually *see* a frame and critique lighting / SSR / shadows
against the CS1.6 → FEAR → Trepang2 bar.
*Caveat:* needs a GPU or software adapter (e.g. lavapipe) in the container; the rest
of the dev layer needs no GPU.

### 4. Bot-player — `dev/botplayer.rs` (a pattern, not a subsystem)
A bot-player is **a normal dev-only script** attached to the Player that drives
**writable Input** (`Input.Press/SetAxis/SetLook`) from its `Update()` — pressing the
same keys a human would. Run it headless via the harness and read the summary.
Writable input is the one new primitive that makes "play as the user" possible; the
Player controller reads an Input state and doesn't care whether a human, a scenario,
or a bot wrote it.

### Example scenario
```lua
-- project/scenarios/hitscan_check.lua  (dev-only)
local enemy = Scene.FindEntityByName("Enemy_1")

Input.Press("W"); Harness.Step(120)            -- walk forward 2s @ 1/60
Debug.Screenshot("01_after_walk.png")

Harness.Shoot(); Harness.Step(1)
Harness.Expect(Health.Get(enemy) < 100, "hitscan should damage the enemy")
Harness.Log("enemy hp = " .. Health.Get(enemy))
```

---

## Migration (legacy → scaffold)

The engine still runs from the legacy modules; the scaffold is the target the logic
moves into, in this order (each step keeps the app building):

1. **Decouple the tick + abstract input** → `app/`, `time/`, writable `api/input.rs`.
2. **Adopt `hecs`** → `ecs/`; move components from `core/scene.rs` into `components/`.
3. **Scene save/load** → `scene/` (SceneData document, clone-on-Play); externalise the
   procedural demo into `assets/scenes/default.scene`.
4. **Formalise the API** → `api/` (add `Health`, `Time`, `Raycast`/`Shoot`, `Camera`).
5. **Build the dev layer** → `dev/` (console/REPL, harness, screenshot, bot-player).
6. **Split into a Cargo workspace** once the boundaries above hold.

[`hecs`]: https://crates.io/crates/hecs
