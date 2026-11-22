---
name: issue-batch
description: Run a set of issues from board to merged — how many branches at once, which ones can safely run together, worktrees, re-reading the board, and the cleanup and local rebuild that finish a batch. Use when starting work on one or more issues, when deciding what to start next, or when told to "do the issues".
---

# Working a batch of issues

The user writes many issues and asks for them in batches, leaving how many and in
what order to you. CLAUDE.md holds the rules a batch runs on (the limits per kind
of agent, one worktree per branch, stage labels, branch names, draft on the first
commit, ≈3 attempts then hand back); `cloud-brief.md` holds what every coder
follows; `ci-merge` holds the merge queue. This skill is how the orchestrator runs
a batch on top of them.

When in doubt mid-batch, write a comment on the issue and resume the work where
possible, rather than asking in the chat and spending the night waiting for an
answer. A conversation that plans issues doesn't wait: ask its questions right away.

## The board's signals

- **An assigned issue is in progress; an unassigned one is fair game.** Assign the
  user the moment work on an issue begins.
- **`planning` and `human` are the only stops**, and no amount of the issue looking
  ready overrides them. An idea still being shaped is what `planning` is for, so
  asking again whether an unlabelled issue is ready when work begins adds nothing.
- **How an issue reads is the writer's call**; the repo's rules about issues are
  CLAUDE.md's *Issues, labels & priority*. A bug or doc fix is filed without
  `planning` and merged like any other ready PR.

## How many coders, and why

The limits are CLAUDE.md's; the reasons are here, so they are kept, not re-argued.

- **Cloud coders ≤5**, paced by how many diffs one orchestrator can read: review,
  not coder count, is the bottleneck. A sixth `make coder` waits its turn in a line.
- **Local coders ≤2**: the queue lands one tested tree per CI run, so a third
  finishes nothing sooner.
- **Read-only agents** (research, review, planning) **without limit**: they build
  nothing.
- **Who runs on what.** The orchestrator runs on whatever model the operator
  picked; everything that writes or judges code runs on a model its definition
  pins, so the choice never follows the orchestrator's. `make coder` pins the
  cloud coder's (`dispatch/routine.py`'s `MODEL`), unless the issue body has a
  `Model:` line of its own (a whole line `Model: <word>` outside a fenced block,
  the word `sonnet` or `opus`, `routine.MODELS`; any other word is refused, and `ARGS=--dry-run` says which it would use). That line is the
  operator's decision written on the issue: an orchestrator never adds one on
  its own. The `Agent` tool's types are
  in `.claude/agents/`: `coder` (a local coder) and `reviewer` on the latest
  Opus, `merge-fixer` (a hand-back's mechanical fix) on Sonnet; `agent-docs`
  holds the pins (`agent_docs::agent::PINS`) and a concrete `model:` on every
  definition. Dispatch them by `subagent_type` and never pass `model`, which
  would override the pin. The orchestrator's own quick read-only searches run on
  what it picks.
- Merging stays one at a time, whatever the count.
- **One worktree per branch**, under `.claude/worktrees/`, off the latest `main`,
  removed the moment it merges. **Never a shared `CARGO_TARGET_DIR`**: worktrees
  overwrite each other's artifacts, and a green gate stops meaning this branch
  compiled.

## What runs together: file collision, not count

Merging is serialized, one tested tree at a time, but the queue tests up to four
pull requests together on one tree (a merge train, `ci-merge`): a branch that
merges cleanly onto `main` and the others rides without being touched. One that
conflicts with a branch merged ahead of it is not: it pays an update (a merge of
`main` into it) and its own CI round alone. Over separate files that never happens;
over shared code it is a session of conflict resolution. **Before starting a branch, ask whether it edits
the same types as one in flight.** Spread across modules (physics, UI,
navigation, audio, CI) at once; within one module, run one branch at a time.

The files every feature appends to, where two branches at once is the case to
avoid:

- `crates/components/src/registry.rs`'s `first_class!` rows: one sorted line per
  component, from which `Entity`, the `World` accessors, `ComponentKind`, the
  dependency dispatch, `KINDS`, the snapshot and the Add Component menu are
  generated, so two component branches touch different lines; what still
  collides is each one's hand-written rows (`inspect!`, `snapshot!`,
  `AddEntry`) when they share an area file;
- `crates/edit/src/serialize.rs`'s `apply` and `crates/scene/src/undo/level.rs`'s list: every
  saved `Scene` field adds a line to both, so two such branches pass clippy's
  50-line cap in `apply`; the level list's check test asks the
  second to land for its field;
- `crates/script/src/api/mod.rs`, `crates/script/src/scripting/callbacks.rs`;
- `crates/app/src/registry.rs`'s `build()`, where order *is* execution order, so a merge
  that reorders it changes behaviour silently;
- `docs/api/index.md`, `docs/api/Scene.md`'s component list,
  `docs/linting.md` (a rule's table row and section; both are kept sorted under
  markers, so two rules land on different lines);
- `Cargo.toml` / `Cargo.lock`: one owner per wave. Name the owner in every
  sibling's brief ("stay out of `Cargo.toml`"); the watch holds a second one
  anyway.

Where order means nothing, a list is kept sorted under a `// sorted: <why>` marker
(the `sorted` rule), so parallel entries land on different lines; reach for
the marker when a new append-only list appears.

**Many instances of one kind: a registry first.** Pairwise, an issue
that adds one more effect, component, lint rule or namespace is "one more row";
six of them are a serial chain of conflicts. So before briefing, group the
batch's issues by what they add. When three or more add an instance of one kind,
and each would edit a shared struct, shader body, match arm or list rather than
add its own file plus one sorted row, the batch's first issue makes that kind a
registry: filed before any of them is briefed, and `Blocked by` on each. The
model is `crates/render/src/CLAUDE.md`, *Adding a post effect*, with the `// sorted:`
marker. No check holds this: it is read from issues before any code exists.

**Split by responsibility, not by parallelism.** Sub-issues that all touch the
same type are one branch: a first-class component's four axes are one change the
completeness gate won't accept in pieces. Sub-issues are for work separable *in
the code*, not merely listable.

**One brief over many coders is piloted.** When the same brief goes to several
coders over separate paths (a sweep, a migration, a bulk rewrite), the first pull
request runs alone, and an independent read-only agent audits its **substance**
before the rest launch: what the brief left to the coder's judgement, read before
and after. What it finds goes into the remaining briefs. A proof of form (only
comments changed, a count under a cap, a green build) says nothing about the
choices made inside it, so the issue says what its proofs cover and what the
pilot's audit covers. The model follows the judgement in the job, not its volume.

## Where a branch runs

**Cloud by default** (CLAUDE.md, *Coding parallelises*). rusty is developed by one
person, locally or in a cloud session, not by many contributors on many cold
machines, so don't propose tooling built for that. Each worktree compiles into its
own `target/`, and one cold build fills every core and several GB of memory:
parallelise the work and stagger the compiles. Everything a container can prove
(lavapipe runs the GPU tests) goes to the cloud, which lifts the local build limit,
never the one-at-a-time merge. A branch goes local only when it needs the machine:
a real GPU (Metal, performance, driver races), the display, the speakers, the
operator's files or keys, or a measurement (a performance PR's measured row is
`make bench-ab PR=<n> PROJECT=<dir>`, with `STRESS=1` for a sim PR's stress-yard
rows, run on the desktop by the orchestrator, never a script outside the repository). With the coding in the cloud, the merge
queue is the only local work (*The orchestrator's loop*). Once the local-only proof
is in the PR, the rest of the branch can go to the cloud. **A bug seen only in the
real case** (the operator's scene, this machine's driver, a load measurement) gets
that check on the branch's first implementation, before its independent review:
a primitive reproduction can pass while the real case still fails, and each review
round spent before the check may be polishing the wrong fix. Local builds: check
`free -g` and `df -h` first, and give each heavy build half the cores
(`CARGO_BUILD_JOBS`) when another may overlap it. Never run `cargo mutants`
locally during a batch. A local coder is the `Agent` tool's `coder` type, its
prompt the issue number and its `Brief` comment's URL: it follows the same
`cloud-brief.md`.

**Launching a cloud coder: post the `Brief`, then `make coder ISSUE=N`.**
It reads the newest `Brief` comment on #N and creates the one-off routine that
starts the cloud session a minute or two later; it prints the routine's link,
where its run links the session. Running it again for the same issue is the
re-brief launch: a new routine whose prompt names the new `Brief`.
`ARGS=--dry-run` prints the routine and creates nothing. Never assemble a
`RemoteTrigger` body by hand; `.github/scripts/dispatch/routine.py` has what the
script keeps, and why:

- **Environment: `anthropic_cloud`**, never a `bridge` one (`archlinux:…` is this
  machine). That choice is what makes it remote; the `Agent` tool's
  `isolation: "remote"` falls back to a local worktree here. The API cannot list
  environments, so the script takes the one the newest `rusty #…` routine ran in;
  `RUSTY_CLOUD_ENV` names it on an account with none yet.
- **What it refuses:** a closed issue, a `planning` or `human` one, and a brief by
  someone who cannot push. A refused launch is never queued.
- **A launch at CLAUDE.md's limit waits its turn**: it joins the waiting line
  (exit 0, "waiting for a slot, N ahead", naming every coder it counted), and a
  second ask for the same issue keeps its place. A round (a `Round:` line in the
  newest `Brief`) goes ahead of first launches, then the order asked. How a fired
  routine is placed (a ready PR holds its slot until its review record exists, so
  its re-brief has one) is `dispatch/running.py`'s docstring; a re-brief is never
  counted against its own issue. While the line holds others, a free slot goes
  to them first: a hand launch not within the free slots in line order waits
  too and names who is ahead. `ARGS=--no-wait` refuses instead of queueing,
  `ARGS=--over-limit` launches anyway (a stalled draft counts until dealt with).
- **`make coders` reads who runs and who waits, and launches from the line**
  while slots are free: every counted coder and why it counts, the uncounted ones
  worth naming, then the line in launch order. Nothing launches without that
  call, so while the line holds anyone run it on every event `make queue-events`
  passes and at least once a minute. Before it launches an entry it re-reads the
  issue: one closed, given a stage label or whose brief is refused is dropped and
  said; one with a newer `Brief` than it was queued on launches on that one;
  one whose issue already has a routine made after its ask and its brief was
  launched (a pass cut short) and is dropped, never launched twice, slot free or
  not, and `make coder` for it drops it the same way. A command
  that finds another session holding the line waits a few minutes, then exits 2
  naming the holder. `ARGS=--dry-run` launches, writes and locks nothing.
- `run_once_at` computed, `allowed_tools` with `ToolSearch` (the GitHub MCP tools
  load through it without being listed), and the prompt below.
- **The prompt names the brief's URL**, the exact comment the script checked,
  never "the newest `Brief`": the repository is public and the routine fires
  minutes later, so a comment posted in between would otherwise be what a coder
  with push access follows.
- It calls `RemoteTrigger` through `claude mcp serve`, so it needs the `claude`
  CLI signed in to claude.ai, as this machine is; the token never leaves
  `claude`. `claude --cloud` cannot do it: it starts a session only from an
  interactive terminal, and an orchestrator's shell has none.

**The brief is a comment on the issue**, first line `Brief`. The prompt is
echoed back about three times by `RemoteTrigger`, so it stays a pointer, and
the comment is durable for a relaunch or a later reader. Standing rules for every
coder live in [`cloud-brief.md`](cloud-brief.md); change them there, once.

```text
Brief

Branch: `N-short-slug`.
Builds on: <what merged that it must build on, as "PR #N, Seam">.
Siblings in flight: <branch/issue → the files it edits; stay out of them>.
Decisions: <anything already settled, or "none">.
```

Add to it when it applies: a shared file a merge left near the 300-line cap, the
`Cargo.lock` owner.

**A `Decisions` line states facts, so check each one first.** An unchecked
premise costs a review round. Before writing a brief:
- before citing Unity, check that rusty has the machinery that makes Unity's way
  safe (an import step, an export step), and read Unity's source for the claim (a
  URP shader or C# file);
- a decision that sets an edge behaviour Unity already has (what a removal, an
  out-of-range lookup or a re-import does) names Unity's behaviour after checking
  it;
- a brief that offers alternatives names the invariant each must keep;
- when a decision removes data or a capability, grep its consumers (a renderer
  path that still samples it, say);
- a brief for work that records, replays or digests the sim names every input
  the sim reads, the frame's `dt` included;
- a brief for work held across ticks (a search, a query, a warm-up, a cache) says
  what a rebake, a scene load, a Stop and a replaced request do to it: carry it
  when the change misses it and restart only when it reaches it, since a restart
  on every change never finishes behind something that changes every few ticks;
  and when it makes state process-wide, it says which tests reset that state;
- verify any claim about a platform or a tool in one command (what a path may
  hold, say);
- a decision resting on a count or a limit (runners, memory, minutes) writes out
  its arithmetic, so a reader can check it;
- a decision that adds a second way to set one value (a flag beside the value it
  marks, a table key beside a changed field) says which wins when a caller sends
  both, as a Get table carried back to its Set does;
- write "Builds on PR #N merged" only after `gh pr view N --json state` says
  `MERGED`: a coder that finds the groundwork missing stands down;
- put no closing keyword in a routine prompt for an issue already closed: the
  queue hands back a PR that closes a closed issue;
- file an issue first and cite the number it got, never the one you expect.

The routine prompt, which `make coder` writes (`dispatch/routine.py`'s `PROMPT`):

```text
Read `.claude/skills/issue-batch/cloud-brief.md` and follow it.
Issue(s): #N. Your brief is the comment at <the brief's URL>; read it there, not the newest `Brief`.
```

A re-brief (a conflict, a fix, a new sibling) is a new `Brief` comment on the
issue with a `Round:` line under `Brief` naming why it happened, `Round: review |
ci | conflict | other` (a review sent it back, CI went red, it conflicts, anything
else; `make rebriefs` counts them), and that says **add commits on top, merge
`origin/main` in only if the PR conflicts, never rebase**, so no brief needs to
authorise a force-push, and the PR turned back to draft. A round's session
readies on `make round-gates` (the gates its diff can break), a first brief's on
`make gates` (`cloud-brief.md`). **Then `make coder
ISSUE=N` again right away**: a new routine whose prompt names the new `Brief`.
Don't wait for the old session: a readied session ends for good (`cloud-brief.md`),
so it never races the fresh one. So never put the re-brief, or a pointer to it,
on the PR: every session subscribed to it would act on it. **Take the `queue`
label off when you re-brief**, and before you push to a queued PR yourself, and
put it back only after reading the fix: a re-briefed PR that keeps the label is
merged as soon as its coder readies it, unread. With the label off the push is
safe: a head that moves during the watch's CI wait goes back in line and waits
there for the label, then its own CI.

**The pull request is the report.** A cloud coder cannot message back; it opens a
draft, pushes often, readies when its gates are green and ends. Watch the PR, not
the agent. A finished routine whose PR is still a draft with no reason in the
description stalled; read its run log (`get_run_log`) and brief a fresh one to
finish the pushed branch.

## The orchestrator's loop

The orchestrator (this session, local) writes no code: not a feature, not a fix
a review asked for, not a conflict's resolution. It picks work, writes issues
and briefs, reads diffs and labels; the one branch it writes itself is a small
documentation or doctrine PR (a skill, a `CLAUDE.md`, an agent definition).
GitHub is the batch's state: open PRs,
their comments and `RemoteTrigger list` resume a batch from nothing, so nothing
lives only in the scratchpad.

**Start the queue once a draft exists, under its supervisor**:
`make queue-daemon ARGS="--no-check --since <batch start>"`, from a worktree under
`.claude/worktrees/` (`merge-queue`); from the main working tree it refuses and
prints the command to run instead. `--no-check` keeps the engine build off this
machine: no updated head or train tree is compiled before CI judges it, only
`tools/lint`'s build-free passes run (`ci-merge`, step 2), so merging costs `git`,
GitHub calls and a small lint build. Prefer it whenever the machine has other
work; the price is that an update broken by a merge ahead (rare) is caught by CI
a round later, not before the push. It detaches (a background command dies at two
hours), logs to `.git/merge-queue/queue.log`, moves its worktree to `origin/main`
before each watch, and restarts the watch by itself on every planned exit (the
deadline, "nothing left", a hand-back, `relaunch:`, the machine or GitHub failing,
with a backoff, five times in a row at most). It locks its worktree so `make
reclaim` leaves it alone; a lock left by a dead supervisor shows in `reclaim` as
stale, with its PID. `make queue-daemon` prints "supervisor started" only once
the child was seen holding its lock; otherwise it prints the log's last lines and
exits non-zero, or says *unconfirmed*. Its exit statuses are in `ci-merge`.

- **Watch it with `make queue-events`**, the one `Monitor` command, never a
  hand-typed `tail | grep`: it replays the log from 30 lines back (a re-arm from
  the end drops what the log said while no `Monitor` ran), passes hand-backs,
  merges, the supervisor's and the watch's stops, a held Dependabot PR, every PR
  turning ready, going back to draft or closing (one that turned ready or closed
  between two watches is said by the next, once; with no kept board, each ready,
  unlabelled one is said `is ready and unqueued`), and every comment a coder
  posts on its open briefed issue (`STANDING DOWN #N` for a stand-down, `COMMENT
  #N` for any other wording; `Brief`s and comments opening `Context update` are
  skipped as yours, so head a note of your own `Context update`, never `Brief`,
  which the dispatch tooling would read as a re-brief), and **exits 1 when the supervisor is not
  running**, so a dead queue is an event. Put the `Monitor` on it, never on the
  watch itself (a `Monitor` dies at 30 minutes and takes its command with it);
  re-arm it the same way.
- **Ask `make queue-status` whether the queue runs** (at a batch start, a resume,
  after a reboot): it checks the supervisor's lock PID alive (exit 0 running, 1
  not), prints the log's last line and its age, and a `STRANDED #N` line for each
  ready, labelled pull request passed over for over an hour on a green head.
  Never `pgrep -f`: it matches the checking shell's own command line.
- **Stop it with `make queue-stop`**, between pull requests; never by pattern:
  `pkill -f` kills scorsese's watch too, and a pattern your own command line
  contains kills the shell running it.
- Run bare (`make queue ARGS="--watch --no-check --for 70"` in the background),
  the watch needs relaunching by hand on each exit: its last line is `watch:
  resume: <command>`.

`ci-merge` has everything else about the queue.

Per pull request:

1. **Read the description and the diff, then add the `queue` label.**
   Check its decisions against the issue. Engine mechanism, a stored format,
   determinism, CI or the merge path, or a refactor gets the
   `independent-review` skill first; any other code PR gets a `No review: <why>`
   comment (`independent-review`, *Answering the findings*, step 2, says which
   PRs need neither); the queue hands back a PR without its record. What the read
   turns up goes back to the coder unlabelled; removing the label pulls a PR out
   of line. File its "follow-up" notes as issues now. If it asked for a scoped
   mutation run (a `Mutation:` line, which the queue dispatches after the merge),
   read the report when it lands on the merged PR (`ci-merge`, *Triaging a
   survivor*): each survivor is a question, so a follow-up names only the ones
   that show behaviour nothing asserts, and leaves the rest; no brief asks for
   zero survivors.
2. **A hand-back** skips that PR and the watch carries on. A mechanical fix (a
   conflict where both sides are kept, a one-line red) goes to the `Agent`
   tool's `merge-fixer`, its prompt the PR, its branch and the hand-back line;
   take `queue` off first and put it back after reading what it pushed. A
   resolution that needs a compile to trust (a split, a dispatcher, system
   order, `Cargo.lock`), or one `merge-fixer` stopped on, goes to a fresh
   routine.
3. **After a merge:** `make reclaim` (removes merged worktrees and deletes
   finished local branches; keeps unpushed work, a lock, and a worktree in use:
   a process standing in it or running a program from it, or a write in the
   last hour), then re-read the board. A helper's worktree survives it while
   something runs there or was written within that hour; a longer pause (a build
   kept to read results later, a session that may die) needs
   `git worktree lock --reason …`.

## Starting, and re-reading the board

At the start: `make blockers ARGS=--fix` (records prose-only blockers, and reports
a recorded open blocker no body states any more) and
`make board` (stray branches, stale assignments, unlabelled issues, bugs with
no `Kind:` line); close out
what it names. Unassign an issue that turns out never started. **A dependency is
lifted by taking its line out of the body (or striking it through) and running
`make blockers ARGS='--lift <issue> <blocker>'`**: the merge queue reads the
recorded relationship, not the prose, and `--lift` is the only thing that removes
one. `--fix` never deletes; `make blockers` reports each recorded open blocker no
body states with its `--lift` command, and a "Blocked by" it could not read until
it is rewritten as `#N` items.

**After every merge, re-read the whole board**, not your list of it: coders and
the operator file mid-batch, and what the merge unblocked is startable now. Count
what is left from `gh issue list`, never from memory. A new label-priority leader
is started next. **A root issue is picked over its instances**: a bug whose
`Kind:` names an open root issue is not started on its own; the root's branch
closes it. Dependabot PRs merge through the same queue at the lowest
priority (`ci-merge` has the Actions-bump hold).

## Filing a bug

The same kind of bug patched one instance at a time, each review finding the
next, is what this prevents. So whoever files a bug (orchestrator, coder or
reviewer) first:

0. **Checks it is worth an issue** (CLAUDE.md, *Issues are for what matters*): it
   already caused a problem, or leaving it costs more than fixing it, or it is a
   crack in the structure or the tooling. An edge nobody hit is not filed.
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
sub-issue parent. It reads the line past list, quote, bold and
backtick markup, and ignores one inside a fenced code block.

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

1. `make reclaim` for what is left. It deletes the finished local branches too,
   reviewers' `prNNNN` included, and names each branch it keeps with
   its reason; a kept branch with unmerged work is the user's to delete.
2. `git pull --ff-only` on `main`.
3. `cargo build --features dev --bin session-mcp --bin rusty` in the main
   checkout, so local MCP clients drive the new engine (`docs/mcp.md`) and the app
   search's **Rusty** (`make launcher`) opens it; a connected client keeps the old
   process until it reconnects.

**The report opens with `make checks SINCE=<batch start>`**: the unticked human
checks and the `planning`+`human` issues. Then `make rebriefs ARGS="--since <batch
start>"`: briefs per issue, issues that took one brief, and re-briefs by `Round:`.
A cause that dominates is what the next infrastructure issue attacks (CLAUDE.md,
*Once is a case; twice is an issue*), and `unstated` means a re-brief left its
line out. `make coder-cost` then says what those sessions cost in
wall-clock (sandbox, reading to the first edit, gates, the rest; first briefs
against rounds) from the run logs of the coder routines still listed, adding to
`.git/coder-cost.json` each time; it has no token counts, which the run log lacks. Then the result, what surprised you (not only what went wrong), and the
lessons mapped to their changes. Two things are raised even mid-batch, while they
can still be overruled: a change to the user's files outside the repo, and a
decision reversed from what the issue said.
