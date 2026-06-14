# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

## Start here
- **README.md** — how the engine works (brief on the Unity-shaped parts, detailed on
  the agentic dev layer).
- **ARCHITECTURE.md** — inventory of resources, components, systems, and classes
  (each tagged `[now]` / `[new]` / `[partial]`).
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
  test/fixture files ≤ 150). Function length + code smells are clippy's job
  (`clippy.toml`); style is rustfmt (`rustfmt.toml`).
- `tools/lint/baseline.txt` grandfathers the legacy monoliths. It's a **burn-down
  list** — remove entries as you split/migrate, never add to it.

## Status
The engine currently runs from the legacy modules (`core/`, `render/`, `physics/`,
`scripting/`, `navigation/`, `editor/`). The `app/`, `ecs/`, `scene/`, `api/`, `dev/`
trees are scaffolded and being migrated into — see README's *Migration* section.
