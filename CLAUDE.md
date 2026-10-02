# CLAUDE.md

**rusty** is a 3D game engine that copies Unity's runtime model (GameObjects,
components, scripts with `Update()`), built with **agentic coding in mind** — an AI
agent can drive, observe, and play-test it headlessly.

You will use a plain language and explain things at a high level, going into the nitty
gritty of the engine, rust or wgsl only when it's needed to address something or for the
user to understand what is going on. You can still speak in a technical, detailed way
when writing Issues and PRs, because those are documentation left for Claude to understand
what is being planned/done. The user must be informed when something is being a bottleneck
to you. You don't need to mention the rules of your CLAUDE.md unless it's needed for the
user to understand how you came up to a conclusion or if it's blocking you in some way
(or if invalidates one of your options).

## North star
The bar rusty aims for is a game on par with **Trepang2** —
visceral first-person combat carried by reactive enemy AI. That is the quality target
every decision serves: the engine is "good enough" when an agent could build a shooter
of that caliber on it. Keep this goal in mind when weighing features, architecture, and
craft — it is the reason the gates below are strict.

The second yardstick is an **offline CS:GO**: bots, bomb/defuse rounds, a buy menu and
HUD, raycast gunplay with spread, recoil and wallbangs, grenades (smoke, flash, HE,
molotov), and multi-floor maps with jump spots, drops and ladders. Trepang2 stays the
quality bar; CS:GO is the tight, mechanical bar beside it. Both are **single-player and
offline: no multiplayer, no networking, no online services — ever.** Netcode and
replication never qualify as foundation work. That rules out the *game* being online,
not the engine calling third-party APIs: tooling that reaches an external service
(e.g. a provider generating audio while authoring) is fine.

## Start here
- **README.md** — what the engine is and what you can do with it.
- **docs/** — `linting.md` (the gate), `testing.md`, `api/` (the Lua API game scripts
  use: `api/index.md`, then one file per namespace), `ui.md` (the in-game UI model). The Rust API reference is generated: `cargo doc --no-deps`.
- **auxmd.md** *(gitignored)* — the operator's short-term scratchpad; read it if a
  session points you there.

**Three skills carry the working protocols** — `issue-write`, `issue-batch` and
`ci-merge` — so this file can hold the reasoning and they can hold the steps. Each
one names its own scope and when to reach for it, so this file does not restate
them; invoke them rather than reconstructing a procedure from memory, and name
them when briefing a subagent.

Where a rule below is stated in one line and a skill has ten, the line is the rule
and the skill is how to keep it. Where they disagree, this file wins and the skill
is wrong.

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
- **Take the initiative.** Claude can always take the initiative of adding or changing
  things so long as they improve the game engine and its resulting games for the better,
  or improve the development of the game engine itself — Claude does not wait to be told
  to take initiative of improving the codebase. The user follows Claude's recommendations,
  and implementing then validating later beats waiting: a clear win without a downside is
  **implemented**, not proposed. The one exception is a change to the **engine's design
  itself** (the five kinds, the one API surface, the determinism rule, the conventions in
  this file) — there Claude proposes and the user decides, unless one option is a plain
  win-win, which Claude takes. Initiative still runs through the normal flow (an issue,
  or an issue-less PR where that is allowed; a branch; a PR; the gates), and a
  **`planning`** issue is still never started.
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
  The description says **what changed, why we want it in the project, and the effect** —
  the *why* is the project-level justification (the same value the label classifies, i.e.
  why this belongs in rusty at all), **not** how you got there (include process only when
  it's needed to understand the diff) — and carries the PR↔issue link when there is one.
  That description, the commit history, and that link are how we see what a branch adds in
  value and whether it still earns its place.
  **A ready PR claims it passes; a draft makes no such claim.** CI's gating jobs skip
  drafts, so readying a PR is the moment anything gets checked — run the gates first. A
  red ready PR **stays ready** and is fixed forward; draft is for unfinished, blocked or
  handed-back work, never for hiding a red run.
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
  needn't be serialized and can merge freely. **Except `docs/api/`:** the
  API-doc drift tests parse it, and a Markdown-only PR skips exactly those tests — so a
  PR touching it is serialized like code.
- **Coding parallelises; merging does not.** Every branch behind another in the merge
  queue pays a rebase per merge ahead of it, so the shape is **two branches in flight** —
  one merging, one being written. The binding limit is **file collision**, not branch
  count: two branches appending to the same enum or registry cost more than four in
  separate modules. **One worktree per branch** under `.claude/worktrees/`, off the latest
  `main`, removed the moment it merges, and **never a shared `CARGO_TARGET_DIR`** (worktrees
  overwrite each other's artifacts and a green gate stops meaning this branch compiled).
  Split issues by responsibility, never by parallelism. The **`issue-batch` skill** has
  the arithmetic and the procedure.
- **Worktrees are cheap, simultaneous builds are not.** rusty is developed by one person,
  locally or in a cloud session — not by many contributors on many cold machines, so
  don't propose tooling built for that. Each worktree compiles into its own `target/`,
  and one cold build fills every core and several GB of memory. Parallelise the work,
  stagger the compiles, and check the machine you are on (`df -h`, free memory) before starting a
  build — never a figure written down somewhere. A batch may also hand GPU-free,
  module-separate branches to **cloud sessions** as extra coders: that lifts the local
  build limit, never the one-at-a-time merge. The `issue-batch` skill says when, and
  how to prove a session really is remote.
- **The lightest batch keeps only the merge queue local.** With the coding in cloud
  sessions, `make queue ARGS=--no-check` skips the local compile check of each rebased
  head and lets CI judge it, so merging costs this machine `git` and GitHub calls, not a
  build. Prefer it whenever the machine has other work. The price is that a rebase broken
  by a merge ahead (rare: a few in 126 PRs) is caught by CI a round later, not before
  the push.
- **Infrastructure- then architecture-first (NOT "make it up as we go").** We do **not**
  improvise or pile on features ad hoc. Whenever we find a problem — something that
  already bites or will bite more than once, a pattern worth adopting, or a gold-standard
  practice we should have had — we **document it and implement it right away**, before
  continuing. We do **not** have the right to add more features/shenanigans until the
  infrastructure/architecture itself is improved first. Infrastructure (the tools we build
  with) outranks architecture (how things are organized) — make building cheap and safe
  before reshaping what gets built, so every later change runs on the faster, safer loop
  instead of burning compile time overnight — and both outrank features. See *Issues,
  labels & priority* for the full order. Fix the foundation, then build on it. Each such
  fix gets its own issue when it carries its own responsibility.
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
- The user writes plenty of Github Issues and then does them in batches, asking you to do
  them as you see fit (regarding how many batches and which issues take priority). In those
  cases, it's best to, when in doubt of something, write a comment in the Issue, and then resume
  the work if possible, than to ask a question in the chat and spend the whole night waiting for
  an answer. However, conversations which are planning of such GH Issues don't have to wait, you
  can ask your questions right away.

## Issues, labels & priority
- **Issues come before PRs.** The unit of work is a well-specified issue — a clear
  statement of **what** to set up or change, **why we want it in the project** (the
  project-level justification the label then classifies), and the **roadmap, not the
  implementation intrinsics**. A good issue description is **documentation of what was
  thought of**: it captures the reasoning so a future Claude can read it cold — no prior
  context, no deep digging — and say *"I understand the assignment and have a good idea
  what to do."* That's what lets Claude Code pick an issue up and run it unattended, even
  overnight. So defining issues well outranks opening PRs: get the roadmap right and the
  doing is the easy part.
- **Assign yourself when you start.** The moment Claude begins actively working an issue,
  **assign the user to that issue** so it's visibly taken — everyone can see it's being
  worked on right now, not just sitting in the backlog. An unassigned issue is fair game;
  an assigned one is in progress.
- **Never file an issue and start it in the same breath** — unless the work is a *direct
  consequence* of another, already-decided issue, or a clear win under *Take the
  initiative*. Filing-then-immediately-implementing
  defeats planning: an idea still being shaped has to settle before anyone codes it.
- **Issue-less PRs are allowed only** for documentation updates or bug fixes; everything
  else starts as an issue.
- **File what you notice.** Claude may open an issue unprompted for anything that will
  recur, or that a tool would solve more than once — when the benefit outweighs the cost
  of building it. The strongest issues come out of doing the work. **A bug is always
  filable**: that test is about whether something is worth *building*, never whether a
  defect is worth *recording*. If the bug questions a decision or exposes a foundational
  crack, tell the user; otherwise keep it brief and carry on. Found outside the branch in
  hand? **File it rather than fix it** — a branch that grows to cover everything it
  noticed is a branch nobody can review.
- **Priority by label.** When choosing what to do next, the order is
  **infrastructure → architecture → bug → foundation → feature.** It encodes how the whole
  project is built, in three stages:
  1. **Guardrail the development first.** If the *way we build the engine* isn't solid — a
     missing tool or guardrail for development (**infrastructure**), or a missing
     structural shape or convention (**architecture**) — we **halt everything and fix that
     first**. We don't earn the right to build more until the means of building are sound.
     Infrastructure leads because it makes every later branch cheaper: a faster build,
     test or merge loop pays off on all the work queued behind it. A **bug** (broken stuff)
     sits here too: a broken engine is no foundation to build on. A bug in the
     *development tooling itself* — CI, the gates, the hooks — ranks as
     **infrastructure**, not bug: while it stands, every branch pays for it.
  2. **Then make the engine as complete as it can be** — **foundation** work, which
     improves the *engine itself*.
  3. **Then features** — **feature** work that makes the eventual *game's* development
     faster, easier, or more complete. The game is built **elsewhere** (another repo, or by
     whoever uses the engine), so engine-completeness always outranks game-facing
     convenience here.
  **documentation** can be done at any time and never waits its turn.

### Labels
- **architecture** — *How we define stuff.* Intrinsic changes to the engine: a new module,
  a significant structural improvement, a convention we adopt, or a guardrail baked into
  the engine's **design** (e.g. the determinism rule, the five-kinds model, the four-axis
  component gate) — the engine's shape, not a single feature. Contrast **infrastructure**,
  which guardrails the *development process* rather than the engine's design.
- **bug** — *Something isn't working.* Broken behaviour to fix.
- **documentation** — *Edit/add documentation.* Keep it clear and clean how the engine is
  used and what its features are — architecture's *definition*, not its implementation.
- **feature** — *Feature or improvement.* A new engine capability that makes the eventual
  game's development faster, easier, and/or more complete.
- **foundation** — *Groundworks for game development.* A new capability that makes the
  **engine itself** more complete. Despite the name, this is stage-2 work that sits **on
  top of** the guardrail layer (architecture/infrastructure), not beneath it — engine
  completeness, not the dev groundwork.
- **human** — *AI can't do this end-to-end.* The issue needs a human in the loop to finish.
- **infrastructure** — *Improves engine or agentic development.* Tools or guardrails for
  the engine's **development process** (e.g. the lint/size gate, CI, the headless harness),
  making that development faster, solid, and correctly guardrailed — distinct from
  **architecture**, which is a guardrail in the engine's own design.
- **planning** — *Approach still being discussed. Do not start.* Still under discussion
  (see below) — must not be started.

Issues tagged **planning** are still being discussed with the user. They must **NOT** be
started by any means. If a planning issue would implement something that affects another
issue — changing how it gets implemented, or even how it's thought of — that other issue
must be marked **blocked by** the planning issue.

**The `issue-write` skill** has the rest: what a good issue body contains, the
evidence that makes one worth reading cold, how relationships are recorded, and
when Claude may file one unprompted. **`issue-batch`** turns the priority order
above into a running batch, and **`ci-merge`** takes each finished branch from
green to merged.

## Architecture — the conceptual model
A high-level map of how the engine is shaped. It deliberately doesn't enumerate every
concrete type — that inventory lives in the rustdoc reference (`cargo doc --no-deps`)
and the script surface in `docs/api/`. Everything in the engine is one of
five kinds of moving part (Unity analogs in parentheses):

A **GameObject** is one entity (`Entity` in `components/entity.rs`): a mandatory
`Transform` plus any mix of the optional first-class components below, and a place in
the parent/child hierarchy. It can be **empty** — a Transform and nothing else, used as
a grouping pivot or a spawn marker. Or it can be configured: instantiating a glTF
yields a GameObject carrying a `Transform`, a `Mesh`, a `Collider` when the asset
provides one, and the engine's default `Material`. Same object, more components — that
is the only difference between an empty marker and a fully-dressed enemy. A *skinned*
glTF also brings its skeleton: one child GameObject per bone (#453), posed by the
`Animator` and read back for skinning, so attaching a gun or a hitbox to a hand is
plain parenting. Bones are rebuilt from the model on load; the scene saves only
per-bone overrides and what hangs from a bone.

1. **Resources** — engine singletons, one per World (Unity's engine statics: `Time`,
   `Input`, the nav graph, the console, the active camera, play-state, the renderer).
   Global state the systems read and write.
2. **Components** — per-entity data, the Unity-style "classes" (`Transform`, `Mesh`,
   `Camera`, `Light`, `Collider`, `Rigidbody`, `NavMeshAgent`, `Animator`,
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
   `Physics`, `Scene`, `Animator`, `Nav`, `Camera`, `Material`, `Application`, and
   the dev-only `Debug`) shared by gameplay scripts, the console REPL, and bot-players.
   One surface, three callers — they never drift apart.

One thing sits outside those five: **zimmer**, the synthesiser behind `Sound.*`, is
an external crate from [scorsese](https://github.com/MatthewLacerda2/scorsese), a git
dependency pinned to a commit (#413) — not a vendored module. `src/api/sound/` is
only its adapter (Lua → zimmer document, file writes, patch-path resolution).

## Conventions that matter
- **Unity is the reference; rusty is a deliberate subset.** Use Unity Engine as the
  yardstick for what rusty *must* be capable of. Unity is exhaustive, so we implement
  only the subset a game actually needs — never feature-for-feature parity. The twist is
  that our API exists to be driven by **Claude Code**, not hand-written: developers won't
  write or even read the engine's code. As long as the docs stay current, Claude can tell
  the developer how anything is done — so keeping documentation truthful is load-bearing,
  not a nicety.
- **When in doubt, Unity's way.** When the work hits a call its issue never foresaw (a
  default, an edge-case behaviour, how an API takes its arguments), do what Unity does
  and say so in the PR; most of it is well figured out. This fills gaps the planning
  missed. It does not override a decision the issue made, and it does not bring in what
  rusty deliberately leaves out.
- **AI-driven, editor↔API parity.** rusty is built for an agent-driven workflow in the
  *Claude Code + Blender-MCP* style: the agent can do anything a user can do in the
  editor — create entities, place and configure components, instantiate assets, save
  scenes — **except pure UI chrome** (collapsing a card, resizing a panel). Every
  user-facing editor capability has an API equivalent.
- **Keep the API doc in lockstep.** When you add or change an API function, update
  its namespace's page in `docs/api/` (`docs/api/<Namespace>.md`) in the *same* change. That doc is the API reference the agent
  reads to drive the engine, so it must never lag the bindings. This is no longer just
  convention: a **CI drift gate** (`tests/api_doc_drift.rs`, #280) fails the build when
  the doc and the live Lua surface disagree about *what exists* (existence parity both
  directions; signatures stay out of scope).
- **ECS via `hecs`.** `Transform` is the one mandatory component; all others optional.
- **No event bus, no plugin trait.** Modules self-register via `register(&mut app)`;
  cross-system signals are direct typed returns.
- **Dev-only build profile.** The console/REPL, harness, bot-players, and `Debug.*`
  live behind the `dev` Cargo feature and are stripped from ship builds.
- **The editor is a feature too.** `src/editor`, the shell's editor frontend and egui
  live behind the default-on `editor` Cargo feature (#431). A shipped game is the
  `player` binary built with `--no-default-features`, so it carries neither the editor
  nor `dev`. Engine code outside those two stays egui-free; clippy checks that build
  (`--no-default-features`) so it can't rot.
- **One runtime shell, two frontends.** `src/shell` owns the window, the frame loop,
  input, cursor, settings and what `Application.Quit()` means; the editor and the
  standalone player are its two frontends. Window/input behaviour both need goes in
  the shell, once — never copied into a frontend.
- **Determinism.** The sim is a pure function of (seed, inputs, fixed dt). Wall-clock
  reads and unseeded RNG are banned from the sim modules — the sim proper (`app`,
  `scripting`, `physics`, `navigation`, `ui`), the `api` scripts call, and the data
  and tools it runs on (`scene`, `components`, `ecs`, `core`, `time`, `asset`,
  `procgen`, `shadergen`, `audio`); the platform layer (`shell`, `render`, `editor`,
  `dev`, `preview`) is exempt.
  Scripts run inside the sim too: the gameplay Lua VM has no `os`/`io`, and
  `math.random` routes to the seeded `Random` resource (`core::random`, #443).
- **Use `glam`** for all math; keep egui / wgpu / mlua decoupled.
- **Single crate.**
- **Ships for macOS and Linux; Windows comes later.** Those two are the platforms rusty
  is used on and shipped to, so a change that works on only one of them is not done.
  Windows is a planned target, not a current one.
- **Group by subfolder, not by filename prefix.** A shared name prefix on sibling
  files (`draw_*`, `setup_*`, `inspector_*`, `prefab_*`) is a subfolder waiting to
  happen: make it one and drop the prefix (`draw_lighting.rs` → `draw/lighting.rs`).
  Aim for ≲10 source files per module directory; past that, cluster by responsibility
  into subfolders. This is the missing half of the 300-line size cap — small files plus
  flat directories is sprawl by construction, and Rust modules nest for free. **One
  exception:** flat registries where one file *is* one public unit (e.g. each `api/`
  file is one Lua namespace) stay flat — there the flat list *is* the documentation.
- **Asset sources are glTF/OBJ, never `.blend`.** Authored 3D content comes from
  Blender's native glTF 2.0 export (or `glTF`/`glb`/`obj`/`fbx` from elsewhere). The
  engine reads those interchange formats directly and **never parses `.blend` nor
  shells out to Blender** — the import path must not drift toward a Blender
  dependency. glTF 2.0 is first-class; `.obj` is static-mesh only.

## Commit gate (programmatic — no AI needed)
`make gates` runs everything CI blocks on, fastest-failing first; run it before
readying a PR (`make help` lists the verbs, `make check` is the fast edit loop).
Failures from `tools/lint` are written to `.lint/report.txt`. See **docs/linting.md**.
- **The commit hook** (`.githooks/pre-commit`, activated once per clone by `make
  setup`) runs formatting and the size gate only, so it stays under a second.
- Size gate: files ≤ 300 lines, test/fixture files ≤ 150. Style is rustfmt;
  **clippy is a hard gate** (`-D warnings`: default, `dev`, and the no-editor player
  build); the lint policy
  lives in `Cargo.toml`'s `[lints]`, not in flags.
- **Determinism guard** (`make determinism`) — fails on wall-clock / unseeded RNG
  in the sim modules listed under *Determinism* above; it protects the harness's
  reproducibility.
- **Direction guard** (`make direction`) — fails when a sim module (the same list)
  references `crate::render`, `crate::editor`, `wgpu`
  or `egui`; the arrow is render/editor → sim.
- `make gates` refuses to run when cargo's target dir is outside the worktree — a
  shared one is a false green.
- `tools/lint/baseline.txt` grandfathers the files that currently exceed the size
  cap. It's a **burn-down list** — remove entries as you split them, never add to it.

## Overrides
Any rule in this file may be overridden by the user's explicit say-so — in the current
prompt or a previous one. The **one exception**: an issue tagged **planning** must never be
started while that tag is on it. The user may tell you to **remove the `planning` label
and then do it** — but never to do it with the label still on. (The user *may* greenlight an
issue that is **blocked by** another; doing so automatically lifts that block.)
