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

**The merge queue is `make queue PRS="a b c"`** (the `ci-merge` skill has the
detail): it rebases, pushes, waits for CI on the new head, asks `make mergeable`
and squash-merges, one at a time, in the order given — so the session running
the batch is not the one sitting through each run. Hand it the ready pull
requests in label-priority order; a hand-back (conflict, red, no run) skips that
entry and the rest carry on, so read its summary rather than assuming the whole
list landed. It never resolves a conflict: a hand-back naming paths goes back to
the branch's author. `ARGS=--dry-run` shows what it would do and writes nothing.

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
- `src/scene/authoring/components.rs` — `ComponentKind`, and `ALL` with its
  hard-coded `[ComponentKind; 10]` length, which neither side's diff gets right.
- `src/api/mod.rs` — the `pub mod` list, the namespace roll-call in the crate doc,
  and the registration body.
- `src/app/registry.rs`'s `build()` — where order *is* the per-frame execution
  order, so a merge that reorders it changes behaviour silently.
- `src/editor/inspector/components/add.rs` — the Add Component menu.
- `docs/scripting-api.md` — 1700 lines of namespace tables, and a hard gate
  parses it.
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
`build-test` and `build-test-cross` without readying anything.

**A cloud session is never woken by its own background work.** A routine session
that starts a build in the background and ends its turn to wait sits idle forever —
#485's first session did exactly that for an hour, gates half-run, PR still a draft.
Brief every cloud session to run builds and gates in the **foreground** (long
timeouts, split across calls), and to schedule its own check-in with `send_later`
(the `Claude_Code_Remote` MCP tool) before ending a turn to wait on CI. From here,
`worker_status: idle` on `list_runs` with a draft PR is the stall's signature; the
fix is a fresh routine briefed to finish the pushed branch.

**Two environment quirks every brief should carry:** `cargo deny` cannot fetch its
advisory database through the cloud proxy with its built-in fetcher — set
`CARGO_NET_GIT_FETCH_WITH_CLI=true`. And the container's disk is finite too: before
#483 folded `tests/` into one binary, the dev-feature test pass filled it.

**What it costs,** so it is chosen on purpose: every cloud session cold-builds the
whole dependency tree, with no local compile cache to help. Have it run the full
gate list before readying — it has the room for both feature sets, which a crowded
local machine may not.

## Running a mixed batch: one orchestrator, cloud coders, a local slot

This is the shape that ran the 2026-09-30 batch (≈40 pull requests merged in one
day). Each role does what only it can do.

**The orchestrator (this session, local)** writes no feature code. It picks work,
briefs cloud coders, reviews and merges. Its loop per ready pull request:

1. Read the description. Check the decisions against the issue, and note any
   human-only checks (a window, speakers) as a checklist; don't hold for them.
2. If the branch is behind `main`, **rebase it here and compile it here** before
   pushing: `cargo check --all-targets --features dev` and
   `--no-default-features`. A clean textual rebase still breaks when a merge
   ahead changed a signature. On #538, for example, #542 removed a
   `Renderer::render` argument that #538's new tests still passed, and CI caught
   it one round later than a local check would have.
3. Push, then merge only when `make mergeable` exits 0. Treat a refusal as final
   only when no run on the head is still in flight; superseded runs get
   cancelled and replaced within seconds.
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

**Every cloud brief carries:**

- step 0, the `CLAUDE_CODE_REMOTE` self-check;
- builds in the foreground, `send_later` for waits, and "ready means finished";
- **what merged tonight that it must build on**, since line numbers in issues go
  stale within hours, and which siblings are in flight in which files;
- the repo's current rules (one test binary, size caps, GPU test naming,
  lavapipe for render tests, `CARGO_NET_GIT_FETCH_WITH_CLI=true` for `deny`);
- "do not merge".

## Starting

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

**`planning` is the absolute stop.** It means *not yet*, and no amount of the issue
looking ready overrides it; `human` is the same in practice. Everything else is
startable — **except an issue filed minutes ago that is still settling**: rusty
does not file-and-start unless the work is a direct consequence of an
already-decided issue. When in doubt about a fresh one, it is the user's call.

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
  and not to run `cargo mutants` locally at all during a batch — CI runs it
  diff-scoped per pull request anyway.
- **Scratch filenames must carry the issue number.** The scratchpad is shared
  between sibling agents, and a collision swaps one pull request's description
  for another's.

## Model, as a hint

Judgement work — design, implementation, triage — wants the strongest model. A
rebase, a module-list conflict, an attribute moved between files does not. Most
sessions on a branch are the second kind. The line is not crisp, so err upwards.

## When to hand back to the user

- ≈3 attempts at the same failure.
- A decision that is genuinely theirs: an API name game scripts will type, a
  change to the five kinds, anything a `planning` label would have carried.
- Mark the pull request **draft**, say why in the description, and stop. Do not
  thrash.

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
