# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

## North star
The bar rusty aims for is a game on par with **F.E.A.R.** (2005) or **Trepang2** —
visceral first-person combat carried by reactive enemy AI. That is the quality target
every decision serves: the engine is "good enough" when an agent could build a shooter
of that caliber on it. Keep this goal in mind when weighing features, architecture, and
craft — it is the reason the gates below are strict.

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
- **Flow:** discuss the idea (if needed) → (usually) write a GitHub issue for it → mark
  its dependencies → implement it on a branch → open its PR → merge. New work normally
  starts as an issue, not a surprise diff, and the PR references the issue it closes.
  **Issue-less PRs are allowed only** for documentation updates or bug fixes; everything
  else starts as an issue. Either way the PR description still has to clear the three
  gates above.
- **Pull requests — open early, draft until ready.** The moment a branch has its first
  commit, **open a PR for it** — you don't wait to be asked; that's the rule, so a branch
  is never a stray with no documented purpose and never goes stale unnoticed. Then set its
  state, and flip between the two as the work moves:
  - **Draft** while the work is still in progress, or whenever you're **blocked or need
    something from the user** — a decision you can't make, another PR/issue that must land
    first, or human/hardware intervention. **Say why** in the description.
  - **Ready for review** once the work is done and you need nothing further from the user.
    That is the signal it can be reviewed and merged.
  The description says **what changed and why** — not how you got there (include process
  only when it's needed to understand the diff) — and carries the PR↔issue link when there
  is one. That description, the commit history, and that link are how we see what a branch
  adds in value and whether it still earns its place.
- **Branch naming.** A PR that closes an issue uses `{issue_number}-short-slug` (e.g.
  `163-fix-coverage-scope`). An issue-less PR uses a readable short slug of its subject
  (e.g. `document-ai-parity`). Lowercase-hyphenated, brief.
- **Merging — serialized, one at a time.** When a PR is ready, CI/CD runs; once it's
  green you may merge to `main` right away. If it conflicts or fails, fix it until it
  passes; if it's taking too many iterations (≈3 fix attempts at the same failure, or a
  failure that needs a decision you can't make), mark it a **draft** and hand it to the
  user. Because Rust is compiled, two PRs can each be green alone yet break `main`
  together (a rename, a changed signature, a moved module — no textual conflict catches
  it). So merging **cannot be parallelized**: rebase each PR onto the latest `main` →
  CI green on that rebased state → merge → repeat, one PR at a time. The only exception
  is a PR that touches **only** Markdown — CI is skipped for Markdown-only PRs, so they
  needn't be serialized and can merge freely.
- **Architecture- then infrastructure-first (NOT "make it up as we go").** We do **not**
  improvise or pile on features ad hoc. Whenever we find a problem — something that
  already bites or will bite more than once, a pattern worth adopting, or a gold-standard
  practice we should have had — we **document it and implement it right away**, before
  continuing. We do **not** have the right to add more features/shenanigans until the
  architecture/infrastructure itself is improved first. Architecture (how things are
  organized) outranks infrastructure (the tools to build them), and both outrank features
  — see *Issues, labels & priority* for the full order. Fix the foundation, then build on
  it. Each such fix gets its own issue when it carries its own responsibility.
- **Dependencies (not batches).** Once an issue is written, record how it relates to the
  others using GitHub's native issue **relationships** — set `Blocked by` / `Blocks`
  directly on the issue, and use GitHub **sub-issues** when one issue is literal
  groundwork for another. Two issues are linked when one **lays the groundwork** for the
  next, **makes it meaningfully easier**, or would **conflict too much** if done
  concurrently; use your best judgment. There are no rigid batches: the dependency graph
  *is* the plan. Any issue with no open blockers is fair game, and independent issues can
  be worked in parallel — stay flexible and efficient.
- **Gates vs. signals — block on correctness, inform on quality.** Keep the bar high
  *without* stalling the agents. A check that proves **correctness** — build, test, clippy,
  the determinism guard, the size gate — is a **hard gate**: green-to-merge, no exceptions. A
  check that *audits quality* — mutation testing, coverage — is an **informational signal**:
  scoped to the diff per-PR for a fresh-context catch and run as a periodic full sweep on
  `main` for the backstop, but it **never blocks a merge**. Nothing may silently slide, so an
  informational signal only earns its keep when it's **surfaced where the agent acts on it** (a
  job summary or PR comment read in-context), not buried in an artifact nobody opens. Don't
  reach for a hard gate where an informational signal does the job.
- **Agent velocity is first-class.** This workflow is agents driving the engine and each
  other, often unattended — throughput counts. Write code that is **readable by design and
  lean**: not for style points but because clear, well-shaped code is cheaper to reason about
  (fewer tokens, fewer wrong turns) and faster for the next agent to extend. Strip avoidable
  blockages and keep CI fast. This is part of **Craft**, not a trade-off against it.

## Issues, labels & priority
- **Issues come before PRs.** The unit of work is a well-specified issue — a clear
  statement of *what* to set up and *what* to do (the roadmap, not the implementation
  intrinsics). That's what lets Claude Code pick an issue up and run it unattended, even
  overnight. So defining issues well outranks opening PRs: get the roadmap right and the
  doing is the easy part.
- **Never file an issue and start it in the same breath** — unless the work is a *direct
  consequence* of another, already-decided issue. Filing-then-immediately-implementing
  defeats planning: an idea still being shaped has to settle before anyone codes it.
- **Issue-less PRs are allowed only** for documentation updates or bug fixes; everything
  else starts as an issue.
- **Priority by label.** When choosing what to do next, the order is
  **architecture → infrastructure → bug → feature.** Architecture comes first because it
  defines how things are organized and so reshapes — or eases — everything downstream;
  infrastructure (the tools to do the work) is next; then bugs; then features.
  **documentation** can be done at any time and never waits its turn.

### Labels
- **architecture** — How we define stuff.
- **bug** — Something isn't working.
- **burn-down** — *(no description yet)*
- **documentation** — Improvements or additions to documentation.
- **feature** — Feature or improvement.
- **foundation** — *(no description yet)*
- **human** — AI can't do this end-to-end.
- **infrastructure** — Laying groundworks.
- **plan** — *not yet defined.*

Issues tagged **plan** are still being discussed with the user. They must **NOT** be
started by any means. If a planning issue would implement something that affects another
issue — changing how it gets implemented, or even how it's thought of — that other issue
must be marked **blocked by** the planning issue.

## Architecture — the conceptual model
A high-level map of how the engine is shaped. It deliberately doesn't enumerate every
concrete type — that inventory lives in the rustdoc reference (`cargo doc --no-deps`)
and the script surface in `docs/scripting-api.md`. Everything in the engine is one of
five kinds of moving part (Unity analogs in parentheses):

A **GameObject** is one entity (`Entity` in `components/entity.rs`): a mandatory
`Transform` plus any mix of the optional first-class components below, and a place in
the parent/child hierarchy. It can be **empty** — a Transform and nothing else, used as
a grouping pivot or a spawn marker. Or it can be configured: instantiating a glTF
yields a GameObject carrying a `Transform`, a `Mesh`, a `Collider` when the asset
provides one, and the engine's default `Material`. Same object, more components — that
is the only difference between an empty marker and a fully-dressed enemy.

1. **Resources** — engine singletons, one per World (Unity's engine statics: `Time`,
   `Input`, the nav graph, the console, the active camera, play-state, the renderer).
   Global state the systems read and write.
2. **Components** — per-entity data, the Unity-style "classes" (`Transform`, `Mesh`,
   `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Health`, `Animator`,
   …). These are the engine's **first-class components** — engine-provided, systems
   expect them, and each must satisfy the four axes the completeness gate enforces
   (see `docs/linting.md`). Every entity has exactly one `Transform` (mandatory, cannot
   be removed); all others are optional. Custom behaviour goes in *script components*
   (Lua MonoBehaviours), never in new first-class components. Script components are
   **not** first-class — even the ones shipped in the engine's own Lua — so the
   four-axis completeness gate never applies to them; it discovers first-class
   components solely from `Entity`'s `Option<…Component>` fields. New first-class
   components are added in Rust, not script.
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
- **Unity is the reference; rusty is a deliberate subset.** Use Unity Engine as the
  yardstick for what rusty *must* be capable of. Unity is exhaustive, so we implement
  only the subset a game actually needs — never feature-for-feature parity. The twist is
  that our API exists to be driven by **Claude Code**, not hand-written: developers won't
  write or even read the engine's code. As long as the docs stay current, Claude can tell
  the developer how anything is done — so keeping documentation truthful is load-bearing,
  not a nicety.
- **AI-driven, editor↔API parity.** rusty is built for an agent-driven workflow in the
  *Claude Code + Blender-MCP* style: the agent can do anything a user can do in the
  editor — create entities, place and configure components, instantiate assets, save
  scenes — **except pure UI chrome** (collapsing a card, resizing a panel). Every
  user-facing editor capability has an API equivalent.
- **Keep the API doc in lockstep.** When you add or change an API function, update
  `docs/scripting-api.md` in the *same* change. That doc is the API reference the agent
  reads to drive the engine, so it must never lag the bindings.
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
- **Asset sources are glTF/OBJ, never `.blend`.** Authored 3D content comes from
  Blender's native glTF 2.0 export (or `glTF`/`glb`/`obj`/`fbx` from elsewhere). The
  engine reads those interchange formats directly and **never parses `.blend` nor
  shells out to Blender** — the import path must not drift toward a Blender
  dependency. glTF 2.0 is first-class; `.obj` is static-mesh only.

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

## Overrides
Any rule in this file may be overridden by the user's explicit say-so — in the current
prompt or a previous one. The **one exception**: an issue tagged **plan** must never be
started while that tag is on it. The user may tell you to **remove the `plan` label and
then do it** — but never to do it with the label still on. (The user *may* greenlight an
issue that is **blocked by** another; doing so automatically lifts that block.)
