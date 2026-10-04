---
name: issue-batch
description: Run a set of issues from board to merged — how many branches at once, which ones can safely run together, worktrees, re-reading the board, and the cleanup and local rebuild that finish a batch. Use when starting work on one or more issues, when deciding what to start next, or when told to "do the issues".
---

# Working a batch of issues

The user rarely has one issue. They write plenty of them and then ask for them in
batches, leaving how many and in what order to you. This is how a set of them gets
worked without the batch costing more than the work.

## Two branches in flight, pipelined

Coding parallelises. **Merging does not** — rusty is one compiled crate, so merges
are serialized, and the queue is the bottleneck.

Every branch that is not first pays a rebase for each merge ahead of it. Over
shared code that is **N(N−1)/2 rebases**: two branches cost one, three cost
three, four cost six. A rebase buys no correctness.

So: **one in the merge queue, one being written.** Nothing idles through a
ten-minute CI run, and nothing rebases twice.

**The merge queue is `make queue ARGS=--watch`** (the `ci-merge` skill has the
detail): started once, in the background, it takes each pull request the moment
it turns ready, highest label priority first, and rebases, pushes, waits for CI
on the new head, asks `make mergeable` and squash-merges, one at a time — so the
session running the batch is not the one sitting through each run, nor the one
noticing each draft flip to ready (#664). It **exits on the first hand-back**
(conflict, red, no run), on the machine failing, or when nothing is left, and
that exit is what wakes you: read the report, deal with the hand-back, start it
again. Start it with `--for 70` (#697; `ci-merge` has why 70, and when to
trade it for a longer per-PR `--deadline`). The watch ends itself first, between
pull requests, with the last line `watch ended: deadline reached …` — relaunch it
as it was, nothing to read. It never resolves a conflict: a hand-back naming paths goes back to the
branch's author. `make queue PRS="a b c"` still takes named pull requests in the
order given (a hand-back there skips the entry and the rest carry on), and
`ARGS=--dry-run` shows what either would do and writes nothing.

## The real limit is file collision, not count

Two branches adding a variant to `ComponentKind` cost more than four branches in
genuinely separate areas. A clean rebase is seconds of `git`; a colliding one is
a whole session.

**Before starting a second branch, ask: does it edit the same types as the
first?** A branch in `src/navigation/`, one in `src/render/postfx/` and one in CI
config barely touch each other. Two branches both adding a first-class component
will collide every time.

The files where everything collides are the ones every feature appends to:

- `src/components/entity.rs` — the `Option<…Component>` fields, twice over (the
  live struct and its serde mirror).
- `src/scene/authoring/components.rs` — the `ComponentKind` variant list (`ALL`
  is generated from it, so keeping both sides' variants is the whole resolution).
- `src/api/mod.rs` — the `pub mod` list, the namespace roll-call in the crate doc,
  and the registration body.
- `src/app/registry.rs`'s `build()` — where order *is* the per-frame execution
  order, so a merge that reorders it changes behaviour silently.
- `src/editor/inspector/components/add/` — the Add Component menu.
- `src/scene/authoring/dependency.rs` — `set_default`, `has_kind`, `clear_one`:
  dispatchers that gain one arm per component and cross clippy's 50-line cap when
  two component branches land together (#699 + #703 on 2026-10-02). Split such a
  dispatcher by group (UI kinds behind `set_default_ui`), never trim it line by line.
- `src/components/entity/repr.rs` imports and `docs/api/Scene.md`'s component
  list — both sides add one line at the same spot every time.
- `docs/api/index.md` — the namespace list and the lifecycle callbacks, and a
  hard gate parses it. The namespace tables are one file each since #569, so two
  branches documenting different namespaces no longer collide.
- `docs/api-faithfulness.md` — the setter catalog.
- `src/scripting/callbacks.rs` — the one lifecycle-callback list.
- The burn-down baselines (`tools/lint/baseline.txt`,
  `components_baseline.txt`, `parity_baseline.txt`, `coverage-baseline.txt`),
  where two branches each *removing* lines is a conflict nobody expects.

Two branches landing in any of those at once is the case to avoid.

## Group the work before splitting it

**Split by responsibility, not by parallelism.** If a parent's sub-issues all
touch the same type, they are **one branch**, not one each.

Splitting an issue so several agents can run at once optimises the half that was
never scarce, and manufactures collisions: a new first-class component's four
axes — the `Entity` field, the Add Component entry, the inspector card, the API
namespace and its doc row — are one coherent change that the completeness gate
will not accept in pieces anyway. Four sub-issues there is four rebases, four CI
cycles and one gate failing four times.

Sub-issues are for work that is genuinely separable *in the code* — not for work
that is merely listable.

## Each branch gets its own worktree

One checkout per branch under `.claude/worktrees/` (gitignored), never two
branches taking turns in one. A shared checkout mixes another issue's edits into
the gate run and thrashes `target/`.

**Never set `CARGO_TARGET_DIR`.** Cargo keys artifacts by package, version,
features and profile — never by source path — so worktrees pointed at one target
directory overwrite each other's output and produce a false *green*: gates
reported on code that was never compiled, which is exactly the claim a **ready**
pull request makes. Cargo's default is already right, so the rule is to stop
overriding it. Nothing in this repo enforces that — no gate refuses to run under
an override — so it is on you.

**Cores and memory bound the builds now, not disk.** The limits, measured on the
operator's 8-core / 15 GB machine on 2026-09-30 (#497 has the raw numbers; on a
different machine, re-measure rather than trust them):

- **Three local agents at most, and at most two heavy builds at once.** A heavy
  build is a cold build of a fresh `target/` or a `make gates` run; an incremental
  rebuild after an edit (seconds, about 1 GB) is not one. A third heavy build fits
  in memory but buys nothing: one cold build already fills 8 cores, so two side by
  side take exactly as long as the same two back to back. The second slot exists
  so an agent is not queued behind another's build, not for throughput.
- **Give each heavy build half the cores** (`CARGO_BUILD_JOBS=4` on 8 cores)
  whenever another may overlap it. Two capped builds finished as fast as two
  uncapped ones and peaked about 2 GB lower. A build alone runs uncapped.
- **Check before a heavy build**, on the machine you are on, today: `free -g`
  shows about 5 GB available (one build adds about 3 GB, the rest is margin), and
  `df -h` about 10 GB free. A built worktree is ~2.5–4.5 GB, no longer 12–16.
  Running out of disk still shows up as `No space left on device` at the link
  step — the tell-tale ENOSPC.
- **A local session that touches Rust also runs rust-analyzer** (the
  `rust-analyzer-lsp` plugin, #488): about 4 GB resident once it has analysed the
  crate (measured 2026-10-02: ~1 min to load, ~1 min to analyse). Count it against
  the cap above: three such sessions are ~12 GB of language servers before anything
  compiles, so on a 15 GB machine the practical limit is two local agents. Coding
  goes to cloud sessions, which bring their own memory.

`docs/testing.md` has the arithmetic.

**Reclaim the moment a branch merges** — `rm -rf .claude/worktrees/<dir>/target`
and remove the worktree. Disposal is what keeps disk from becoming the overnight
failure.

## Cloud sessions are extra coders, not extra merges

The build limits above are this machine's cores and memory, not the workflow's. A **cloud
session** brings its own CPU, memory and disk, so it lifts the cap on how many
branches can be *written* at once. It does nothing for merging — that queue is
still one CI run at a time, here — so reach for cloud when writing is the
bottleneck (hours-long foundation branches), never when the queue is.

**Where a branch runs:**

- **Local:** the batch's own session; every merge; any branch whose proof needs a
  **real** GPU (Metal-specific behaviour, performance, anything a software driver
  can't stand in for); anything that needs files only this machine has. Ordinary
  render/screenshot/probe-bake tests run in a cloud container once it installs
  Mesa's lavapipe (`apt-get install mesa-vulkan-drivers libvulkan1`), exactly as
  Linux CI does (#489).
- **Cloud:** a branch that needs no real GPU to prove **and** sits in modules no other
  in-flight branch touches. The collision list above still decides that; cloud
  removes the build-slot limit, not the rebase cost.

**How many:** at most **four branches in flight in total**, local and cloud
together, no more than three of them local. Each one behind another still pays a
rebase per merge ahead of it, and a full queue drains about one pull request per
10–15 minutes (one CI run each, seen 2026-09-30): four finishing together leave the
last waiting most of an hour with three rebases paid. Past four, the queue is the
bottleneck and more coders only lengthen it.

**Launching one — and proving it is one.** Not with the `Agent` tool's
`isolation: "remote"`: in a local session that silently falls back to a local
worktree (seen 2026-09-30). What works is a **one-off cloud routine** — the
`RemoteTrigger` tool (the `schedule` skill has the body shape) with `run_once_at` a
minute or two out, the repository as its source, and the whole brief as its prompt.
Two traps decide whether it is really remote:

- **Pick the `anthropic_cloud` environment, never a `bridge` one.** The environment
  list includes a *bridge* to the operator's own machine (`archlinux:…`); a routine
  on it runs **here**. This is the likely cause of past "cloud" sessions that ran
  locally.
- The brief's **first instruction**: run `echo "$CLAUDE_CODE_REMOTE"; pwd` (the
  variable `.claude/hooks/session-start.sh` keys on). If it is not `true`, or the
  path is the operator's home, **stop before touching anything** and report that
  the session is local.

Then check from here: `list_runs` / `get_run_log` on the routine show the session's
first tool result (`CLAUDE_CODE_REMOTE=true`, a `/home/user/…` path), and no new
worktree under `.claude/worktrees/` or local `cargo`/`rustc` process appeared for
that branch. The cloud session has GitHub MCP tools (`create_pull_request`,
`add_issue_comment`), so it can open and update its own pull request.

**The pull request is the report.** A cloud session cannot message this one back.
Brief it to open a draft on its first commit, push often (a dead container takes
its uncommitted work with it), write decisions and hand-backs into the description
and an issue comment, and mark the PR ready when its gates are green. Watch the
PR, not the agent.

**Ready means finished.** The batch merges a ready PR the moment its gates are
green, so a PR must never be readied just to make CI run. On 2026-09-30 #520's session
readied its PR to get Windows CI on a temporary 200-round test loop, and it merged with
the loop still in (#559 reverted it). For a proof run on a draft, dispatch the
workflow on the branch instead: `gh workflow run ci.yml --ref <branch>` runs
the gates (`build-test`, `build-test-cross`, `deny`, `ci-gate`) and nothing else,
without readying anything. Mutation and coverage are not in `ci.yml` any more (#750):
a branch that adds mechanism asks for a scoped mutation run instead
(`make mutants-remote SCOPE=diff`, or a `mutants-on-request.yml` dispatch).
Never dispatch `mutants-sweep.yml` for a branch: that is the full sweep, five
runners for most of a day taken from every other PR's gates (#705).

**A cloud session is never woken by its own background work.** A routine session
that starts a build in the background and ends its turn to wait sits idle forever —
#485's first session did exactly that for an hour, gates half-run, PR still a draft.
Brief every cloud session to run builds and gates in the **foreground** (long
timeouts, split across calls), and to schedule its own check-in with `send_later`
(the `Claude_Code_Remote` MCP tool) before ending a turn to wait on CI. From here,
`worker_status: idle` on `list_runs` with a draft PR is the stall's signature; the
fix is a fresh routine briefed to finish the pushed branch.

**The container's disk is finite too:** before #483 folded `tests/` into one binary,
the dev-feature test pass filled it. (`cargo deny`'s proxy trap needs no brief
line any more: the repo's `.cargo/config.toml` fetches with the system git, #572.)

**What it costs,** so it is chosen on purpose: every cloud session cold-builds the
whole dependency tree, with no local compile cache to help. Have it run the full
gate list before readying — it has the room for both feature sets, which a crowded
local machine may not.

## Running a mixed batch: one orchestrator, cloud coders, a local slot

This is the shape that ran the 2026-09-30 batch (≈40 pull requests merged in one
day). Each role does what only it can do.

**The orchestrator (this session, local)** writes no feature code. It picks work,
briefs cloud coders, reviews and merges. It **starts `make queue
ARGS="--watch --no-check --for 70"` in the background** (from a worktree under
`.claude/worktrees/`), relaunches it unchanged each time it exits on its
deadline (`watch ended: deadline reached`, #697: the first overnight watch was
killed at the two-hour background cap mid-CI-wait), and is otherwise woken only
when the watch exits — never a
per-PR polling loop or a chain of `make queue PRS=N` runs (#664: the 2026-09-30
batch spent about 69 queue invocations and dozens of hand-built watchers on
that, and one chain was cancelled by mistake). Its loop per pull request:

1. Read the description. Check the decisions against the issue, and note any
   human-only checks (a window, speakers) as a checklist; don't hold for them.
2. **The watch does** rebase → check → push → wait → merge; don't hand-roll
   that loop. The queue rebases in a throwaway worktree, runs
   `cargo check --locked --all-targets` with `dev` and `--no-default-features`
   plus the size gate on the rebased tree **before** pushing (#571), and hands
   the branch back — unpushed — on a conflict or a failed check. A clean
   textual rebase still breaks when a merge ahead changed a signature: on #538,
   #542 had removed a `Renderer::render` argument that #538's new tests still
   passed, and CI caught it ten minutes later; a rebase of #563 onto #558 and
   #564 pushed a file to 308/300 lines. It then waits for the runs on the
   rebased head and merges only on `make mergeable`'s verdict, which reads a
   cancelled run beside its still-running replacement as "wait", not red
   (#562). The local check is a heavy build on a cold target, so it takes a
   build slot like any other. Run it from a worktree under `.claude/worktrees/`,
   never the scratchpad (a tmpfs a cold build fills, #580).
   **`ARGS=--no-check` drops that build**: the rebase is pushed and CI is the
   only judge. With every coder in the cloud, that makes the batch's whole local
   footprint `git` plus `gh` (CLAUDE.md, *The lightest batch*), so use it whenever
   the machine is shared or busy. What it gives up is catching a semantic break
   before the push. That cost three breaks in the 126-PR batch of 2026-09-30, each
   a CI round instead of a three-minute check, and each still handed back unmerged.
   **The watch exits at once when no pull request is open at all**, which is the
   state right after a wave of coders launches and before their first drafts
   exist. Start it once a draft is up (or poll for the first ready one yourself).
3. A hand-back is the queue's whole report, and the watch exits on it: fix a
   conflict or a failed check on the branch (or brief its coder to), and start
   the watch again right away — the other ready pull requests are waiting. The
   handed-back head is passed over until it moves, so restarting never re-takes
   an unfixed branch; once its fix is pushed the watch takes it again on its
   own (`make queue PRS=N` takes it regardless). A one-line semantic
   break (a field a merge ahead added) is quickest fixed here — but with
   `--no-check` nothing compiles your resolution before CI does, so **grep every
   use of anything either side moved or renamed, and check the 300-line file and
   50-line function caps**, before pushing. On 2026-10-02 a hand-resolved #700
   kept a variable #699 still used, and only the coder's check-in caught it. A
   resolution that needs a compile to trust (a file split, a dispatcher over the
   cap, two systems' order) goes to a cloud routine instead.
   **A coder's own check-in may push while the queue holds its PR**; the queue
   hands it back as "somebody else pushed". Wait for that coder's PR comment,
   then `make queue PRS=N`. A many-hunk
   conflict goes back to **the session that wrote the branch**, which still holds
   its design: the coder's `send_later` check-in is a routine bound to its
   persistent session, so `update` that routine's prompt with the rebase request
   (paths, what merged, "force-push authorised"), `run` it, then disable it so its
   scheduled fire does not repeat the request. #660 came back that way after #659,
   27 hunks resolved in one pass with a new test for the interaction. No check-in
   routine left: a fresh routine briefed to rebase the pushed branch.
4. After merging: remove the worktree and its `target/`, re-read the board, and
   start the next piece of work.

**Cloud coders write the branches.** Pick an issue for the cloud when it needs no
real GPU and **no in-flight branch works in the same module**. Run one branch per
module at a time: tonight physics went #445 → #521 → #446 → #447, and UI went
#417 → #418 → #419 → #420, each started when the previous one merged. Parallelism
comes from spreading across modules (physics, UI, navigation, audio, scripting,
CI) at once, not from stacking work in one.

**The local slot takes what only this machine can do:** measurements that decide
something (#487, #492, #497, #552 each put a before/after table in their pull
request), fixes that need its real GPU (#518's driver race), and the
orchestrator's own compile checks. Don't let the orchestrator compile while a
local agent is timing builds; it skews the numbers.

**Every cloud brief carries** step 0 inline, then a pointer to
[`cloud-brief.md`](cloud-brief.md) — the standing rules (foreground builds,
`send_later`, ready means finished, the repo's rules, the PR protocol, "do not
merge") live there, versioned with this skill (#568) — and only what is specific
to its issue. `RemoteTrigger` echoes a prompt back three times, so standing text
copied into each one fills the orchestrator's context; a change to the rules goes
in the file, once. The prompt:

```text
STEP 0 — before anything else, prove you are a cloud session. Run
`echo "CLAUDE_CODE_REMOTE=$CLAUDE_CODE_REMOTE"; pwd; uname -a`. If it is not
exactly `true`, or the path is under the operator's home, STOP: touch nothing,
end with "LOCAL — aborted" and that output.

Then read `.claude/skills/issue-batch/cloud-brief.md` and follow it.

Issue(s): #N — <one line>. Branch: `N-short-slug`.
Builds on: <what merged that it must build on; line numbers in issues go stale
within hours>.
Siblings in flight: <branch/issue → the files it edits; stay out of them>.
Decisions: <anything already settled, or "none">.
```

No helper script fills it: `RemoteTrigger` is a tool the orchestrator calls, not
an API a script can reach, and five slots don't need one.

### What the orchestrator keeps, and where

Lessons scorsese's batches paid for (MatthewLacerda2/scorsese, 2026-10-02/03);
they hold for any repo running this workflow.

- **GitHub is the batch's state.** Open PRs, their draft/ready state and
  comments, plus `RemoteTrigger list` for the routines, are enough to resume a
  batch from nothing. The orchestrator's own notes (a routine table, which coder
  holds which issue) are conveniences. Never keep anything **only** in `/tmp`:
  the scratchpad lives there, and a reboot wipes it. A machine crash mid-batch
  on scorsese took its queue list and routine table with it, and the batch
  resumed from GitHub alone.
- **Never wait on another process by `pgrep -f <text>`.** The waiting shell's
  own command line contains the text, so it waits on itself forever (about 25
  minutes lost on 2026-10-02). Wait on a PID you hold, or on the watch's own
  exit. With one `make queue ARGS=--watch` there is nothing to chain: it takes
  every ready PR, and the order lives on GitHub, not in a list.
- **Watch each PR's head commit, not only its draft/ready state.** A rebased PR
  stays *ready* the whole time, so a watcher keyed on state never sees the push.
  The queue's watch keys on the head for exactly this reason; your own reading
  of a re-pushed PR still waits for the coder's PR comment ("rebased, gates
  green"). If a push should *not* be retaken yet, flip the PR back to draft
  first: rusty's queue takes ready PRs, there is no `queue` label.
- **Merge the broad PR last among those that share lists.** A PR that touches
  every registry (a new first-class component, a new namespace) sends every
  sibling appending to the same lists back with a conflict if it lands first.
  On scorsese, 2026-10-03, one broad PR landing first handed back three small
  ones, rebased one after another at about an hour apiece; small ones first
  would have cost one hand-back, on the broad PR. Until the watch orders by
  conflicts itself (#753), hold the broad PR in draft until its small siblings
  have merged.

## Starting

- Run `make blockers ARGS=--fix` once at the start of a batch: it records every
  "Blocked by #N" an issue body states but GitHub never recorded (#624).
- Assign the user the moment work begins — unassigned means fair game.
- **Unassign** if it turns out the issue was never started.
- Branch `{issue_number}-short-slug` off the latest `main`; an issue-less pull
  request uses a readable slug.
- Open a **draft** pull request on the first commit — that is the standing rule,
  not something to be asked for. Draft is how work survives a session that ends
  badly; the issue is the durable context, and a hand-back comment may never get
  written.

## Re-read the board after every merge

A merge changes the graph. Whatever the merged issue blocked is fair game the
moment it lands — so the decision is one merge wide, not one batch wide.

**Re-read the whole board, not your list of it.** Coders file what they find
(seven issues mid-batch on 2026-10-02, two of them red-main and a test race), and
the operator files from other machines. A batch that only tracks the issues it
started stops early and misreports what is left. That happened once on
2026-10-02: the operator had to point out the board was not done. Count what is
left from `gh issue list`, never from memory.

But re-reading is not a licence to start everything: **start the next one, and
keep the second slot for whatever is furthest along.** The label order decides
which one that is. An unblocked issue left unstarted is not wasted capacity; it is
a rebase not yet paid for.

**Dependabot pull requests** (`.github/dependabot.yml`, monthly, one grouped PR per
ecosystem) have no issue, which is allowed for maintenance. A batch merges them like
any other ready pull request, through the same serialized queue, at the **lowest
priority** — after every labelled issue in flight. Dependabot rebases its own
branch when `main` moves and cancels its own runs as it does, so a bot pull
request often reads *not mergeable* between rebases; `make queue` never
force-pushes one — it asks `@dependabot rebase` and waits for the new head.

**A stage label is the only absolute stop.** `planning` and `human` mean *not
yet*, and no amount of the issue looking ready overrides that. Everything else is
startable the moment it exists, including an issue filed a minute ago. An issue
filed mid-batch can outrank the work in flight: when a new label-priority leader
appears, it is the one started next.

## Briefing a subagent

Point it at `CLAUDE.md` first, then the issue — issues here are written to be
read cold. Beyond that:

- Name the **base commit** and what has landed recently that it must respect.
- Name the **siblings** and which files they are touching.
- Tell it to invoke the **`ci-merge` skill** rather than restating that protocol.
- For a **cloud** session: launched as a one-off routine on the `anthropic_cloud`
  environment, the `CLAUDE_CODE_REMOTE` self-check comes first, and the pull request
  is its only way to report (see above).
- Tell it **not** to merge — merging is serialized and belongs to the session
  running the batch.
- Tell it not to start a heavy build while two siblings are already compiling,
  to give it half the cores (`CARGO_BUILD_JOBS`) while one is,
  and not to run `cargo mutants` locally at all during a batch: when the branch
  adds mechanism, it asks for a scoped run on GitHub's runners instead
  (`make mutants-remote SCOPE=diff`; nothing runs mutation per pull request, #750).
- **Scratch filenames must carry the issue number.** The scratchpad is shared
  between sibling agents, and a collision swaps one pull request's description
  for another's.

## Model, as a hint

Judgement work — design, implementation, triage — wants the strongest model. A
rebase, a module-list conflict, an attribute moved between files does not. Most
sessions on a branch are the second kind. The line is not crisp, so err upwards.

## When to hand back to the user

- ≈3 attempts at the same failure.
- A decision that is genuinely theirs: a change to the five kinds, a departure
  from what the issue decided, anything a `planning` label would have carried.
  A call the issue simply did not foresee is **not** one of these when Unity has
  an answer: take Unity's (CLAUDE.md, *When in doubt, Unity's way*), name it in
  the PR, and carry on.
- **Save, ask, move on.** Push what exists (a dead session takes uncommitted work
  with it), mark the pull request **draft**, write what is needed and why in the
  description and an issue comment, then **take the next startable work**. The
  branch stops, never the batch (operator, 2026-10-02: "the work is saved but the
  progress doesn't stop"). Do not thrash on the stopped branch.

When working unattended, prefer leaving a comment on the issue and continuing
over stalling the night on a question. Questions asked *while planning* are asked
right away.

**A human check is never a merge hold.** Some proof only a person can give: a real
window, real speakers, taste. When a green pull request carries such a check, merge
it anyway, and put the checklist in the PR description and an issue comment for
the user to run later. If it turns out broken, that is a bug to file and fix, not
a reason the batch waited. The same goes for a judgement call the issue left open:
take the default the issue, CLAUDE.md or Unity points to, write down what you
chose and why, and keep going. The batch exists so the user does not have to be
here. (On 2026-09-30, holding three PRs for a smoke test and an ear check stalled
the queue for hours; every one was fine.)

## Learn from the batch: every lesson ends as a change

A batch is trial by fire: it finds the traps, gaps and bugs in the tooling and in
the engine faster than anything else. A lesson that only reaches the chat is lost,
because the next session starts without it. So every lesson ends as exactly one of:

- **fixed**: a pull request that removes the cause;
- **enforced**: a lint, gate or test that makes it impossible to repeat;
- **documented**: this skill, `ci-merge`, `issue-write` or CLAUDE.md, for what the
  repo can't change (a platform behaviour, a procedure);
- **tracked**: an issue with the evidence (run ids, PR numbers, measurements) and a
  proposed fix, never just the symptom.

**Mid-batch, fix what blocks or bites twice.** When a lesson costs the running batch
time, such as a tooling bug that stalls the queue or a brief that makes every agent
re-solve the same trap, fix it now. It is infrastructure, so it outranks whatever
was next. A skill or doc edit is Markdown-only and merges without queuing. On
2026-09-30, for example: #515 (red gates on cancelled runs), #555 (merges waiting
on the mutation signal), and #519, #561 and #566 (skill fixes landed within the
hour the lesson appeared).

**Otherwise, file it and keep going.** The strongest issues come from doing the
work. File an agent's "follow-up, not in this PR" notes and a PR's "decisions to
check" the moment you read them, not at the end.

**At the end, run a retro before reporting back:**

1. List every trap, gap, surprise, workaround and hand-back from the batch: your
   own notes, each PR's *Decisions* and *Not exercised* sections, the issues the
   agents filed.
2. Map each one to fixed, enforced, documented or tracked. Anything unmapped gets
   an issue now.
3. **Refresh the unstarted issues whose ground moved.** A batch invalidates line
   references and assumptions fast, so add a short *Context update* comment to the
   issues next in line: what merged that they build on, and what to rebase over.
   Re-scope or close any that the batch made moot. Never touch a `planning`
   issue's scope.
4. Put the mapping in the report, so the user sees each lesson turned into a change
   rather than only described.

## Finishing a batch

The batch is not done when the last branch merges — it is done when the main
checkout runs what was merged. Last, once nothing is compiling:

1. **Clean up what is finished.** `make queue`'s summary lists what it merged and
   deliberately removes nothing. Remove the worktree and delete the local branch
   of every pull request that is `MERGED` — ask `gh pr view N --json state`, never
   its exit code, which is 0 for an open one too. `git branch --merged` cannot see
   a squash merge, so it is not the test. A branch with **no commits beyond
   `main`** (`git rev-list --count origin/main..BRANCH` is 0) goes too, once its
   worktree has nothing uncommitted and no agent is still standing in it. Anything
   else stays: unmerged work is only ever deleted by the user.
2. **Bring the main checkout up to date** — `git pull --ff-only` on `main`.
3. **Rebuild the MCP bridge, and launch nothing.** A local client is pointed at
   `target/debug/session-mcp` (`docs/mcp.md`), so until
   `cargo build --bin session-mcp --features dev` runs in the main checkout, every
   session drives the old engine. Never beside a sibling's build. A client already
   connected keeps the old process until it reconnects; say so in the report.

## Reporting back

The user is not reading the transcript of a batch. They take long — often hours,
usually overnight — and the transcript is, if anything, notes for Claude itself.

**When things go well, say what the result was.** When things did not go as one
would expect, say what the surprise was. That does not necessarily mean things
went badly: we write it down because the more we can predict, the better we
improve.

Two things still interrupt, because they are the ones the user would want to
overrule and overruling is only possible while the batch is still running: **a
change to the user's own files** outside the repo, and **a decision reversed** —
where the issue said one thing and the branch did another.
