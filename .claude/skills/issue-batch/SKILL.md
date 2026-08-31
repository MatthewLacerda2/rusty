---
name: issue-batch
description: Run a set of issues from board to merged — how many branches at once, which ones can safely run together, worktrees, and re-reading the board. Use when starting work on one or more issues, when deciding what to start next, or when told to "do the issues".
---

# Working a batch of issues

The user rarely has one issue. He writes plenty of them and then asks for them in
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

**Disk is the binding constraint, and it fails as a link error.** Each worktree's
`target/` is ~12–16 GB and the session volume is ~38 GB usable, so **at most ~2
heavy builds at once**; serialize the rest. Check `df -h` for ~16 GB of headroom
before launching one. Running out shows up as `No space left on device` at the
**link** step, not at compile — the tell-tale ENOSPC. `docs/testing.md` has the
arithmetic.

**Reclaim the moment a branch merges** — `rm -rf .claude/worktrees/<dir>/target`
and remove the worktree. Disposal is what keeps disk from becoming the overnight
failure.

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

**`plan` is the absolute stop.** It means *not yet*, and no amount of the issue
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
- Tell it **not** to merge — merging is serialized and belongs to the session
  running the batch.
- Tell it not to start a heavy build while two siblings are already compiling,
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
  change to the five kinds, anything a `plan` label would have carried.
- Mark the pull request **draft**, say why in the description, and stop. Do not
  thrash.

When working unattended, prefer leaving a comment on the issue and continuing
over stalling the night on a question. Questions asked *while planning* are asked
right away.

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
