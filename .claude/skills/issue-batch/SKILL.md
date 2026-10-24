---
name: issue-batch
description: Run a set of issues from board to merged — how many branches at once, which ones can safely run together, worktrees, re-reading the board, and the cleanup and local rebuild that finish a batch. Use when starting work on one or more issues, when deciding what to start next, or when told to "do the issues".
---

# Working a batch of issues

The user writes many issues and asks for them in batches, leaving how many and in
what order to you. CLAUDE.md already holds the rules a batch runs on (the limits
per kind of agent, one worktree per branch, stage labels, branch names, draft on
the first commit, ≈3 attempts then hand back); `ci-merge` holds the merge queue.
This skill is how the orchestrator runs a batch on top of them.

## What runs together: file collision, not count

Merging is serialized, so every branch behind another pays a rebase per merge
ahead of it. Over separate files that is seconds of `git`; over shared code it is
a session of conflict resolution. **Before starting a branch, ask whether it edits
the same types as one in flight.** Spread across modules (physics, UI,
navigation, audio, CI) at once; within one module, run one branch at a time.

The files every feature appends to, where two branches at once is the case to
avoid:

- `src/components/entity/mod.rs` (the document struct, its imports and
  `Entity::new`);
- `src/scene/authoring/components.rs` (`ComponentKind`) and
  `src/scene/authoring/dependency.rs` (`set_default`, `has_kind`, `clear_one`:
  two component branches together push these past clippy's 50-line cap; split
  by group, never trim);
- `src/api/mod.rs`, `src/scripting/callbacks.rs`,
  `src/editor/inspector/components/add/`;
- `src/app/registry.rs`'s `build()`, where order *is* execution order, so a merge
  that reorders it changes behaviour silently;
- `docs/api/index.md`, `docs/api/Scene.md`'s component list,
  `docs/api-faithfulness.md`,
  `docs/linting.md` (a rule's table row and section; both are kept sorted under
  markers, so two rules land on different lines);
- `Cargo.toml` / `Cargo.lock`: one owner per wave. Name the owner in every
  sibling's brief ("stay out of `Cargo.toml`"); the watch holds a second one
  anyway (#969).

Where order means nothing, a list is kept sorted under a `// sorted: <why>` marker
(the `sorted` rule, #911), so parallel entries land on different lines; reach for
the marker when a new append-only list appears.

**Split by responsibility, not by parallelism.** Sub-issues that all touch the
same type are one branch: a first-class component's four axes are one change the
completeness gate won't accept in pieces. Sub-issues are for work separable *in
the code*, not merely listable.

## Where a branch runs

**Cloud by default** (#924). Lavapipe runs the GPU tests in the container (#826),
so only the machine-bound work stays local: a real GPU (Metal, performance, driver
races), the display, the speakers, the operator's files or keys, or a
measurement. Once the local-only proof is in the PR, the rest of the branch can go
to the cloud. Local builds: check `free -g` and `df -h` first, and give each heavy
build half the cores (`CARGO_BUILD_JOBS`) when another may overlap it. Never run
`cargo mutants` locally during a batch. A local coder gets the same `Brief` and
`cloud-brief.md`, and puts its issue number in every scratch filename: siblings
share the scratchpad.

**Launching a cloud coder.** A one-off routine via `RemoteTrigger` (the `schedule`
skill has the body shape):

- **Environment: `anthropic_cloud`**, never a `bridge` one (`archlinux:…` is this
  machine). That choice is what makes it remote. Not the `Agent` tool's
  `isolation: "remote"`, which fell back to a local worktree here (#504).
- `run_once_at` computed, never typed: `date -u -d '+2 min' +%Y-%m-%dT%H:%M:00Z`
  (`-v+2M` on macOS). The API refuses a time already past.
- **The model** from the brief's `Model:` line (*Which model*), never the
  example body's default.
- **`allowed_tools`** includes `ToolSearch`; the GitHub MCP tools load through it
  without being listed.

**The brief is a comment on the issue** (#910), first line `Brief`. The prompt is
echoed back about three times by `RemoteTrigger`, so it stays a fixed pointer, and
the comment is durable for a relaunch or a later reader. Standing rules for every
coder live in [`cloud-brief.md`](cloud-brief.md); change them there, once.

```text
Brief

Branch: `N-short-slug`.
Builds on: <what merged that it must build on, as "PR #N, Seam">.
Siblings in flight: <branch/issue → the files it edits; stay out of them>.
Decisions: <anything already settled, or "none">.
Model: <Opus or Sonnet, by *Which model*>.
```

Add to it when it applies: a shared file a merge left near the 300-line cap, the
`Cargo.lock` owner.

**A `Decisions` line states facts, so check each one first.** In the 2026-10-22
batch, four briefs carried a premise nobody had checked, and each cost a review
round:
- "Play never imports" assumed Unity's import-on-add, which rusty lacks;
- "the player ships its caches" assumed an export step that didn't exist;
- "leave transparent runs out" forgot the renderer still samples their lightmap;
- "a path can't hold `::`" is false on Linux and macOS.

So, before writing a brief:
- before citing Unity, check that rusty has the machinery that makes Unity's way safe;
- when a decision removes data or a capability, grep its consumers;
- verify any claim about a platform or a tool in one command.
- write "Builds on PR #N merged" only after `gh pr view N --json state` says
  `MERGED`: a coder that finds the groundwork missing stands down (2026-10-24, #1387);
- put no closing keyword in a routine prompt for an issue already closed: the
  queue hands back a PR that closes a closed issue (#1422);
- file an issue first and cite the number it got, never the one you expect: two
  briefs named the wrong issue in one batch.

The routine prompt:

```text
Read `.claude/skills/issue-batch/cloud-brief.md` and follow it.
Issue(s): #N. Your brief is the newest comment on it that starts with `Brief`.
```

A re-brief (a rebase, a fix, a new sibling) is a new `Brief` comment on the
issue, with "force-push authorised" when it works a pushed branch, and the PR
turned back to draft. **Then `RemoteTrigger run` the coder's own routine right
away.** Its prompt already points at the newest `Brief`. Don't wait for the old
session: it is subscribed to the PR, not the issue. In the 2026-10-22 batch,
sessions woke on the draft flip, read only the PR, found no instruction and
stood down, by design (`cloud-brief.md`: once ready, the branch is the
queue's). That looked like "half never wake", and it is also why they never
race the fresh session. So never put the re-brief on the PR: every session
subscribed to it would act on it, and a PR re-briefed twice has two. **Take the
`queue` label off when you re-brief**, and put it back only after reading the
fix. A re-briefed PR that keeps the label is merged as soon as its coder readies
it, unread (#1255, 2026-10-23).

**The pull request is the report.** A cloud coder cannot message back; it opens a
draft, pushes often, readies when its gates are green and ends. Watch the PR, not
the agent. A finished routine whose PR is still a draft with no reason in the
description stalled; read its run log (`get_run_log`) and brief a fresh one to
finish the pushed branch.

## The orchestrator's loop

The orchestrator (this session, local) writes no feature code. It picks work,
briefs coders, reads diffs and labels. GitHub is the batch's state: open PRs,
their comments and `RemoteTrigger list` resume a batch from nothing, so nothing
lives only in the scratchpad.

**Start the queue once a draft exists, under its supervisor** (#908):
`make queue-daemon ARGS="--no-check --since <batch start>"`, from a worktree under
`.claude/worktrees/`. It detaches (a background command dies at two hours), logs
to `.git/merge-queue/queue.log`, moves its worktree to `origin/main` before each
watch, and restarts the watch by itself on every planned exit: the deadline and a
hand-back at once, "nothing left" after ten minutes, `relaunch:` (status 5, the
queue's own code changed, #967) at once, the machine or GitHub failing with a
backoff, five times in a row at most. Put a `Monitor` on `tail -F
.git/merge-queue/queue.log | grep --line-buffered -E 'HANDED BACK|supervisor:.*stopped'` and on new issue comments matching `Standing down` (a coder that stops says so on its issue, not on a PR),
never on the watch itself (a `Monitor` dies at 30 minutes and takes its command
with it). The log's `watch:` lines are the whole view of the coders: no second
poller. **Stop it with `make queue-stop`**, between pull requests; never by
pattern: `pkill -f` kills scorsese's watch too. Never `pkill -f` a pattern your own
command line contains either: it kills the shell running it. Run bare (`make queue ARGS="--watch
--no-check --for 70"` in the background), the watch needs relaunching by hand on
each exit: its last line is `watch: resume: <command>`.
`ci-merge` has everything else about the queue.

Per pull request:

1. **Read the description and the diff, then add the `queue` label** (#925).
   Check its decisions against the issue. Engine mechanism, a stored format,
   determinism, CI or the merge path, or a refactor gets the
   `independent-review` skill first. What the read turns up goes back to the
   coder unlabelled; removing the label pulls a PR out of line. File its
   "follow-up" notes as issues now. If it dispatched a scoped mutation run, read
   the report when it lands (`ci-merge`, *Triaging a survivor*).
2. **A hand-back** skips that PR and the watch carries on. A one-line fix is
   yours: under `--no-check` nothing compiles it before CI, so grep every use of
   what either side moved and check the 300-line and 50-line caps first; the
   `pre-push` hook (`make resolved`) catches a resolution rustfmt rejects. A
   resolution that needs a compile to trust (a split, a dispatcher, system
   order, `Cargo.lock`) goes to a fresh routine.
3. **After a merge:** `make reclaim` (removes merged worktrees, refuses unpushed
   work), then re-read the board.

## Starting, and re-reading the board

At the start: `make blockers ARGS=--fix` (records prose-only blockers) and
`make board` (stray branches, stale assignments, unlabelled issues, bugs with
no `Kind:` line); close out
what it names. Unassign an issue that turns out never started.

**After every merge, re-read the whole board**, not your list of it: coders and
the operator file mid-batch, and what the merge unblocked is startable now. Count
what is left from `gh issue list`, never from memory. A new label-priority leader
is started next. **A root issue is picked over its instances**: a bug whose
`Kind:` names an open root issue is not started on its own; the root's branch
closes it. Dependabot PRs merge through the same queue at the lowest
priority (`ci-merge` has the Actions-bump hold).

## Filing a bug

From 2026-10-19 to 21 the same kind of bug got about twelve separate patches,
each one's review finding the next, before anyone filed the root fix (#1170).
So whoever files a bug (orchestrator, coder or reviewer) first:

0. **Checks it is worth an issue** (CLAUDE.md, *Issues are for what matters*): it
   already caused a problem, or leaving it costs more than fixing it, or it is a
   crack in the structure or the tooling. In the 2026-10-24 batch, reviews filed
   about as many issues as the batch closed, mostly for edges nobody had hit; the
   operator closed them unbuilt.
1. **Looks for earlier cases** of its kind, open and closed, by symbol, module
   and wording. `gh search issues` answers 403 in a cloud session; there, page
   `gh api 'repos/MatthewLacerda2/rusty/issues?state=all&per_page=100&page=N'` and
   grep titles and bodies (write the repository out: gh may fail to resolve
   `{owner}/{repo}` through a cloud session's proxy remote).
2. **Classes it: a case or a symptom**, even the first time. A case is a discrete
   slip; a symptom is an underlying problem that will produce more.
3. **For a symptom, files the fix that scales**: a type, a lint rule, a shared or
   property test, one API. The instance becomes its sub-issue
   (`gh api -X POST repos/MatthewLacerda2/rusty/issues/<root>/sub_issues -F
   sub_issue_id=<instance id>`: `-F`, an integer, and the issue's `id`, not its
   number),
   never its own PR. An instance of an **open** root issue is folded into it: a
   comment on the root, or a sub-issue of it, never a sibling issue.
4. **The second case of a kind with no root issue triggers one.**

Every bug issue carries one line, `Kind:`, exactly one of:

- `Kind: #N`: an instance of root issue N, which is also its sub-issue parent;
- `Kind: case — searched <terms>`: a discrete slip, with the search that found no
  kin;
- `Kind: root`: the root issue itself.

`make board` lists an open `bug` issue (not `planning`) without the line, with
more than one, or with a form not listed above; a `case` that names no search;
a `Kind: #N` whose sub-issue parent is not N; and a `case` or `root` that has a
sub-issue parent. It reads the line past list, quote, bold and backtick markup,
and ignores one inside a fenced code block.

## Which model

The model follows the shape of the task, not the area of the code (operator,
2026-10-19). **Sonnet when the issue already says exactly what to do and a gate
proves it was done; Opus when the work has to decide something.**

- **Sonnet:** a mechanical rebase (two rows in one table, a moved file); a doc
  fix the docrefs check proves; splitting an oversized file or regrouping a
  directory; a lint rule whose issue names the case to flag and the case to pass;
  the editor's egui panels, cards and inspectors.
- **Opus:** everything else, in particular tests; read-only agents; the renderer
  and shaders, determinism, physics and navigation; the merge-queue scripts; the
  five kinds, the API surface or a stored format; a red `main`; a conflict in
  code; writing issues and reviewing diffs.

When the line is not crisp, err upwards. **One issue, one agent, one model**: a
second model starts cold and pays again for everything the first read. Work split
into independent pieces takes each piece's own model.

## Hand-backs and human checks

**Save, ask, move on.** On ≈3 attempts or a decision that is genuinely the
user's (the five kinds, a departure from the issue): push, mark the PR draft,
write the reason in the description and an issue comment, and take the next
startable work. The branch stops, never the batch. A call the issue did not
foresee is not the user's when Unity has an answer.

**A human check is never a merge hold.** A real window, speakers, taste: the
check goes under the PR's *For a human* heading and the PR merges when green;
`make checks` gathers them for the report. If it turns out broken, that is a bug
to file. The same for a judgement the issue left open: take the default, write it
down, keep going.

## Learning from the batch

A batch finds the traps in the tooling faster than anything else, and a lesson
that only reaches the chat is lost. Each one ends **fixed** (a PR removes the
cause), **enforced** (a gate), **documented** (here, `ci-merge` or CLAUDE.md, for
what the repo can't change) or **tracked** (an issue with the evidence). A lesson
that blocks the running batch, or bites twice, is infrastructure: fix it now.
Prefer fixing and enforcing to documenting; a line added here is a line every
batch pays to read. A lesson goes in as the rule, not its story, and before
adding one, take out what tooling or a better model has made unnecessary.

At the end, before reporting: list every trap, surprise and hand-back (your
notes, each PR's *Decisions*, the issues coders filed, their run logs), map each
to one of the four, and add a *Context update* comment to the unstarted issues
whose ground moved (never changing a `planning` issue's scope).

## Finishing and reporting

The batch is done when the main checkout runs what merged, once nothing is
compiling:

1. `make reclaim` for what is left. Delete a local branch only when
   `gh pr view N --json state` says `MERGED` (`git branch --merged` cannot see a
   squash) or it has no commits beyond `main`; other unmerged work is the user's
   to delete.
2. `git pull --ff-only` on `main`.
3. `cargo build --features dev --bin session-mcp --bin rusty` in the main
   checkout, so local MCP clients drive the new engine (`docs/mcp.md`) and the app
   search's **Rusty** (`make launcher`) opens it; a connected client keeps the old
   process until it reconnects.

**The report opens with `make checks SINCE=<batch start>`**: the unticked human
checks and the `planning`+`human` issues. Then the result, what surprised you
(not only what went wrong), and the lessons mapped to their changes. Two things
are raised even mid-batch, while they can still be overruled: a change to the
user's files outside the repo, and a decision reversed from what the issue said.
