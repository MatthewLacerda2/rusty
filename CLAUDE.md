# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

## Start here
- **README.md** — how the engine works (brief on the Unity-shaped parts, detailed on
  the agentic dev layer).
- **ARCHITECTURE.md** — the module map + conceptual overview of the engine.
- **docs/** — `linting.md` (the gate), `testing.md`, `scripting-api.md` (the Lua API
  game scripts use). The Rust API is generated: `cargo doc --no-deps`.
- **auxmd.md** *(gitignored)* — the operator's short-term scratchpad; read it if a
  session points you there.

## Conventions that matter
- **ECS via `hecs`.** `Transform` is the one mandatory component; all others optional.
- **No event bus, no plugin trait.** Modules self-register via `register(&mut app)`;
  cross-system signals are direct typed returns.
- **Dev-only build profile.** The console/REPL, harness, bot-players, and `Debug.*`
  live behind the `dev` Cargo feature and are stripped from ship builds.
- **Use `glam`** for all math; keep egui / wgpu / mlua decoupled.
- **Single crate** for now; split into a Cargo workspace later.

## Commit gate (programmatic — no AI needed)
Commits are blocked unless the checks pass; failures are written to
`.lint/report.txt`. See **docs/linting.md**.
- Size gate: `cargo run --manifest-path tools/lint/Cargo.toml` (files ≤ 300 lines,
  test/fixture files ≤ 150). Style is rustfmt; **clippy is a hard gate** (`-D
  warnings`, both feature sets).
- **Determinism guard:** `cargo run --manifest-path tools/lint/Cargo.toml --
  --determinism` — fails on wall-clock / unseeded RNG in the sim modules (`app`,
  `scripting`, `physics`, `navigation`); it protects the harness's reproducibility.
- `tools/lint/baseline.txt` grandfathers the legacy monoliths. It's a **burn-down
  list** — remove entries as you split/migrate, never add to it.

## Status
**Phase 2 complete & merged.** `app/`, `ecs/`, `components/`, `scene/`, `api/`,
`dev/` are implemented: hecs ECS storage, the headless harness + `play` binary, the
scripting API (Health/Time/Camera/Physics/Input/Debug), scene save/load
(`assets/scenes/default.scene` + clone-on-Play), offscreen screenshots, the
console+REPL, the bot-player, and rapier3d physics. `main` is branch-protected
(CI green + up-to-date required to merge).

**Resuming a fresh session?** The open GitHub **PRs and issues are the live to-do
list** — list them first. Phase 3 (human-overseen) currently covers: particles,
post-FX (the UI-only "dead knobs"), raycast unification, de-hardcoding
gameplay→scripts, and the kinematic character controller. The de-hardcode issue
moved the player controller / weapon / damage into bundled Lua scripts and removed
the winit-only `space_pressed`/`mouse_left_clicked` input fields and the
`"Antigravity"` boot print; the remaining `entity.name == "Player"` branches in
physics/render are the last weed to root out there.
