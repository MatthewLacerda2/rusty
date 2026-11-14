---
name: ci-merge
description: Take a finished branch through to merged — the gates, CI, verifying a run happened on the head commit, updating a branch from `main` and its conflicts, and triaging the mutation and coverage signals. Use when a branch is ready, when a pull request has gone red, when a mutation report needs triage, or when merging anything into `main`.
---

# Getting a branch merged

The steps and the traps. `CLAUDE.md` says why merging is serialized; this is how.

## Draft or ready

- **Open the PR on the branch's first commit**, unasked, so no branch is a stray
  with no documented purpose. Branch names: `{issue_number}-short-slug` for a PR
  that closes an issue (`163-fix-coverage-scope`), a readable slug of the subject
  for an issue-less one (`document-ai-parity`), lowercase-hyphenated.
- **Draft** while the work is in progress, or whenever it is blocked or needs the
  user: a decision you can't make, another PR or issue that must land first, human
  or hardware help. Say why in the description. **Ready for review** once the work
  is done and nothing more is needed from the user; flip between the two as the
  work moves. CI's gating jobs skip drafts, so readying is when anything is checked.
- **The description** says what changed, why it belongs in rusty (the project-level
  value its label classifies, not how you got there; process only when the diff
  needs it) and the effect, and carries the PR↔issue link. Check the number in a
  closing keyword: a typo silently closes the wrong issue.
- **Why one tree at a time:** two PRs can each be green alone and break `main`
  together (a rename, a changed signature, a moved module) with no textual
  conflict. A branch is updated by merging `main` in: nothing is rewritten, no push
  is forced, a conflict is resolved once. PRs are squash-merged, so the branch's
  own history never reaches `main`.

## Before marking a pull request ready

    make gates

All green, fastest-failing first. The `Makefile`'s `GATES` list mirrors
`.github/workflows/{ci,lint}.yml` (`make scripts` fails when they drift), and
`make help` names each gate.

- **Both feature sets, always.** Half the crate is behind `dev` (the harness, the
  session, the MCP bridge, `Debug.*`); a default-features run compiles none of it.
  The third clippy pass, `--no-default-features`, is the player build, the only
  compile without egui.
- **Some hard gates hide inside the tests.** `tests/api_doc_drift.rs` and
  `tests/callback_doc_drift.rs` fail when `docs/api/` and the live Lua surface
  disagree in either direction. They are dev-only, so only the `--features dev`
  run sees them.
- **The hooks are not a substitute.** `.githooks/pre-commit` runs formatting and
  the size gate only, and only once `make setup` (or the cloud session-start
  hook) set `core.hooksPath`; `git commit --no-verify` and `git rebase --continue`
  skip it. `.githooks/pre-push` runs rustfmt on both crates when `HEAD` is pushed
  (`make resolved` runs it by hand). Nothing heavy is caught for you. The full
  list is not run before every push: checkpoint commits stay cheap.
- **Human-only checks** go under a heading that starts `For a human`, as `- [ ]`
  items. They never hold the merge; `make checks SINCE=<time>` gathers the
  unticked ones from PRs merged since then. Nothing for a human: no heading, or a
  sentence, never a `- [ ]` saying so.
- **Failures land in `.lint/report.txt`**: read it rather than re-running.

## The merge, one tested tree at a time

A batch runs `make queue ARGS=--watch` under its supervisor, `make queue-daemon`
(`issue-batch` says how), typically `ARGS="--watch --no-check --for 70"`. The
watch takes every open pull request that is **ready and labelled `queue`**: the
label is the orchestrator's go-ahead after reading the description and diff;
removing it pulls a PR out of line. Dependabot's cargo bumps label themselves,
its Actions bumps don't. `make queue PRS="a b c"` takes the named PRs in that
order, label or not.

**Order.** The PR that textually conflicts with the fewest others in line goes
first (`git merge-tree` per pair, no checkout), so the broad one lands last and
takes the one conflicted update. Ties go by label, infrastructure → architecture →
bug → foundation → feature (from the PR and the issues it closes; a `bug` touching
only tooling — `.github/`, `.githooks/`, `tools/`, `.config/`, `.claude/`,
`Makefile`, `clippy.toml`, `deny.toml`, `.gitignore` — ranks as infrastructure),
then unlabelled, oldest first; Dependabot last. Of two open PRs editing
`Cargo.toml`/`Cargo.lock` (drafts included) the younger waits (`watch: LOCKFILE …`).
The watch says when the count overrides the label (`watch: #A goes before #B, …`);
a pair git cannot answer about counts as no conflict. **No PR starves:** one that
has waited 60 minutes or seen three others taken ahead goes first
(`watch: #N has waited …`). `--since <ISO time>` limits the board lines to PRs
opened since then.

**One take**, for a PR that goes alone (always, for named PRs):

1. Merge the latest `origin/main` into the branch (`git merge --no-edit`, never a
   rebase) in a throwaway detached worktree.
2. Compile-check the updated tree: `cargo check --locked --all-targets` with `dev`
   and with `--no-default-features`, then the size gate, into
   `target/merge-queue`. Skipped for Markdown-only branches, Dependabot and
   `--no-check`. Every merged tree but a Markdown-only one touching no agent doc
   (a `CLAUDE.md`, a skill's file) still gets `tools/lint`'s build-free passes, and
   so does a head touching one that `main` passed only by Markdown (merged
   unpushed, just to lint) (`merging/queue/lints.py`; size, rules, layers, determinism, direction,
   components, parity), built once per `tools/lint` tree into
   `target/merge-queue-lint`; a violation hands the head back unpushed, naming the
   pass, rule and file and the PRs merged since the branch's base that touched it.
   A lint tool that cannot build or start is said once and the tree left to CI; a
   machine failure is said and retried on the next take.
3. Push, leased on the head the take began from
   (`--force-with-lease=refs/heads/<branch>:<head>`, the one deliberate lease in
   the normal flow), skipped when the merge moved nothing. If anything moved the
   branch since the take began, the push is refused: the watch retakes it on the
   new head (`not ready — its branch moved during the take`); a named PR is
   reported *not merged* and counts as a hand-back.
4. Wait for the runs **on the updated head**. A push landing during the wait
   sends a watched PR back in line (`watch: #N goes back in line: …`); a named PR
   is handed back. A pushed head GitHub never shows is a hand-back.
5. Ask `make mergeable`'s judgement (next section).
6. Squash-merge as `Title (#issue) (#pr)` (the queue adds each closed issue the
   title lacks), one body bullet per branch commit minus its merges of `main`,
   with `--match-head-commit`. "Base branch was modified" is retried three times,
   then the PR is taken again.

**Hand-backs.** The watch prints `queue: HANDED BACK #N: <why> …`, skips that PR
and keeps merging the rest. Causes: a conflict (naming the paths), an updated
head failing the local check (never pushed), a red run, a run that never
appears, a draft, a description that claims what it may not, a merge GitHub
refused (a 502 on merge is followed by asking whether it merged). A ready PR
that already conflicts with `main` is handed back at once, unless the take
resolves every conflicted file itself (below). The handed-back head is
remembered (under the git directory) and passed over until it moves. What brings
each kind back:

- **Red CI, a conflict, an image:** a push, or a new run attempt on the head
  (`gh run rerun --failed ID`, no push). A hand-back for red `ci` or `lint` also
  comes back by itself once its head's runs read green.
- **Outwaited runs** (deadline passed, nothing red or green): comes back by itself
  once every run is green; re-run nothing. If they end cancelled or red, the
  watch says so once and it becomes a red hand-back.
- **The description or labels** (a missing `Closes #N`, a stray keyword, an
  issue-less code change): "no push needed"; retaken once the body or labels change.
- **The issue refuses it** (`planning`/`human`, an open blocker, already closed):
  retaken once the issue no longer refuses it.
- **No review decision on record** (`independent-review`, *Answering the
  findings*): retaken once that comment appears.

A run is judged by its **latest attempt**. **A runner outage is not red:** when
the only non-green jobs are `cancelled`, or failed in a setup step before
anything built (`merging/verdict/setup.py`), the queue re-runs them once within
the `--deadline` and hands back only if it repeats (`runners unavailable …`). A
PR that edits its own setup (`rust-toolchain.toml`, `.github/workflows/`,
`.github/actions/`; any train member) owns those failures: handed back at once,
re-run once the runners are back. `make queue-status` names every ready,
labelled PR passed over for an hour on a green head (`STRANDED #N: …`).

**Exits.** The watch ends on `--for`, when nothing is left (only drafts,
unlabelled or handed-back heads, unmoved for `--idle` minutes, default 90), or on
a machine failure. Its last line is `watch: resume: <command>`.

| Status | Meaning | Do |
|---|---|---|
| 0 | nothing to read (or `--for` reached) | relaunch with the same arguments (the supervisor does) |
| 1 | something was handed back | grep the output for `HANDED BACK` |
| 3 | the machine or GitHub failed; nothing known about the branches | fix the machine; transient `gh`/git errors are retried for about a minute first |
| 4 | another watch holds `.git/merge-queue.lock` | it names the PID |
| 5 | its own code changed on `main` | update its worktree to `origin/main` and relaunch (the supervisor does) |
| 6 | `make queue-stop` asked | — |
| 9 | `main`'s tree is not what a train proved | the supervisor never relaunches: compare `main` with the members before merging anything |

The supervisor (`merging/watch/supervisor.py`) passes a status through when it
gives up, and adds **1** (a checkout it refuses to move), **7** (its worktree is
gone), **8** (crashed), **10** (`make queue-daemon` started from the main working
tree; it prints the command to run from `.claude/worktrees/merge-queue`) and
**11** (could not lock its worktree; `git worktree unlock <path>` clears a lock
that is not a supervisor's). `make queue-daemon` exits non-zero with the log's
tail when the supervisor dies in its first seconds, and reports one never seen
holding its lock as *unconfirmed*: ask `make queue-status` before trusting it.
`make queue-events` is the stream to watch. Stop it with `make queue-stop` or
`kill <pid>`.

**Deadlines.** `--for MINUTES` is checked only before a take, and a take can
outlast it by the per-PR `--deadline` (default 45, shared by Dependabot's rebase
wait and CI wait). A background command is killed at two hours, so the watch
refuses a pair summing past 120; when runners are slow, raise `--deadline` and
lower `--for` together (`--for 50 --deadline 60`).

**The machine failing stops the whole queue** (out of disk or memory, rustc or the
linker killed): the rest are named *not taken*. Run it from a worktree under
`.claude/worktrees/`, never the tmpfs scratchpad; it refuses a target dir on tmpfs.

**What it leaves to you.** Its local check only refuses a push, never grants a
merge (`make gates` before readying is the author's job); it never reads the
mutation or coverage signal; it removes no worktree and deletes no branch. The
summary lists the merged ones: remove each worktree (several GB of `target/`)
and branch once nobody is in it (`make reclaim`).

- `ARGS=--dry-run` reads everything and computes merges locally, pushing and
  merging nothing; `ARGS=--no-merge` does the update and wait and stops at green.
- **Dependabot branches are never pushed to.** The queue comments
  `@dependabot rebase` when one is behind and judges the new head, within one
  `--deadline`.
- **A Dependabot PR editing `.github/workflows/` or `.github/actions/` is held**
  (`queue: HELD #N: Dependabot edits …`, not a hand-back): its run never
  executes the workflows it edits. Read the changelog, dispatch each touched
  workflow on the branch (`gh workflow run <file> --ref <branch>`, or MCP
  `actions_run_trigger`; `mutants-sweep.yml` only when no batch needs the
  runners), and once green merge it by name with `make queue PRS=N`.
- **By hand**, the same loop: `git merge origin/main`, `cargo check` both feature
  sets and `make size`, a plain `git push`, wait, `make mergeable PR=N`,
  squash-merge. Prefer the queue. Never rebase a pushed branch (a force-push is
  refused); `--force-with-lease` is only for a branch that must be rewritten (a
  committed secret) and the queue's own push.

### The merge train

At each take the watch builds a train of up to 4 (`train.group.CAP`): a chain
of single-parent commits on `origin/main`, one per member, each the tree
`git merge-tree` writes for that head on the chain so far. A PR rides only if it
passes the take's first checks (ready, labelled, review recorded, description),
**its own head's `ci-gate` and `lint-gate` are green**, and it merges cleanly onto
the chain; otherwise it waits this round and keeps its place (`watch: #N waits …`),
never updated for the train.

These **go alone**, by the single-PR steps: the first in line when it cannot ride
(it conflicts with `main`, even a `sorted` conflict the take resolves; its CI is
not green; Dependabot); a Markdown-only PR as CI's `code` filter counts it; one
touching `.github/scripts/merging/`, `.github/workflows/`, `.github/actions/`,
`tools/lint/` (except the data tables in `train.group.RIDES`), `Makefile`,
`merge-queue.py`, `queue-daemon.py`, `Cargo.toml` or `Cargo.lock`
(`watch: #N goes alone: it changes …`); one whose head was in a red train; a
train that found nobody to ride with its first. A train's take counts as one pass
for those behind it in the aging count.

The tip is pushed to `merge-train/<main>-<a>-<b>…` (never forced) and `lint.yml`
then `ci.yml` are dispatched on it. It is checked first as step 2 (compiled
unless `--no-check`, `tools/lint`'s passes either way; a violation splits the
train and hands no member back; a checkout failure is a machine failure). Its
runs are read like a PR's. The branch is deleted once they settle. Then:

- **Green:** if `main` moved, the members are rebuilt on the new `main`. If `main`
  cannot be read, nothing lands (exit 3). Every member is re-read before the first
  squash; one that moved (head, label, draft, description) goes back in line and
  the rest are rebuilt without it. Then each is squash-merged in order as step 6,
  and `main`'s tree must equal the train's with exactly that many new commits, or
  the watch prints `watch: STOPPED: main's tree is not the train's …` and exits 9.
- **A member refused or lost in transit** ends the landing: it is handed back,
  every member after it goes back in line, and what landed is checked against the
  train's trees. "Base branch was modified" is retried three times, then the
  member goes back in line. GitHub failing mid-landing settles what landed, checks
  `main`, and exits 3; if `main` stays unreadable for about five minutes
  (`watch.trains.CHECK_TRIES`) the exit says the landing **was not compared**:
  compare `main` with the tree it names by hand.
- **Red:** split in half, each half retested on the latest `main` before any new
  train; a green half lands, a half of one goes alone, so a PR red on its own is
  handed back with its run linked. A train that only **stalled** splits the same
  way without marking anyone red, and one red only through a runner outage is re-run,
  not split.

A dry run prints the train it would form. A watch killed mid-train leaves a
`merge-train/*` branch; the next watch deletes them all before its first take.
`main-health` reads only `main`'s runs, so a red train files nothing.

### The Markdown exception

CI's `changes` filter starts `code: - '!**/*.md'`: a Markdown-only PR skips the
heavy steps, but its gating jobs still run (a ruleset reads a skipped required
check as unsatisfied) and succeed, save `lint`'s size gate: it holds every agent doc
and goes red on one over its cap. The three Markdown paths code reads (`docs/api/`,
`docs/game-agent.md`, `crates/core/src/project/game_claude.md`) are listed in CI's `code` filter and in
`merging/verdict/changes.py`'s `CODE_DOCS`, so a PR touching one runs the tests
and is serialized like code.

## Verify the run happened on the head commit

    make mergeable PR=N

Exit 0 means CI genuinely ran on the head commit and passed; anything else prints
why. It is the last thing before any merge, and the queue asks it too.

`main`'s ruleset is `.github/rulesets/main.json`: no deletion, no force-push, and
`ci-gate` and `lint-gate` required on the head from GitHub Actions, not strict.
`make ruleset` reports drift from the live ruleset; `make ruleset ARGS=--apply`
writes it, which the orchestrator does between merges, never a coder, after a PR
changing the file. **The ruleset does not replace `make mergeable`:** a draft's
run leaves both gates green with nothing built, and a draft readied without a
push shows that run's gates until the new one's finish; nor does it read the
description.

What `make mergeable` asks:

- **What the PR claims** (`merging/verdict/closing.py`, `protocol.py`,
  `carried.py`), read at merge time: no closing a `planning` or `human` issue
  (removing the label greenlights it, no push), or one with an open recorded
  blocker; no closing keyword at a PR, a closed issue or a missing number, or
  anywhere but on a line of its own; a head branch `N-…` closes #N or says
  `Refs #N`; an issue-less PR touching anything but Markdown (or a `CODE_DOCS`
  file) carries `bug` or is Dependabot's; no image (`png`, `jpg`, `hdr`, `exr`,
  `ktx2`) added outside `IMAGES_ALLOWED`. Each is fixed by editing the
  description, a label or a relationship, and an image by a commit that deletes it. No **Gates** line passes with a warning.
- **Both workflows on the head SHA**, `ci-gate` and `lint-gate`.
- **A gated job itself succeeded.** The gates pass over skipped jobs, so a draft's
  run is gate-green with nothing compiled; `changes` succeeding proves nothing.
- **Every run on the commit.** A red run beside a green one refuses. A cancelled
  run is ignored only when a non-cancelled run of the same workflow on the commit
  is still going (wait) or was created no earlier; otherwise it refuses.
- **No run at all**: a branch conflicting with `main` gets none (GitHub cannot
  build a merge ref; don't edit the workflow), versus a PR readied just after a
  push that lost its run (push an empty commit). An invalid workflow produces a
  `startup_failure` run.
- **A Markdown-only change to a `CODE_DOCS` file** needs `build-test`'s dev test
  step to have really run.

It ignores what is not a gate: `main-health`, coverage, mutation and `cargo-test`,
and jobs outside a gate's `needs:` (a `ci` run counts as settled once `ci-gate`
concluded). Its tests run in `make scripts`.

`gh pr checks N` is **not** a substitute: it has no commit column and blends runs.
By hand, ask about the commit:

    SHA=$(gh api repos/:owner/:repo/pulls/N --jq .head.sha)
    gh api "repos/:owner/:repo/commits/$SHA/check-runs" \
      --jq '.check_runs[] | "\(.name)\t\(.status)\t\(.conclusion)"' | sort

**Never hand-roll a "wait for CI" loop that treats zero checks as success.**
`make queue` is that loop, written so it cannot. (GitHub's native merge queue
needs an organization-owned repository; moving the repo is the user's call.)

## A red ready pull request stays ready

Fixed in the next commit, not sent back to draft. Draft is for unfinished, blocked
or handed-over work, including the hand-back after ≈3 attempts at the same
failure, with the reason in the description.

## Updating a branch from `main`, and its conflicts

Expect conflicts where every feature appends in an order that matters:
`app/registry.rs`'s ordered `register` calls, `docs/api/index.md`'s namespace
list, `scripting/callbacks.rs`. Lists under a `sorted` marker (`api/mod.rs`, the
Lua surface, the lint rules, `components/registry.rs`'s `first_class!` rows, from
which `Entity`, `ComponentKind` and the Add Component menu are generated) mostly
merge clean; when two entries meet, keep both, in order.

**`make queue` resolves one kind by itself:** when every conflicted hunk of every
conflicted file lies in a `sorted` list and both sides only added entries (none
edited or removed, no comment or attribute line in or directly above the hunk, no
key added twice with different text), it keeps both in key order, commits and
pushes, and CI judges it. One file outside that rule hands the whole merge back.
`Cargo.lock` and `app/registry.rs` never qualify. Every conflict it meets is
recorded in `merge-queue/conflicts.jsonl` under the git directory; the watch
prints totals in its summary and `make conflicts SINCE=24h` on demand. Read them
before building more conflict automation.

**Both authors being right is the common case:** keep both sides, ordered
deliberately. `app/registry.rs`'s `build()` is the **only** place per-frame
system order is defined; merging it by textual proximity silently reorders the
schedule. Mechanical resolutions (a `mod` list, an import) are fine to do
directly; the `pre-push` hook (or `make resolved`) runs rustfmt, but a lost
`#[cfg(test)]` or a file over its cap still waits for CI. Hand a conflict back to
its author when resolving it needs to know *why* the code is shaped so: a
variant joining a documented grouping, two prose paragraphs to order, a grown
signature, a system whose stage placement was argued for.

**A `Cargo.lock` conflict is never hand-merged** (hunk-merged lockfiles resolve to
a graph neither side built). Mid-merge of `origin/main`: `git checkout --theirs
Cargo.lock` (during a merge *theirs* is `main`), `cargo update -p <only the crates
this branch moves>` (never a blanket `cargo update`), then `cargo check --locked
--all-targets` and `make deny` before pushing. Under `--no-check` such an update
goes back to a coder, never a quick orchestrator fix.

After an update, re-check any claim **about the base** the branch measured (the
Gates line, a survivor count, a coverage number, a byte-identical proof): a
measurement against an older `main` is stale. The queue dates the Gates line on
each update it pushes (`updated by the queue to main at <main>; local gates not
re-run; CI judged <head>`).

## The same-commit obligations

Each backed by a gate that otherwise fails at merge time:

- **A binding change updates its namespace's page in `docs/api/`** (existence
  parity both directions, dev feature set; signatures out of scope).
- **A new lifecycle callback updates `docs/api/index.md`'s callback section**,
  against `crates/script/src/scripting/callbacks.rs`.
- **A new first-class component satisfies all four axes** (the `Entity` field, the
  Add Component entry, an inspector card, a documented `crates/script/src/api/` namespace) or
  gets a `WAIVERS` row in `tools/lint/src/components/waivers.rs` with a reason.
- **An inspector card routes through `scene::authoring`**: `--parity` fails on a
  direct field write.

A new top-level `src/<dir>/` owes the coverage ratchet nothing:
`.github/scripts/coverage-ignore.py` derives its lists from `src/` and
`coverage-baseline.txt`. A new *floor* is one line in the baseline.

There is no format or bake version to bump. The only version stamps are the
import cache's (a converter's `version`, held by `asset::convert::fingerprint`,
and `asset::texture_import::ENCODER_VERSION`), bumped when output changes. Replay
is guarded by the determinism gate instead.

## The signals

Mutation and coverage are **signals, never gates**: neither fails a build, holds a
merge or runs on a pull request.

| | on request | weekly |
|---|---|---|
| mutation | a merged PR's `Mutation: diff` (or globs) line, dispatched by the queue after the merge; by hand, `make mutants-remote SCOPE=diff` (or globs) on `mutants-on-request.yml`: one runner, report and survivors' diffs in the terminal, posted as one comment on the branch's PR (open, or merged). One run per branch: a newer dispatch cancels the older and an unmerged close cancels what is left (`mutants-cancel.yml`); on `main` each run stands alone | `mutants-sweep.yml`, Saturdays, the sim modules in as many shards as `mutants-plan` sizes, a summary and artifact; skipped when `main` has not moved since the last sweep |
| coverage | `gh workflow run coverage.yml --ref <branch>` | `coverage.yml`, Mondays, ratchet table |

**A branch that adds mechanism** (arithmetic, a state machine, a boundary) asks
for a scoped run with a line in its description, `Mutation: diff` or
`Mutation: <globs>`. The queue dispatches it after the merge
(`merging/after/`), never the coder, so a signal never slows the gating CI:

- **What runs:** `mutants-on-request.yml` on `main` at the squash commit (input
  `commit`); `diff` is that commit's Rust change against its parent
  (`mutants-scope.py --commit`). A `diff` ask whose commit changed no Rust is not
  dispatched; globs pass as written; an unreadable line asks for nothing.
- **The cap:** two at once (`merging/after/slots.CAP`); the rest wait in
  `merge-queue/mutants.json`, surviving a relaunch, and are dispatched as slots
  free (also by `make queue PRS=N`).
- **The report** lands on the merged PR as one comment edited in place. `after:`
  lines in the queue's log say each dispatch, end and refusal (refused three
  times: dropped, and said; a network failure keeps its place). `make
  queue-events` passes the lines that mean a report was lost (`dropped`,
  `could not …`, `never appeared`).

Keep `make mutants-remote` by hand for a question you are waiting on; a scoped run
takes 30 minutes or more. Survivors found after a merge go in a comment on the
issue, and a follow-up PR off `main` when worth fixing.

How to read a clean report:

- **The weekly sweep covers only `src/app`, `crates/script/src/scripting`, `crates/physics/src`,
  `crates/navigation/src`.** Elsewhere, silence means unmeasured, not clean.
- **A scoped run names what it planned** (how many mutants, any from outside the
  scope). A `diff` with nothing to mutate says so; it is not a pass.
- **A run stopped at its budget names what it never reached**: unmeasured; ask
  again narrower.

### Triaging a survivor

Read the report when it lists survivors in code **this branch wrote**; survivors in
untouched code need no reading.

**Code behind GPU timestamps is never measured on CI.** Lavapipe has no
`TIMESTAMP_QUERY`, so the `gpu_ms` / `gpu_passes` path and the bench's clock
normalisation run only on a real GPU, and their mutants read `MISSED` on every
runner. Pin the pure arithmetic with ordinary tests, re-measure the rest on the
operator's Mac (`make bench`, alternating before and after) when it changes, and
don't file its survivors as gaps. Everything else GPU is measured: the mutation
workflows install lavapipe and set `RUSTY_REQUIRE_GPU=1`, so a lost driver or a
capture wrongly reporting "no adapter" fails the run instead of skipping.

Sort by cost:

- **Fix what is cheap** while the code is in hand.
- **File a real bug** and fix it after the current branch merges.
- **File an architectural or foundational crack**, and don't start it without the
  user's judgement.
- Or **exclude it** with a written reason, **line-qualified**, so it cannot
  swallow a real gap next door.

Stop the queue only for a report saying a **module** has nothing asserting its
mechanism at all.

**Establish equivalence by applying the mutation and running the suite**, never by
reasoning about symmetry. Re-ask narrower (`make mutants-remote
SCOPE=crates/physics/src/query/through.rs`), or reproduce locally as the job runs it
(nextest, fail-fast, two test processes to fit the memory limit, and the workspace's
own baseline, because cargo-mutants' tests only the mutated package; `docs/testing.md`,
*Mutation testing*, says why):

    git diff origin/main > branch.diff
    cargo nextest run --workspace --features=dev --fail-fast --test-threads 2   # time it
    cargo mutants --workspace --test-tool nextest --baseline skip --timeout <3x secs> \
      --features dev --in-diff branch.diff -- --fail-fast --test-threads 2

`cargo mutants --list` names the functions and builds nothing: reach for it before
believing a structural explanation of a bad report.

Two shapes to recognise:

- **A measurement that discards sign cannot test an operation that changes it**:
  `Vec3::length`, a distance, a magnitude, a squared term, a count, an absolute
  value, a `Quat` dot are all blind to `+` → `-`.
- **A boundary comparison surviving both `<=` and `>=` means neither end is
  exercised**: an accumulator never landing exactly on a step, an agent exactly on
  its arrival radius, a contact at the epsilon, a trigger overlap beginning and
  ending on one tick. A real gap until measured otherwise.

Why this is not a gate: `--in-diff` line matching drifts after an update from
`main`, and a mutant inducing a slow loop times out and reports as a survivor.

### The coverage floors

`coverage-baseline.txt` holds line floors for `app`, `scripting`, `physics`,
`navigation` and `api`. It is a **ratchet, not a gate**: `coverage.yml` runs
weekly and flags a drop without failing. Raise a floor as coverage improves;
lowering one needs an inline `# reason:` (`make scripts` fails otherwise).

The platform layer (`shell`, `render`, `dev`) is unfloored. GPU tests run on both
CI OSes, Linux on lavapipe and macOS on Metal, with `RUSTY_REQUIRE_GPU=1`, so a
green run means they rendered. Lavapipe is a software adapter: a change that
depends on real-GPU behaviour is proven on macOS's Metal.
