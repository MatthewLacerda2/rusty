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
  use: `api/index.md`, then one file per namespace), `ui.md` (the in-game UI model), `providers.md` (bringing a provider key, and the budget). The Rust API reference is generated: `cargo doc --no-deps`.
- **auxmd.md** *(gitignored)* — the operator's short-term scratchpad; read it if a
  session points you there.
- **Nested CLAUDE.md files** carry one area's rules and load only when you read files
  there: `.github/scripts/CLAUDE.md` (the merge and batch tooling), `tools/lint/CLAUDE.md`
  (the lint tool) and `src/render/CLAUDE.md` (the renderer and `engine/shaders/`).
  Project-wide rules stay in this file.

**Three skills carry the working protocols** — `issue-batch`, `ci-merge` and
`independent-review` (an unbiased read of a PR before it queues) — so
this file can hold the reasoning and they can hold the steps. Each
one names its own scope and when to reach for it, so this file does not restate
them; invoke them rather than reconstructing a procedure from memory, and name
them when briefing a subagent.

Where a rule below is stated in one line and a skill has ten, the line is the rule
and the skill is how to keep it. Where they disagree, this file wins and the skill
is wrong.

## How we work
- **The project serves its development first** (the operator, 2026-10-16; scorsese
  adopted it first). Claude is never tied to the current way of doing things. First it looks for a
  more scalable way to improve the project faster; then it improves the project itself —
  its tools, structure and infrastructure; only then does it use it. Everything we do runs
  inside a framework that says how it is done, and the more of it is automated the better.
  The one carve-out is a session spent building or playtesting a game *with* rusty: there
  findings are noted and dealt with afterwards, so the game is not derailed. Three rules
  keep the principle:
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
  - **Agent velocity is first-class.** This workflow is agents driving the engine and each
    other, often unattended — throughput counts. Write code that is **readable by design and
    lean**: not for style points but because clear, well-shaped code is cheaper to reason about
    (fewer tokens, fewer wrong turns) and faster for the next agent to extend. Strip avoidable
    blockages and keep CI fast. This is part of **Craft**, not a trade-off against it.
    **Efficiency over familiarity:** do what works best for the project, not what fits
    the user's mental model of how it should be done.
  - **A convention a machine can check is a gate, not a sentence.** Finding what can be
    enforced is Claude's job, not the operator's. A written rule that a check could hold
    gets that check on Claude's own say-so, and a new convention ships with its check.
    Every check has a case it must flag and a case it must pass, runs in `make gates` and
    CI, and takes exceptions **inline, with a written reason**, never as an unread list in
    a config file. Where a check would misfire, the prose says so and why. How the lint
  tool keeps this: `tools/lint/CLAUDE.md`.
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
  **implemented**, not proposed. The one exception is **changing a decision**: the
  engine's design (the five kinds, the one API surface, the determinism rule) or a
  convention in this file — there Claude proposes and the user decides, unless one option
  is a plain win-win, which Claude takes. **Making a description true or complete is never
  the exception:** a stale or incomplete doc, README, module doc, comment, CLI help, MCP or
  API description, skill or line of this file is fixed on Claude's own say-so and merged. Initiative still runs through the normal flow (an issue,
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
  needn't be serialized and can merge freely. **Except the Markdown code reads:**
  `docs/api/` (the API-doc drift tests parse it), `docs/game-agent.md` (served by
  `session-mcp`) and `src/core/project/game_claude.md` (embedded by the build). A
  Markdown-only PR skips exactly the tests that read them, so CI's `code` filter counts
  them as code and a PR touching them is serialized like code.
- **Coding parallelises; merging does not.** How many branches are written at once
  scales with **how separate their files are**: two branches appending to the same enum
  or registry cost more than five in separate modules, because every branch behind
  another in the merge queue pays a rebase per merge ahead of it, and only a colliding
  rebase is expensive. **Three kinds of agent, three limits:** **cloud coders ≈10**,
  paced by token spend and by how many diffs one orchestrator can read, not by
  hardware (so mechanical work runs on Sonnet; `issue-batch`, *Which model*); **local coders ≤2**, because the queue merges one PR per CI run and a third
  local coder finishes nothing sooner; **read-only agents** (research, review,
  planning) without limit, since they build nothing. Merging stays **one at a time**,
  whatever the count. **One worktree per branch** under
  `.claude/worktrees/`, off the latest `main`, removed the moment it merges, and **never
  a shared `CARGO_TARGET_DIR`** (worktrees
  overwrite each other's artifacts and a green gate stops meaning this branch compiled).
  Split issues by responsibility, never by parallelism. The **`issue-batch` skill** has
  the arithmetic and the procedure.
- **Worktrees are cheap, simultaneous builds are not.** rusty is developed by one person,
  locally or in a cloud session — not by many contributors on many cold machines, so
  don't propose tooling built for that. Each worktree compiles into its own `target/`,
  and one cold build fills every core and several GB of memory. Parallelise the work,
  stagger the compiles, and check the machine you are on (`df -h`, free memory) before starting a
  build — never a figure written down somewhere. So **a branch is written in a cloud
  session by default**: everything a container can prove (lavapipe runs the GPU tests,
  #826) goes there, which lifts the local build limit, never the one-at-a-time merge.
  A branch goes local only when it needs the machine: a real GPU (Metal, performance,
  driver races), the display, the speakers, the operator's files or keys, or a
  measurement. The `issue-batch` skill says how to launch one.
- **The lightest batch keeps only the merge queue local.** With the coding in cloud
  sessions, `make queue ARGS=--no-check` skips the local compile check of each rebased
  head and lets CI judge it, so merging costs this machine `git` and GitHub calls, not a
  build. Prefer it whenever the machine has other work. The price is that a rebase broken
  by a merge ahead (rare: a few in 126 PRs) is caught by CI a round later, not before
  the push.
- **Dependencies (not batches).** Once an issue is written, record how it relates to the
  others using GitHub's native issue **relationships** — set `Blocked by` / `Blocks`
  directly on the issue, and use GitHub **sub-issues** when one issue is literal
  groundwork for another. Two issues are linked when one **lays the groundwork** for the
  next, **makes it meaningfully easier**, or would **conflict too much** if done
  concurrently; use your best judgment. There are no rigid batches: the dependency graph
  *is* the plan. Any issue with no open blockers is fair game, and independent issues can
  be worked in parallel — stay flexible and efficient.
- **Memory is the developer's; what the project follows lives in the project.**
  Claude's local memory holds only personal preferences and machine setup. Every rule
  the project follows goes into this file, a skill or a doc: cloud coders and other
  machines never see a local memory, so a rule kept only there is a rule they break.
  A project rule found in memory is moved into the repo and pruned from memory.
- **Gates vs. signals — block on correctness, inform on quality.** Keep the bar high
  *without* stalling the agents. A check that proves **correctness** — build, test, clippy,
  the determinism guard, the size gate — is a **hard gate**: green-to-merge, no exceptions. A
  check that *audits quality* — mutation testing, coverage — is an **informational signal**:
  it runs **weekly** on `main` as the backstop, or **on request** over exactly what someone
  names (`make mutants-remote SCOPE=diff`), never on a pull request (#750), and it **never
  blocks a merge**. Nothing may silently slide, so an informational signal only earns its
  keep when it's **surfaced where the agent acts on it** (a report printed in the terminal
  or a job summary read in-context), not buried in an artifact nobody opens. Don't
  reach for a hard gate where an informational signal does the job.
- The user writes plenty of Github Issues and then does them in batches, asking you to do
  them as you see fit (regarding how many batches and which issues take priority). In those
  cases, it's best to, when in doubt of something, write a comment in the Issue, and then resume
  the work if possible, than to ask a question in the chat and spend the whole night waiting for
  an answer. However, conversations which are planning of such GH Issues don't have to wait, you
  can ask your questions right away.

## Issues, labels & priority
How an issue reads is Claude's call. What follows are this repo's rules about issues,
the ones tooling or a past mistake depends on.
- **Issues come before PRs.** The unit of work is an issue a future Claude can read
  cold — no prior context — and run unattended, even overnight. So defining issues
  well outranks opening PRs.
- **Point at code by symbol and path, never a bare line number.**
  `physics::shape::build::collider_inputs` survives the edits that land before the issue is
  picked up; `src/main.rs:210` does not. On 2026-09-30 `src/main.rs` split into
  `src/shell/` mid-batch and nearly every brief had to warn its coder that the
  issue's line references were stale. A line number is fine as a hint *beside* a
  symbol.
- **Record relationships on GitHub**, not only in prose: `Blocked by` / `Blocks`, and
  sub-issues for literal groundwork (*Dependencies*, above). `make blockers` reads
  them, and `make blockers ARGS=--fix` records a "Blocked by #N" an issue body states
  but GitHub doesn't.
- **The closing-keyword trap.** GitHub closes an issue on `closes #N` (and its
  synonyms) anywhere in a PR description, even quoted, in backticks or negated. A PR
  that leaves an issue open writes `Refs #N`, and never puts the keyword next to a
  number it doesn't mean to close. Check the number: a typo'd one silently closes the
  wrong issue.
- **Assign yourself when you start.** The moment Claude begins actively working an issue,
  **assign the user to that issue** so it's visibly taken — everyone can see it's being
  worked on right now, not just sitting in the backlog. An unassigned issue is fair game;
  an assigned one is in progress.
- **A stage label is the only absolute stop.** `planning` and `human` mean *not yet*,
  and no amount of the issue looking ready overrides them. Everything else is startable
  the moment it exists, including an issue filed a minute ago, mid-batch or not, except
  a bug whose `Kind:` names an open root issue: that root's branch does it, not one of
  its own (the operator, 2026-10-21). An idea
  still being shaped is what `planning` is for: the judgement lives in the label, so
  asking it again when work begins adds nothing.
- **Issue-less PRs are allowed only** for documentation updates or bug fixes; everything
  else starts as an issue.
- **Issues are for what matters** (the operator, 2026-10-24). A finding becomes an
  issue when it **has already caused a real problem**, or when **leaving it costs more
  than fixing it**. A code review reports what it found; Claude decides what is worth
  fixing in the branch and drops the rest without filing it. A finding nobody hit and
  that costs little is not filed. What must stay absolutely organised is the project's
  **structure and architecture and its tools**, so development stays easy and fast: a
  crack there (a slow build, a CI gap, a missing gate, a module out of shape) is always
  worth an issue. Small findings in the way (a stale doc included) are folded in;
  something worth filing outside the branch is filed rather than fixed, since a branch
  that grows to cover everything it noticed is a branch nobody can review. If a finding
  questions a decision or exposes a foundational crack, tell the user.
  **Before filing a bug** (orchestrator, coder or reviewer): look for earlier cases of
  its kind, open and closed, by symbol, module and wording; decide whether it is a
  **case** (a discrete slip) or a **symptom** (a problem that will produce more), even
  the first time. A symptom gets the **fix that scales** (a type, a lint rule, a shared
  or property test, one API), and the instance becomes its sub-issue. An instance of an
  open root issue is folded into it, never filed beside it. Every bug issue carries
  one `Kind:` line: `Kind: #N` (root N, also its sub-issue parent), `Kind: case —
  searched <terms>`, or `Kind: root`; `make board` lists the ones without it (#1173).
  The steps: `issue-batch`, *Filing a bug*.
- **Once is a case; twice is an issue** (the operator, 2026-10-18). Something that
  happens once is handled as a case. The second time, it is an issue: attack the root
  of the problem so it never becomes a bottleneck again — fix the cause, build the tool
  or add the check — and prefer what scales over a one-off fix. **A kind of bug
  counts, not only the same bug:** the second case of a kind with no root issue
  triggers one. It counts problems that happened, not ones a review imagined.
- **Every issue Claude files unprompted carries the `agent` label.** It says only who
  decided to file it, and never stops work.
- **The approval test.** A finding is filed with `planning`, and never folded in, when
  it changes the engine's design (the five kinds, the one API surface, the
  determinism rule) or a convention in this file. A stored format is not on the list:
  it changes freely (*No backward compatibility*, below).
- **Bug and doc fixes merge without waiting.** They are filed without `planning` and
  merged like any other ready PR, even when the fix changes how a scene plays.
  **Guard:** a change to the five kinds, the one API surface or the determinism rule
  never slips in under a `bug` label; it is a decision, and goes through the test above.
- **Decisions are written down, not waited on.** An issue that needs a decision states
  each one with a recommendation. The user answers, the answers go into the issue, and
  it is filed startable. The one `planning` Claude applies itself is to an *unattended*
  finding that fails the approval test; it is raised with the user at the next
  check-in.
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
- **agent** — *Written autonomously by an agent (not asked for by a person).* Says who
  decided to file the issue, nothing more: it is never a stop, and combines with any
  other label.
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

**`issue-batch`** turns the priority order above into a running batch, and
**`ci-merge`** takes each finished branch from green to merged.

## Architecture — the conceptual model
A high-level map of how the engine is shaped. It deliberately doesn't enumerate every
concrete type — that inventory lives in the rustdoc reference (`cargo doc --no-deps`)
and the script surface in `docs/api/`. Everything in the engine is one of
five kinds of moving part (Unity analogs in parentheses):

A **GameObject** is one entity (`Entity` in `components/entity/mod.rs`): a mandatory
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
   `Physics`, `Scene`, `Animator`, `Navigation`, `Camera`, `Material`, `Application`, and
   the dev-only `Debug` and `Editor`) shared by gameplay scripts, the console REPL, and bot-players.
   One surface, three callers — they never drift apart.

One thing sits outside those five: **zimmer**, the synthesiser behind `Sound.*`, is
an external crate from [scorsese](https://github.com/MatthewLacerda2/scorsese), a git
dependency pinned to a commit (#413; `deny.toml` requires a `rev` on every git dependency) — not a vendored module. `src/api/sound/` is
only its adapter (Lua → zimmer document, file writes, patch-path resolution).

## Conventions that matter
- **No backward compatibility; everything is version 1** (the operator, 2026-10-20).
  No game has been built on rusty yet, so nothing saved has to keep loading. A stored
  format (scene, prefab, save, asset sidecar) changes freely, with no migration, no
  compatibility shim, no legacy alias and no format-version bump. A PR that changes one
  updates the engine's own fixtures and the default scene in the same change.
  `Converter::version` is not a format version: it only tells the import cache to
  re-make its files, and still bumps when a converter's output changes (a test holds
  it: `asset::convert::fingerprint`, #1286). When games
  exist, the operator revisits this rule.
- **Unity is the reference; rusty is a deliberate subset.** Use Unity Engine as the
  yardstick for what rusty *must* be capable of. Unity is exhaustive, so we implement
  only the subset a game actually needs — never feature-for-feature parity. The twist is
  that our API exists to be driven by **Claude Code**, not hand-written: developers won't
  write or even read the engine's code. As long as the docs stay current, Claude can tell
  the developer how anything is done — so keeping documentation truthful is load-bearing,
  not a nicety.
- **When in doubt on how to approach, just follow Unity Engine and you should be
  good** (the operator, 2026-10-24). This holds when planning an issue and when the work
  hits a call its issue never foresaw (a default, an edge-case behaviour, how an API
  takes its arguments, which side wins when the editor and the disk disagree). Do what
  Unity does and say so in the issue or PR; most of it is well figured out. It does not
  override a decision already made, and it does not bring in what rusty deliberately
  leaves out.
- **Our engine must give support for the agentic AI to do anything a game dev can do**
  (the operator, 2026-10-24). rusty is built for an agent-driven workflow in the
  *Claude Code + Blender-MCP* style. The agent can do anything a game developer does,
  both in the editor (create entities, place and configure components, instantiate
  assets, save scenes) and around it (author and import assets, play-test, build),
  **except pure UI chrome** (collapsing a card, resizing a panel). Every user-facing
  editor capability has an API equivalent, and a dev task the agent can't do yet is a
  gap to file.
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
- **One declared layering.** Which module may import which is written once, in
  `tools/lint/src/layers/table.rs` (#724): one row per `src/<module>`, bottom-up,
  each naming the modules it may use and the external crates it may name (#938).
  Rows also mark which modules are **sim**; the rest are the platform layer.
  `make layers` checks the code against it. Read the table, not a file header, to
  answer "may X use Y?", and change the table in the same PR that adds a new
  module-to-module import or a crate use.
- **Determinism.** The sim is a pure function of (seed, inputs, fixed dt). Wall-clock
  reads, unseeded RNG and `std`'s randomly seeded `HashMap`/`HashSet` are banned from
  the sim modules (the table's `sim` rows; maps go through `core::collections`); the
  platform layer is exempt.
  Scripts run inside the sim too: the gameplay Lua VM has no `os`/`io`, and
  `math.random` routes to the seeded `Random` resource (`core::random`, #443).
  `require` / `dofile` / `loadfile` read against the project; a file outside it
  is allowed but warns once in the console, since it is an input the seed
  doesn't cover (#1062).
- **Provider calls are authoring, never gameplay.** A paid, networked call (a
  vendor generating audio, say) lives behind `src/dev/providers/` only (#383): `dev`
  builds only, refused during Play (the determinism guard scans for clocks, not
  sockets, so this rule is what keeps the network out of the sim), held to the
  project's `budget` against its running `spent`, and never made by a test — every
  provider sits behind a trait and tests use mocks. Every cost figure is an
  estimate. Keys come from the environment or a gitignored `.env`; see
  `docs/providers.md`.
- **Use `glam`** for all math (`deny.toml` bans `nalgebra` outside the physics stack, and
  `cgmath`); keep egui / wgpu / mlua decoupled.
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
- **Test content is primitive; levels are Dust2-sized.** Test levels and fixtures are
  built from primitive shapes (cubes, planes, capsules), not authored art, so they cost
  nothing to make and nothing to change. The size target for a level is CS:GO's Dust2:
  a feature that has to handle "a whole map" (navigation, lighting, streaming, a
  bake) is proven at that scale.
- **No machine state in repo docs.** Never write a machine's specs or a free-space or
  memory figure into a doc as the number to plan by; write "check the machine"
  (`df -h`, free memory). A figure quoted as evidence of a past incident, dated, is
  history, not a limit.
- **One format inside, glTF; everything else is converted at import.** The engine's
  loaders read glTF 2.0 (first-class) and `.obj` (static meshes only). Other sources are
  turned into a cached glTF when imported, the way Unity read only FBX and had Blender
  export one from a `.blend`; the source stays in the project, the cache is re-made when
  it changes, and nothing past the importer learns another format (operator,
  2026-10-17):
  - `.fbx` is converted inside the engine with the `ufbx` crate (#1024);
  - `.blend` is converted by **Blender**, run headless (#1072; `RUSTY_BLENDER` names a
    non-standard install).
  **Blender 5 or later is a requirement**, with the Blender MCP set up: rusty assumes it
  is installed, and when it isn't, the import says so and asks for it — no fallback
  parser and no promise of support without it. Running Blender is **authoring only**
  (editor, MCP, dev tools): a shipped game reads the cached glTF and never runs Blender,
  and the sim only ever sees glTF. The engine never parses `.blend` itself.
  `asset::MODEL_EXTENSIONS` is the one list of what the importer reads.
  The `subprocess` rule (`make rules`) holds where a process may start: `src/` never
  names `std::process::Command` without an inline reason (#975).

## Commit gate (programmatic — no AI needed)
`make gates` runs everything CI blocks on, fastest-failing first; run it before
readying a PR (`make help` lists the verbs, `make check` is the fast edit loop).
Failures from `tools/lint` are written to `.lint/report.txt`. See **docs/linting.md**.
- **The commit hook** (`.githooks/pre-commit`, activated once per clone by `make
  setup`) runs formatting and the size gate only, so it stays under a second; the
  push hook (`.githooks/pre-push` → `resolved.py --hook`) runs rustfmt on both crates,
  `tools/lint` included, when a pushed tip is `HEAD`, covering the commits a
  `git rebase --continue` made; `make resolved` runs the same check unconditionally.
- Size gate: files ≤ 300 lines, test/fixture files ≤ 150 (how a `.py` file is
  counted: `.github/scripts/CLAUDE.md`). Style is rustfmt;
  **clippy is a hard gate** (`-D warnings`: default, `dev`, and the no-editor player
  build); the lint policy
  lives in `Cargo.toml`'s `[lints]`, not in flags.
- **Layering lint** (`make layers`) — fails when a `crate::<module>` import is not
  in the layer table, when a table entry is stale, on any cycle, and when a sim
  module imports a platform one (so leaks through a middle module fail too). Test
  code is exempt. Known exceptions sit in the table, each with its open issue.
- **Determinism guard** — clippy bans wall-clock / unseeded RNG / `std` hash maps crate-wide
  (`clippy.toml`'s `disallowed-*`), and `make determinism` fails when anything but a
  platform module's root opts out; it protects the harness's reproducibility.
- **Direction guard** (`make direction`) — fails when a sim module references
  `crate::render`, `crate::editor`, `wgpu` or `egui`; the arrow is render/editor → sim.
- `make gates` refuses to run when cargo's target dir is outside the worktree — a
  shared one is a false green.
- **Nothing is grandfathered.** A file over its cap is split, not excused; no gate
  reads a baseline file (#926). An exception a check needs is written inline, with its
  reason. The one committed list of numbers, `coverage-baseline.txt`'s floors, only
  ratchets up: `make scripts` fails a floor lowered without an inline `# reason:` (#963).

## Overrides
Any rule in this file may be overridden by the user's explicit say-so — in the current
prompt or a previous one. The **one exception**: an issue tagged **planning** must never be
started while that tag is on it. The user may tell you to **remove the `planning` label
and then do it** — but never to do it with the label still on. (The user *may* greenlight an
issue that is **blocked by** another; doing so automatically lifts that block.)
