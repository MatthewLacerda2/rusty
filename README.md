# rusty engine

A 3D game engine built **without me ever looking at the code** — every line is
written by an AI coding agent.

rusty copies Unity's runtime model — GameObjects, components, and scripts with an
`Update()` loop — so if you've used Unity, you already know how to think in it. The
difference is *why* it exists: rusty is made to be **driven by a coding agent**, so
you build the game from your IDE (Claude Code, ideally) instead of clicking around an
editor — and a lot faster.

rusty is, in most ways, a **subset of Unity** — the traditional engine features you need
to build a game, and not the long tail you don't — but with **full Claude support**:
Claude (and Claude Code) can create scenes, edit GameObjects and assets, and even
playtest, all through one API. Developers drive the engine by talking to Claude rather
than writing code by hand.

The north star is a game on par with **F.E.A.R.** (2005) or **Trepang2** — visceral
first-person combat carried by reactive enemy AI. The engine is "done enough" when an
agent could build a shooter of that caliber on it.

## What it is

- **Unity-shaped.** Entities each have a `Transform` plus optional components (`Mesh`,
  `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Animator`,
  …). Behaviour lives in Lua scripts that act like MonoBehaviours, with lifecycle
  hooks (`Start`, `Update(dt)`, `OnTrigger`) — see
  [`docs/scripting-api.md`](docs/scripting-api.md#script-lifecycle-callbacks).
- **One scene at a time.** A scene is saved to disk as a plain document of references
  and values. Entering Play runs on a *clone*, and Stop restores your edits, so
  edit-mode is always what gets saved.
- **Scriptable against one stable API.** Gameplay calls a single set of namespaces —
  `Transform`, `Input`, `Time`, `Physics`, `Scene`, `Camera`, `Nav`,
  `Animator`, `Material`. See [`docs/scripting-api.md`](docs/scripting-api.md).
- **A real 3D engine underneath:** rendering, physics (rapier3d), navigation/navmesh,
  shadows, a skybox, and a post-processing chain.

## Assets

Authored 3D assets come from **Blender** (its native glTF 2.0 export), or are
`glTF` / `glb` / `obj` / `fbx` files brought in, downloaded, or exported from
anywhere else. The engine reads those standard interchange formats directly — it
**never parses `.blend` and never shells out to Blender** as a subprocess (the
fragile Unity-style convenience that breaks the moment Blender isn't installed).
glTF 2.0 is the first-class path; `.obj` covers static meshes.

## Built to be played by an agent

The headline feature: the entire simulation runs **headlessly** — no window, no GPU —
because the sim knows nothing about rendering. So a coding agent can play your game
and report back instead of you opening the app every time. Four pieces, all speaking
the same API as gameplay, all compiled out of a shipped build (the `dev` feature):

- **Harness** — steps the game at full speed for as many ticks as you want, then dumps
  world state as JSON. Ten seconds of game time computes in milliseconds, and a fixed
  timestep makes every run reproducible.
- **Bot-players** — ordinary scripts that press the same keys a human would, so the
  agent can "play as the user".
- **Console + REPL** — evaluate API calls against the live game; the same evaluator
  backs the in-editor terminal and the headless runs, so they never drift.
- **MCP bridge** — the same evaluator spoken as the Model Context Protocol, so Claude
  Code attaches to a live edit-mode session natively and drives it like Blender-MCP
  drives Blender (see [`docs/mcp.md`](docs/mcp.md)).
- **Screenshots** — render a single frame offscreen to a PNG so the agent can actually
  *see* and critique a frame.

The payoff: you can trust your IDE — preferably Claude — to write, run, and play-test
the game for you, and only open a window when you want to.

## Running it

- `cargo run` — the editor and game window.
- `cargo run --bin play --features dev -- <scenario.lua> <out_dir>` — the headless
  harness; writes `results.json` + `console.log` (and any screenshots) to `<out_dir>`.
- `cargo run --bin session-mcp --features dev` — drive the live engine from Claude
  Code over MCP (see [`docs/mcp.md`](docs/mcp.md)).
- `cargo doc --no-deps` — the Rust API reference.

For the engine's architecture and the conventions agents follow, see
[`CLAUDE.md`](CLAUDE.md); for the commit gate, testing, and the Lua scripting API, see
[`docs/`](docs/).
