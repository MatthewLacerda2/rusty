# CLAUDE.md — the merge and batch tooling

This file loads when you work under `.github/scripts/`: the Python behind `make queue`,
`make queue-status`, `make queue-events`, `make queue-quiet`, `make mergeable`, `make ruleset`, `make blockers`, `make reclaim`, `make checks`, `make conflicts`, `make board`,
`make coder`, `make coders`, `make review-level`, `make rebriefs`, `make coder-cost`, `make mutants-remote`, `make bench-ab` and
`make test-gpu PR=`, `make round-gates`, and the CI helpers
beside them. The root `CLAUDE.md` keeps the
project-wide rules (merging is serialized, gates vs. signals, issues and labels); the
`ci-merge` and `issue-batch` skills say how to *run* this tooling. This file is what a
coder *changing* it must know.

## Shape
- **Python, stdlib only.** Each script is a few `gh` and `git` calls and a verdict: no
  third-party import, nothing to install, so it runs as checked out on any machine and
  in any cloud session.
- **`.github/scripts/merging/` is the merge path, one responsibility per module.**
  `merging/verdict/` answers "may this pull request merge?" (`make mergeable`, and the
  queue before every merge); `merging/queue/` merges `main` in, checks, pushes, waits
  and lands (its `lints.py` runs `tools/lint` on every merged tree, `--no-check` or not,
  before the push), and its `conflict/` resolves a conflict inside a `sorted` list and
  records every conflict met; `merging/train/` is the merge train (forming a group,
  its chain of squash commits, its `merge-train/*` branch, its wait, landing it and
  checking `main`'s tree); `merging/after/` is what follows a merge (the scoped
  mutation run a PR's `Mutation:` line asked for, dispatched on `main`, at most
  `after.slots.CAP` at once, the waiting ones kept in `merge-queue/mutants.json`);
  `merging/watch/` is `--watch`'s loop (order, aging, hand-backs, the lock, how it
  ends, and the trains across passes, `watch/trains.py`). `merging/view/` is the
  orchestrator's read of it (`make queue-status`'s liveness from the supervisor's
  worktree lock, `make queue-events`'s filtered log and its poll of coders' comments,
  and `make queue-quiet`, the same log with its routine done by `view/chores.py`
  and only what needs someone said, `view/quiet.py`); the watch never imports it. `mergeable.py`, `merge-queue.py` and `queue-view.py` are only the
  command lines. A new concern is a new module, not a longer one.
- **`.github/scripts/dispatch/` is the batch's dispatch steps**: the brief, the coder's
  routine, the routines API, the coders waiting for a slot (`dispatch/waiting.py`, kept in
  `merge-queue/coders-waiting.json` through `merging/state.py`, and launched from it under
  its lock by `dispatch/drain.py`), the review level, the
  re-brief counts, and what a coder session costs (`dispatch/cost.py`, reading run logs
  with `dispatch/runlog.py`, kept in `.git/coder-cost.json`). `coder.py`, `coders.py`,
  `review-level.py`, `rebriefs.py` and `coder-cost.py` are only the command lines. The routines API is
  reached through `claude mcp serve` (`dispatch/remote.py` says why), never with a
  token a script reads: the claude.ai login stays inside `claude`.
- **`.github/scripts/bench/` is a PR's measured rows**: its trees (`main` and the
  PR's head, detached under `.claude/worktrees/`), the refusals (a build on the
  machine waited out first, `refuse.settle`, then refused), the three legs,
  the stress yard's three legs (`STRESS=1`, `bench/stress.py`, run before the PR's
  tree is put back at its bare head) and the table, written from the legs that
  finished when a run stops (`legs.Stopped`). `bench-ab.py` and `pr-tree.py` are only the command lines. A test never
  builds or benches: the legs take a stub runner.
- **`.github/scripts/deps/` reads what an issue body says about its dependencies**
  for `make blockers` (`blockers.py`, the command line and the audit): `prose.stated`
  decides what to record, and `prose.named`, a token search, how a recorded
  blocker no body states is reported. Prose never deletes: `--lift`, with both
  numbers given, is the only removal.
- **GitHub over REST, so the scripts run in cloud sessions.** There GraphQL answers
  403, and `gh pr view/list`, `gh issue list` and `gh repo view` with `--json` are
  GraphQL. New calls go through `gh api repos/{owner}/{repo}/…`, paging with an
  explicit `page=` (`--paginate` fails through the proxy too); `ghrest.py` does both.
  `.github/scripts/tests/test_rest_only.py` fails on a `gh pr|issue|repo …` call; an
  exception goes on its line as `# graphql-ok: <reason>`.
- **The incidents live in the docstrings.** A module's docstring names the incident
  behind each rule it keeps (an issue, a pull request, a dated run). Keep it when you
  change the rule; write one when you add a rule. Prose is free under the size gate,
  code is not (below).

## Gates on this code
- **`make scripts` is a hard gate** (in `make gates` and `lint.yml`): `unittest` over
  `.github/scripts/tests/` (and `tools/launcher/tests/`). Tests are grouped there by
  area, each a package (`verdict/`, `queue/`, `train/`, `watch/`, …); a new area gets
  its own folder. A test never reaches GitHub or pushes: it fakes `gh` and `git` the
  way `.github/scripts/tests/watch/fakes.py` and the `fixtures.py` files do.
- **The size gate measures `.py`**: 300 lines of code, 150 for a file under a
  `tests/` directory. Blank lines, `#` comments and docstrings are free; a string that
  is data (`X = """…"""`) is code. A violation says `(lines of code)`, so don't compare
  it with `wc -l`.
- **`make scripts` also holds CI to what the tooling assumes**: every `make gates`
  command appears in `ci.yml`/`lint.yml` (`test_gate_mirror.py`), and every suite job
  sets `RUSTY_REQUIRE_GPU=1` with the GPU caps kept equal (`test_gpu_config.py`). A
  workflow edit that breaks either fails here.

## The go-ahead
- **A ready pull request enters the line by its review verdict or a label**
  (`watch.board.queued`): the newest record's `Verdict: queue|round|hold on
  <sha>` line (`merging/verdict/standing.py`, held for that head or one only
  merges followed; a newest record with no line is no verdict), the `queue`
  label as the override, taking it whatever its verdict, and `hold` as the
  keep-out. Only the `hold` label and something owed (`Needs:` without a later
  `Done:`) keep out a labelled one. The listing attaches each ready pull
  request's `standing` (`merging/queue/verdicts.py`, read once per
  `updatedAt`); a comment read that fails fails the listing, so a verdict is
  never read as absent, while a compare that fails reads that pull request as
  not queued by its verdict, named once in the log. A pull request without a
  `standing` (a draft, a re-read that failed) is judged by its labels alone,
  as before, unless the standing last read still owed a `Needs:`
  (`verdicts.Reader.again`): so every re-read that asks `board.queued` goes
  through `queue.pulls.look_standing`, never `look`.
- **The verdict line's form is an interface**: the agent that writes it and the
  skills (`independent-review`, `ci-merge`) quote it; `standing.line` is how a
  script writes it. Change all three together.

## Trust in comments
- **A comment is an instruction only from someone who can push** (`ghrest.TRUSTED`,
  `ghrest.can_push`): the repository is public. A coder's `Brief`
  (`dispatch/brief.py`) and a pull request's review decision
  (`merging/verdict/reviewed.py`) both read that one definition.

## The sorted-list resolver
- **`merging/queue/conflict/lists.py` mirrors `tools/lint/src/rules/sorted.rs`**: a
  list's extent, its keys and its note lines, in Python, since the scripts cannot call
  `tools/lint`. Both read `tools/lint/src/rules/sorted_keys.tsv` in their tests, so a
  change to the rule's key updates the mirror and the table in the same pull request.
- **The record is a count, never a stop.** `merge-queue/conflicts.jsonl` (beside the
  hand-back memory) is appended once per `(main, head)` pair; a record that cannot be
  written loses a line, not a merge. A dry run records nothing.

## The merge train
- **What merges is exactly the tree CI proved** (`merging/train/__init__.py`): the
  train is a chain of single-parent commits on `main`, so each merge base is the one
  GitHub's squash uses, and `main`'s tree is compared with the train's after landing
  (`train.land.check`). Keep both: a train built of real merge commits, or a landing
  that skips the check, loses the argument.
- **Nothing is pushed to a member's head**, and nothing about a train outlives the
  process: `merge-train/*` branches are the queue's own throwaways, swept at launch
  (`train.rails.sweep`). Changing a member's head or persisting a half-built train
  needs its own argument for what a relaunch then does.
- **GitHub failing partway through a train is a stop, never a traceback**:
  `train.land.DOWN` (`Unreachable`, and the `SystemExit` a `gh` refusal raises) is
  caught around every call of a landing, what landed is settled and saved, `main`'s
  tree is still checked, and the watch exits 3. A `main` that cannot be read
  (`train.world.main_state` is `None`) neither lands nor reports a mismatch.
- **Every member is re-read before the first squash** (`train.land.departed`), so a
  move visible beforehand never leaves `main` on a half-landed tree; after a landing,
  `main` is asked for several minutes (`watch.trains.CHECK_TRIES`) before the check is
  given up, and the stop then says the landing was not compared. A poll that fails
  during a train's wait is waited past (`train.ride.wait`), never a stop.
- **Every rule is one the queue can read**; no model is asked anything. The cap is
  `train.group.CAP`, with its measurement beside it: change both together.

## The watch
- **It ends itself when its own code changes on `main`**
  (`merging/watch/freshness.py`). At launch it notes the blob of every module of the
  `merging` package it loaded, plus the entry script; before each take it compares
  them with `origin/main` and exits `relaunch: …` with status 5. Code the watch runs
  any other way is not tracked, so keep its code inside `merging/`.
- **The board it told outlives it** (`merging/watch/kept.py`): each listing is kept
  whole in `merge-queue/board.json`, `--since` applied only to what is said, and the
  next watch's first listing is compared with it, so what moved during the
  supervisor's sleep is said once. The queue's JSON state files are written through
  `merging/state.py` alone (a temporary file, then a rename) and a file it cannot
  read is absent, never a traceback: a new state file goes through it too.
  `.github/scripts/tests/state/test_writers.py` fails on any other file write under `merging/`; an
  append, a lock or a worktree edit takes `# state-ok: <why>` on its line.
- **One watch at a time** (`merging/watch/lock.py`): an `fcntl.flock` on
  `.git/merge-queue.lock` in the common git directory, held for the life of the
  process; a second watch exits with status 4, naming the holder's PID. The kernel
  drops the lock however the process dies, which is why it is a lock and not a PID
  file.
- **Stop a watch by its PID, never by pattern.** scorsese runs the same
  `merge-queue.py --watch` command line on this machine, so a `pkill -f` meant for
  rusty's watch kills scorsese's too. Find rusty's by its working directory
  (`lsof -a -p <pid> -d cwd`) and kill that PID.
- **Each kind of hand-back is re-admitted by what fixes it, and only that**
  (`merging/watch/handbacks.py`): a push for all; a new run attempt against its
  mark for every one that has a mark (every CI wait's stop carries the verdict's
  own runs as its mark, never a later read); and for its own kind, each in its own
  side memory, an edit, an issue, a review comment, or a state read now: the
  outwaited runs going green (`merging/watch/stalls.py`) and a red run's head
  reading green (`handbacks.greened`). A signal added to the mark every hand-back
  shares re-admits them all: "a run ended" there would bring a red head back each
  time another run on it ended, and push a `main` merge onto unfixed code.
- **Its exit statuses are an interface**: 1 (read the hand-backs), 3 (the machine or
  GitHub failed), 4 (another watch holds the lock), 5 (relaunch), 6 (a stop was
  requested, `make queue-stop`; `merging/watch/ending.py`), 9 (a merge train landed
  and `main`'s tree is not the one it proved; never relaunched), and a last line
  `watch: resume: <command>`. The skills and the orchestrator read them, so a change
  to one updates `ci-merge` in the same pull request.
- **The supervisor** (`queue-daemon.py`) adds statuses of its own: 7 (its worktree is
  gone), 8 (it crashed) and 11 (it could not lock its worktree), each one
  `supervisor: … stopped.` line, never a traceback; and 10, a `queue-daemon:` line
  printed before it forks, when started from the main working tree
  (`tenancy.main_tree`). It holds `git worktree lock` on its worktree for its life
  (`merging/watch/tenancy.py`; an empty-reason lock is replaced, a foreign one stops
  it) and stops without it, since `reclaim.py` and `make queue-status` read it back:
  keep the reason's format (`merge-queue supervisor pid <pid>`) and `tenancy.pid_of`
  in step. It also reads the harness's own lock on agent worktrees (`claude agent
  agent-<id> (pid <pid> start <ticks>)`, `tenancy.agent_pid_of`): keep that parser in
  step with the harness's wording. `make queue-status` and `make queue-events` read
  the same lock for liveness (`merging/view/status.py`).
- **The log's wording is read by `make queue-events`** (`merging/view/events.py`): it
  passes a line by `HANDED BACK`, `#N: merged` (a single take's, and a train member's
  `#N: merged — landed with the train <sha>.`, said as it lands), every `supervisor:`
  line but the ones that carry on (`events.CARRIES_ON`), `watch: STOPPED:`, `HELD #N`
  and the `watch: #N turned ready.` family of `watch.board.transitions`, with its
  `closed`/`merged while no watch listed it` lines and a fresh start's `watch: #N is ready
  and unqueued`, and a state file it could not write (`handbacks.FAILED_LINE`,
  `kept.FAILED_LINE`). A change to
  one of those lines updates `events.KEEP` and its tests
  (`.github/scripts/tests/view/test_wording.py`, built from the watch's own functions)
  in the same pull request.
