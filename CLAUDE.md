# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

## Start here
- **README.md** — what the engine is and what you can do with it.
- **docs/** — `linting.md` (the gate), `testing.md`, `scripting-api.md` (the Lua API
  game scripts use). The Rust API reference is generated: `cargo doc --no-deps`.
- **auxmd.md** *(gitignored)* — the operator's short-term scratchpad; read it if a
  session points you there.

## Architecture — the conceptual model
A high-level map of how the engine is shaped. It deliberately doesn't enumerate every
concrete type — that inventory lives in the rustdoc reference (`cargo doc --no-deps`)
and the script surface in `docs/scripting-api.md`. Everything in the engine is one of
five kinds of moving part (Unity analogs in parentheses):

1. **Resources** — engine singletons, one per World (Unity's engine statics: `Time`,
   `Input`, the nav graph, the console, the active camera, play-state, the renderer).
   Global state the systems read and write.
2. **Components** — per-entity data, the Unity-style "classes" (`Transform`, `Mesh`,
   `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Health`, `Animator`,
   …). First-class and engine-provided; systems expect them. Every entity has exactly
   one `Transform` (mandatory, cannot be removed); all others are optional. Custom
   behaviour goes in *scripts*, never in new built-in components.
3. **Systems** — per-frame logic, a plain `fn(&mut World, &mut Resources)`, grouped
   into ordered stages (`Startup` once, then each frame
   `FixedUpdate → Update → LateUpdate → Render`). Order within a stage is the order
   modules `register` them. `FixedUpdate` is the deterministic, fixed-dt stage the
   headless harness steps.
4. **Scene & serialization** — one active scene as a serde `SceneData` document
   (references + values, no GPU buffers). Save/load replaces the World; a
   clone-on-Play / restore-on-Stop snapshot makes edit-mode authoritative, mirroring
   Unity's play-mode behaviour.
5. **The API surface** — one stable set of namespaces (`Transform`, `Input`, `Time`,
   `Physics`, `Scene`, `Animator`, `Nav`, `Health`, `Camera`, `Material`, and the
   dev-only `Debug`) shared by gameplay scripts, the console REPL, and bot-players.
   One surface, three callers — they never drift apart.

## Conventions that matter
- **ECS via `hecs`.** `Transform` is the one mandatory component; all others optional.
- **No event bus, no plugin trait.** Modules self-register via `register(&mut app)`;
  cross-system signals are direct typed returns.
- **Dev-only build profile.** The console/REPL, harness, bot-players, and `Debug.*`
  live behind the `dev` Cargo feature and are stripped from ship builds.
- **Determinism.** The sim is a pure function of (seed, inputs, fixed dt). Wall-clock
  reads and unseeded RNG are banned from the sim modules (`app`, `scripting`,
  `physics`, `navigation`); the platform layer (`main.rs`, `render`, `dev`) is exempt.
- **Use `glam`** for all math; keep egui / wgpu / mlua decoupled.
- **Single crate.**

## Commit gate (programmatic — no AI needed)
Commits are blocked unless the checks pass; failures are written to
`.lint/report.txt`. See **docs/linting.md**.
- Size gate: `cargo run --manifest-path tools/lint/Cargo.toml` (files ≤ 300 lines,
  test/fixture files ≤ 150). Style is rustfmt; **clippy is a hard gate** in CI (`-D
  warnings`, both feature sets).
- **Determinism guard:** `cargo run --manifest-path tools/lint/Cargo.toml --
  --determinism` — fails on wall-clock / unseeded RNG in the sim modules (`app`,
  `scripting`, `physics`, `navigation`); it protects the harness's reproducibility.
- `tools/lint/baseline.txt` grandfathers the files that currently exceed the size
  cap. It's a **burn-down list** — remove entries as you split them, never add to it.
