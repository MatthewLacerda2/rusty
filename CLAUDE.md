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

## How we work
- **The gates (push back before you build).** An idea becomes an issue only when all
  three hold; if any fails, **push back instead of complying**:
  1. **Understanding.** Claude actually understands the idea — the user has a clear
     intent and Claude can restate it. If unsure, restate it back and confirm before
     proceeding; don't guess.
  2. **Value.** The issue adds real value to the project. No busywork, no features for
     their own sake.
  3. **Craft.** It follows Rust/Lua good practices and the gold standards of game-engine
     design and architecture. If it doesn't, say so and propose the right shape.
- **Flow:** discuss the idea (if needed) → write a GitHub issue for it → batch it (mark
  dependencies) → implement it in a pull request → merge. New work starts as an issue,
  not as a surprise diff; the PR references the issue it closes.
- **Infrastructure-first (NOT "make it up as we go").** We do **not** improvise or pile
  on features ad hoc. Whenever we find a problem — something that already bites or will
  bite more than once, a pattern worth adopting, or a gold-standard practice we should
  have had — we **document it and implement it right away**, before continuing. We do
  **not** have the right to add more features/shenanigans until the
  infrastructure/architecture itself is improved first. Fix the foundation, then build
  on it. Each such fix gets its own issue when it carries its own responsibility.
- **Batching & dependencies.** Once an issue is written, record how it relates to the
  others so sessions can fan out subagents on independent issues in parallel. In the
  issue body add a `## Dependencies` block with `Blocked by: #N` / `Blocks: #N` lines,
  and use GitHub **sub-issues** when one issue is literal groundwork for another. Two
  issues block each other when one **lays the groundwork** for the next, **makes it
  meaningfully easier**, or would **conflict too much** if done concurrently. Use your
  best judgment.
- **Pull requests.** Each PR's description says **what changed and why**, not how you
  got there — include process only when it's needed to understand the diff. Open it
  **ready for review** unless you need help, in which case leave it a **draft and say
  why** (blocked on another issue, needs a brand-new PR, software/hardware constraints,
  human intervention, etc.).
- **Merging — serialized, one at a time.** When a PR is ready, CI/CD runs; once it's
  green you may merge to `main` right away. If it conflicts or fails, fix it until it
  passes; if it's taking too many iterations (≈3 fix attempts at the same failure, or a
  failure that needs a decision you can't make), mark it a **draft** and hand it to the
  user. Because Rust is compiled, two PRs can each be green alone yet break `main`
  together (a rename, a changed signature, a moved module — no textual conflict catches
  it). So merging **cannot be parallelized**: rebase each PR onto the latest `main` →
  CI green on that rebased state → merge → repeat, one PR at a time. The only exception
  is a PR that touches **only** Markdown.

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
