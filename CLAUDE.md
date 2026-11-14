# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

You will use a plain language and explain things at a high level, going into the nitty
gritty of the engine, rust or wgsl only when it's needed to address something or for the
user to understand what is going on. You can still speak in a technical, detailed way
when writing Issues and PRs, because those are documentation left for Claude to
understand what is being planned/done. The user must be informed when something is being
a bottleneck to you. You don't need to mention the rules of your CLAUDE.md unless it's
needed for the user to understand how you came up to a conclusion or if it's blocking
you in some way (or if invalidates one of your options).

## North star
The bar is a game on par with **F.E.A.R.** (2005) or **Trepang2** — visceral
first-person combat carried by reactive enemy AI. The engine is "good enough" when an
agent could build a shooter of that caliber on it; that is why the gates below are
strict. The engine behind it matches **Unity** in quality and capability, and follows
Unreal where Unreal goes deeper at a bounded cost (*Unity is the shape; Unreal is the
depth*, below). What Unity lacks, rusty has from the start: **the agent can create and
edit the whole game, and the developer only says what they want done**. It gets there
one piece at a time, each capability built when the work needs it, in the priority order
below: we are not trying to one-shot a game engine.

The second yardstick is an **offline CS:GO**: bots, bomb/defuse rounds, a buy menu and
HUD, raycast gunplay with spread, recoil and wallbangs, grenades (smoke, flash, HE,
molotov), and multi-floor maps with jump spots, drops and ladders. Both are
**single-player and offline: no multiplayer, no networking, no online services — ever**,
and netcode never qualifies as foundation work. That rules out the *game* being online,
not authoring tooling that calls an external service.

## Start here
- **README.md** — what the engine is and what you can do with it.
- **docs/** — `linting.md` (the gate), `testing.md`, `api/` (the Lua API, one file per
  namespace), `ui.md` (in-game UI), `providers.md` (keys, budget), `mcp.md` (the
  `session-mcp` bridge) and `game-agent.md` (the guide it serves a game project).
- **auxmd.md** *(gitignored)* — the operator's scratchpad; read it if pointed there.
- **Nested CLAUDE.md files** carry one area's rules and load only when you read files
  there: `.github/scripts/CLAUDE.md` (merge and batch tooling), `tools/lint/CLAUDE.md`,
  `src/render/CLAUDE.md` (with `engine/shaders/`) and `crates/procgen/src/CLAUDE.md`.

**Three skills carry the working protocols**: `issue-batch` (issues from board to
merged: coders, worktrees, briefs, filing a bug), `ci-merge` (the gates, CI, the merge
queue and trains, the mutation and coverage signals) and `independent-review` (an
unbiased read of a PR before it queues). This file holds the rules, the skills the
steps: invoke them, name them in a subagent's brief; where they disagree, this file
wins.

## How we work
- **The project serves its development first.** Claude is never tied to the current way
  of doing things: it looks for a more scalable way to improve the project, then
  improves the project itself (tools, structure, infrastructure), and only then uses it;
  the more is automated the better. The carve-out is a session building or playtesting a
  game *with* rusty: findings are noted and dealt with afterwards.
  - **Infrastructure- then architecture-first (NOT "make it up as we go").** No ad-hoc
    features. A problem that bites or will bite again, a pattern worth adopting or a
    gold-standard practice we lack is **documented and implemented right away**, before
    continuing. Infrastructure (the tools we build with) outranks architecture (how
    things are organized); both outrank features. Each such fix gets its own issue when
    it carries its own responsibility.
  - **Agent velocity is first-class.** Agents drive the engine and each other, often
    unattended. Code is **readable by design and lean**, which costs the next agent
    fewer tokens and wrong turns; CI stays fast. This is part of **Craft**. **Efficiency
    over familiarity:** do what works best for the project, not what fits the user's
    mental model of how it should be done.
  - **A convention a machine can check is a gate, not a sentence.** Finding what can be
    enforced is Claude's job, not the operator's. A written rule that a check could hold
    gets that check on Claude's own say-so, and a new convention ships with its check.
    Every check has a case it must flag and a case it must pass, runs in `make gates`
    and CI, and takes exceptions **inline, with a written reason**, never as an unread
    list in a config file. Where a check would misfire, the prose says so and why. A
    check speaks when it fails, so its failure message says what is wrong and what to
    do; prose that only restates a check is dropped. How the lint tool keeps this:
    `tools/lint/CLAUDE.md`.
- **The gates (push back before you build).** An idea becomes an issue only when all
  three hold; if any fails, **push back instead of complying**:
  1. **Understanding.** Claude can restate the intent; if unsure, confirm first.
  2. **Value.** It adds real value: no busywork, no features for their own sake.
  3. **Craft.** It follows Rust/Lua good practice and the gold standards of game-engine
     design; if not, say so and propose the right shape.
- **Take the initiative.** A clear win without a downside to the engine, its games or
  its development is **implemented**, not proposed or waited on. The exception is
  **changing a decision** (the engine's design: the five kinds, the one API surface, the
  determinism rule; or a convention in this file): Claude proposes and the user decides,
  unless one option is a plain win-win. **Making a description true or complete is never
  the exception:** a stale doc, README, comment, CLI help, MCP or API description, skill
  or line of this file is fixed and merged on Claude's own say-so, through the normal
  flow; a **`planning`** issue is never started.
- **Agent docs state the rule, not its history.** Every `CLAUDE.md` and skill says what
  to do and, where it helps, why — never the incident behind it, a date, an attribution
  or an issue/PR number. The story lives in the issue and PR that made the rule, where
  `git log -S` and the issue tracker find it. A pointer names the symbol, path or `make`
  verb, not the issue. Every line of this file is a convention whether or not it says
  who asked for it.
- **Flow:** discuss (if needed) → an issue → its dependencies → a branch → its PR →
  merge. **Issue-less PRs are allowed only** for documentation or bug fixes; every
  description still clears the three gates.
- **Pull requests — open early, draft until ready.** A branch's first commit gets a PR,
  unasked: **draft** while in progress or blocked (**say why**), **ready for review**
  when nothing more is needed. **A ready PR claims it passes; a draft makes no such
  claim**, so run the gates before readying; a red ready PR stays ready and is fixed
  forward. Branches are `{issue_number}-short-slug`, or a slug of the subject when
  issue-less. The description, and how a PR flips: `ci-merge`, *Draft or ready*.
- **Merging — serialized, one tested tree at a time:** one PR green on the latest
  `main`, or a **merge train** of up to four tested together; a branch is updated by
  **merging `main` in, never rebasing**; ≈3 attempts at one failure, then draft and hand
  it to the user. Markdown-only PRs skip CI's build, **except the Markdown code reads**
  (`docs/api/`, `docs/game-agent.md`, `crates/core/src/project/game_claude.md`). `ci-merge`.
- **Coding parallelises; merging does not.** **Cloud coders ≤5, local coders ≤2,
  read-only agents without limit**; **one worktree per branch** under
  `.claude/worktrees/`, **never a shared `CARGO_TARGET_DIR`**; split issues by
  responsibility, never by parallelism. **Worktrees are cheap, simultaneous builds are
  not:** **a branch is written in a cloud session by default** (`issue-batch`, *Where a
  branch runs*, has the reasons).
- **Dependencies (not batches).** Record how issues relate with GitHub's native
  **relationships** (`Blocked by` / `Blocks`, **sub-issues** for literal groundwork)
  when one lays groundwork for, eases, or would conflict with another. The dependency
  graph *is* the plan: an issue with no open blockers is fair game.
- **Memory is the developer's; what the project follows lives in the project.** A
  project rule found in local memory moves into this file, a skill or a doc, which cloud
  coders read. **What the repository keeps is written in English**, whatever the chat's.
- **Gates vs. signals — block on correctness, inform on quality.** A check proving
  **correctness** (build, test, clippy, the determinism guard, the size gate) is a
  **hard gate**, green-to-merge. One *auditing quality* (mutation, coverage) is an
  **informational signal**: it runs **weekly** on `main` or **on request** over what
  someone names, never on a pull request, **never blocks a merge**, and is **surfaced
  where the agent acts on it** (terminal, job summary); no hard gate where a signal does.

## Issues, labels & priority
- **Issues come before PRs.** The unit of work is an issue a future Claude can read cold
  and run unattended, so defining issues well outranks opening PRs.
- **Point at code by symbol and path, never a bare line number.**
  `physics::shape::build::collider_inputs` survives edits; `src/main.rs:210` does not.
- **Record relationships on GitHub**, not only in prose (`make blockers ARGS=--fix`).
  **Assign yourself when you start:** assign the user to the issue, so it's taken.
- **The closing-keyword trap.** GitHub closes an issue on `closes #N` anywhere in a PR
  description, even quoted or negated; a PR that leaves it open writes `Refs #N`.
- **A stage label is the only absolute stop.** `planning` and `human` mean *not yet*.
  Anything else is startable the moment it exists, except a bug whose `Kind:` names an
  open root issue, which that root's branch does.
- **Issues are for what matters.** A finding is filed when it **already caused a real
  problem** or **costs more to leave than to fix**; otherwise a review reports it and
  Claude fixes it in the branch or drops it. A crack in the **structure, architecture or
  tools** (a slow build, a CI gap, a missing gate) is always worth an issue. Small
  findings in the way are folded in; one outside the branch is filed, not fixed. One
  that questions a decision or a foundation is told to the user. **Before filing a
  bug**, search for its kind and class it a **case** or a **symptom** (which gets the
  **fix that scales**); it carries one `Kind:` line (`issue-batch`, *Filing a bug*).
- **Once is a case; twice is an issue.** The second time, attack the root (the cause, a
  tool, a check); **a kind of bug counts, not only the same bug**, if it happened.
- **The approval test.** A finding that changes the engine's design (the five kinds, the
  one API surface, the determinism rule) or a convention here is filed with `planning`,
  never folded in. A stored format is not on the list.
- **Bug and doc fixes merge without waiting**, even when a scene plays differently
  (**Guard:** no design change slips in under `bug`). **A default the game developer can
  change is not a question:** pick a standard one, name it in the PR, go on.
- **Decisions are written down, not waited on.** An issue states each decision with a
  recommendation; the answers go into it. Claude applies `planning` itself only to an
  *unattended* finding failing the approval test, raised at the next check-in.
- **Priority by label:** **infrastructure → architecture → bug → foundation → feature**.
  (1) **Guardrail the development first**: a missing tool or guardrail, a missing
  structural shape, or a bug halts everything (a bug in CI, the gates or the hooks ranks
  as infrastructure). (2) **Then make the engine as complete as it can be**
  (foundation). (3) **Then features** for the game, which is built elsewhere.
  **documentation** never waits its turn.

### Labels
| Label | Description | Means |
|---|---|---|
| **agent** | *Written autonomously by an agent (not asked for by a person).* | on every issue Claude files unprompted; never a stop |
| **architecture** | *How we define stuff.* | the engine's shape: a module, a convention, a design guardrail (determinism, the five kinds, the four-axis gate) |
| **bug** | *Something isn't working.* | broken behaviour |
| **documentation** | *Edit/add documentation.* | how the engine is used, not how it is built |
| **feature** | *Feature or improvement.* | makes the game's development easier |
| **foundation** | *Groundworks for game development.* | makes the **engine itself** more complete (stage 2, above the guardrails) |
| **human** | *AI can't do this end-to-end.* | needs a human in the loop |
| **infrastructure** | *Improves engine or agentic development.* | the **development process**: the lint gate, CI, the harness |
| **planning** | *Approach still being discussed. Do not start.* | an issue it would change is **blocked by** it |

## Architecture — the conceptual model
A high-level map (types: `cargo doc --no-deps`; scripts: `docs/api/`). Everything is one
of five kinds of moving part (Unity analogs in parentheses).

A **GameObject** is one entity (`Entity` in `components/entity/mod.rs`): a mandatory
`Transform` (never removed), any optional first-class components, and a place in the
hierarchy. It can be **empty** (a pivot, a spawn marker) or configured: a glTF instance
carries a `Transform`, a `Mesh`, a `Collider` when the asset has one, and the default
`Material`. A *skinned* glTF adds one child GameObject per bone, posed by the
`Animator`, so a gun on a hand is plain parenting; bones are rebuilt on load and the
scene saves only per-bone overrides and what hangs from a bone.

1. **Resources** — engine singletons, one per World (Unity's engine statics: `Time`,
   `Input`, the nav graph, the console, the active camera, play-state, the renderer).
2. **Components** — per-entity data (`Transform`, `Mesh`, `Camera`, `Light`, `Collider`,
   `Rigidbody`, `NavMeshAgent`, `Animator`, …): the **first-class components**, each
   meeting the completeness gate's four axes (`docs/linting.md`). Custom behaviour goes
   in *script components* (Lua MonoBehaviours), which are **not** first-class, even the
   engine's own, so the gate never applies to them; it finds first-class components
   solely from the component registry's rows (`components::registry`), which generate
   `Entity`'s fields. New first-class components are added in Rust, not script.
3. **Systems** — a plain `fn(&mut World, &mut Resources)` in ordered stages (`Startup`,
   then `FixedUpdate → Update → LateUpdate → Render`), in the order modules `register`
   them. `FixedUpdate` holds the whole deterministic sim on the 1/60 s tick (the harness
   one tick per step, the window through `shell::frame::ticks`' accumulator, drawn
   blended by `render::interpolate`); scripts' `Update` and `LateUpdate` run inside it
   as phases, in `app::play::register`'s order. `Update` is empty; `LateUpdate` holds
   the audio mix and the UI's sway and layout.
4. **Scene & serialization** — one active scene as a serde `SceneData` document (no GPU
   buffers). Save/load replaces the World; clone-on-Play / restore-on-Stop keeps
   edit-mode authoritative, as in Unity.
5. **The API surface** — one stable set of namespaces (`Transform`, `Input`, `Time`,
   `Physics`, `Scene`, `Animator`, `Navigation`, `Camera`, `Material`, `Application`,
   and the dev-only `Debug` and `Editor`) shared by gameplay scripts, the console REPL,
   and bot-players. One surface, three callers — they never drift apart.

Outside the five: **zimmer**, `Sound.*`'s synthesiser, a git dependency on
[scorsese](https://github.com/MatthewLacerda2/scorsese) by `rev`; `crates/script/src/api/sound/`
adapts it.

## Conventions that matter
- **Backward compatibility is not a concern; everything is version 1.** rusty's one user
  is its operator: an upgrade may break a game, and the game is what moves. A stored
  format (scene, prefab, save, sidecar) changes freely, no migration, shim or version
  bump owed, and the PR updates the engine's fixtures and default scene.
  `Converter::version` only re-makes the import cache, and bumps when a converter's
  output changes (`asset::convert::fingerprint`).
- **Unity is the floor; rusty reaches it as needed.** Everything Unity gives a developer
  is the destination, never a menu: a capability is deferred, never turned down as
  "something no game needs", nor filed before the work needs it. Developers never read
  the engine's code; Claude tells them from the docs, so truthful docs are load-bearing.
- **Unity is the shape; Unreal is the depth.** Where a north-star system goes deeper in
  Unreal, Unreal's version is the bar if bounded (builds on what exists, a few issues,
  no new kind of moving part); where Unity has nothing (EQS), follow Unreal. Nanite,
  Lumen and Blueprints stay out.
- **When in doubt on how to approach, just follow Unity Engine**, in planning and on a
  call the issue never foresaw (a default, an edge case, an API's arguments), and say
  so. It never overrides a decision made or the north star.
- **The agent builds the game; the developer directs** (the *Claude Code + Blender-MCP*
  style). The agent does everything a developer does, in the editor and around it
  (assets, play-tests, builds), **except pure UI chrome**; every editor capability has
  an API equivalent, and a dev task it can't do yet is a gap to file.
- **The editor's clickable UI is a subset of Unity's:** the hierarchy, the inspector and
  Play. Chrome serving only hand authoring (a file browser, a node graph) is not built,
  and existing chrome of that kind is removed.
- **Keep the API doc in lockstep.** An API change updates `docs/api/<Namespace>.md` in
  the *same* change; `tests/api_doc_drift.rs` holds what exists.
- **ECS via `hecs`.** **No event bus, no plugin trait:** modules self-register via
  `register(&mut app)`, and cross-system signals are direct typed returns.
- **Dev-only build profile.** The console/REPL, harness, bot-players and `Debug.*` sit
  behind the `dev` feature, stripped from ship builds.
- **The editor is a feature too.** `src/editor`, its shell frontend and egui sit behind
  the default-on `editor` feature; a shipped game is the `player` binary built
  `--no-default-features`, and engine code outside those stays egui-free.
- **One runtime shell, two frontends.** `src/shell` owns the window, frame loop, input,
  cursor, settings and `Application.Quit()`; what both frontends need goes there, once.
- **One declared layering.** `tools/lint/src/layers/table.rs` says which module may
  import which and name which crates, and marks the **sim** rows (the rest is the
  platform layer); `make layers` checks it. Change it in the PR adding the import.
- **Determinism.** The sim is a pure function of (seed, inputs, fixed dt): no wall-clock
  reads, unseeded RNG or `std`'s `HashMap`/`HashSet` in sim modules (use
  `core::collections`). The gameplay Lua VM has no `os`/`io`; `math.random` uses the
  seeded `Random` resource (`core::random`); a `require` outside the project warns.
- **Provider calls are authoring, never gameplay.** Paid, networked calls live behind
  `src/dev/providers/`: `dev` only, refused during Play, held to the project's `budget`,
  never made by a test (traits and mocks). `docs/providers.md`.
- **Use `glam`** for all math (`deny.toml` bans `cgmath`, and `nalgebra` outside
  physics); keep egui / wgpu / mlua decoupled. **A workspace being split one crate
  per module** (`crates/<module>/`): `crates/CLAUDE.md` is the recipe for the next.
- **Ships for macOS and Linux**, no other platform (`platform-cfg`). Not done on one.
- **Group by subfolder, not by filename prefix** (`draw_lighting.rs` →
  `draw/lighting.rs`): aim for ≲10 source files per module directory (`dir-size` fails
  past 12; components cluster by Unity's *Add Component* categories). **One exception:**
  a flat registry of one public unit per file (`api/`), marked
  `// lint: allow(dir-size) <why>`.
- **Test content is primitive; levels are Dust2-sized.** Fixtures are primitive shapes;
  a whole-map feature (navigation, lighting, a bake) is proven at Dust2 scale.
- **No machine state in repo docs.** No specs or free-space figure to plan by: write
  "check the machine" (`df -h`, free memory).
- **One format inside, glTF; everything else is converted at import.** Loaders read glTF
  2.0 and `.obj` (static meshes). `.fbx` converts in-engine (`ufbx`), `.blend` through
  headless **Blender** (`RUSTY_BLENDER` for a non-standard install), into a cached glTF
  beside the kept source. **Blender 5 or later is a requirement**, with the Blender MCP;
  without it the import asks for it, no fallback. Blender is **authoring only**: a
  shipped game and the sim only see glTF. `asset::MODEL_EXTENSIONS` lists what the
  importer reads; `std::process::Command` takes an inline reason (`subprocess`).

## Commit gate (programmatic — no AI needed)
`make gates` runs everything CI blocks on, fastest-failing first (`make check` is the
edit loop; failures land in `.lint/report.txt`). **Nothing is grandfathered.** The gates
and hooks: `docs/linting.md`, *The commit gate in brief*.

## Overrides
Any rule in this file may be overridden by the user's explicit say-so — in the current
prompt or a previous one. The **one exception**: an issue tagged **planning** must never
be started while that tag is on it. The user may tell you to **remove the `planning`
label and then do it** — but never to do it with the label still on. (The user *may*
greenlight an issue that is **blocked by** another; doing so automatically lifts that
block.)
