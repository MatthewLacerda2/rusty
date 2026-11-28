# The orchestrator's loop

What the orchestrator does, wherever it runs; `SKILL.md` keeps the batch's other
steps (how many coders, what runs together, where a branch runs).

## Where it runs

**In the cloud** (`make orchestrator`): an hourly routine whose fresh session follows
`.claude/agents/orchestrator.md` for one fire, its only job orchestrating. The local
session takes what needs the machine: GPU checks on the real adapter, benches (`make
bench-ab`, `make bench-scenes`), bakes of the operator's scenes, a look in a real
window. It finds that work where the fires leave it: `Needs:` lines without a `Done:`,
the unticked items `make checks` gathers, and comments on the issues.

- **`make orchestrator`** creates the schedule from this machine (the routines API
  is reached through `claude mcp serve`, as `make coder` reaches it) and stores its
  id on the lock, beside the cap; `ARGS=--stop` disables it, found by that id
  first (the routines listing holds only the newest, and a fallback to it warns),
  `ARGS=--dry-run` prints it. Hourly: the API refuses a shorter
  interval. Keep a fire under the hour (the queue step's 30 minutes is most of it);
  a fire that overruns finds the lock held by itself's predecessor and ends.
- **One orchestrator at a time, across machines**: the `orchestrator-lock` branch,
  a commit naming its holder and the time, taken and renewed by a push that must
  fast-forward (`.github/scripts/orchestrating/lease.py`). `make orchestrator
  ARGS=status` reads it. A fire takes it first and ends at once, exit 4, when it is
  held; `make queue-daemon` takes it at start and refuses the same way (its one
  `supervisor: … stopped.` line names the holder), renews it while it runs and
  releases it on its way out. A holder that stopped renewing is taken over after
  45 minutes. A bare `make queue ARGS=--watch` takes no lock, nor do `make coder` and
  `make coders` by hand: to orchestrate from this machine again, `make orchestrator
  ARGS=--stop` first, and let the running fire release.
- **The queue's state travels with the lock**: each renewal and release saves the
  `.git/merge-queue/` state files (hand-back memories, the kept board, the waiting
  line, `quiet.json`, the mutation requests) as the lock commit's tree, and a side
  taking it from another place restores them, so a fire's memory outlives its
  container. `quiet.json` keeps the last poll of coders' comments, so a comment
  posted after one fire's poll is said by the next.
- **The cap lives on the lock** (`orchestrating/carry.py`, `SHARED`): `make coders
  ARGS='--cap N'`, on either side and whoever holds the lock, pushes it into the
  lock's tree at once without changing the lease; renewals keep the lock's copy, and
  every take, on either side, restores it over the local file. So a cap is set
  through the lock: a push GitHub refused is said, and the cap then holds on that
  machine only until its next take of the lock puts the lock's copy back; set it
  again once GitHub answers, or after that take.

## A cloud fire

Each step is one command; every command after the second is prefixed with
`RUSTY_ROUTINES=<file>`, so the scripts list and launch through this session's own
tools (`dispatch/handed.py`): a script in a cloud session cannot reach the routines
API, and a routine made with `create_trigger` starts its session without the
repository, so a fire launches with `create_session`.

1. `make orchestrator ARGS=take`. Exit 4: say who holds it and end. Its `last
   <time>` line is when the lease it replaced was released or last renewed: that
   time is `<last>` below (no such line when the lock was never taken: drop
   `--since <last>`).
2. Write `<file>` in the scratchpad, `{"triggers": …, "sessions": …,
   "environment_id": …}`: the `list_triggers` result (limit 25, `include_completed: true`), the
   `list_sessions` result (`mine: true`, limit 30) and this session's
   `environment_id` (`get_session`). Of each row copy only the fields
   `dispatch/handed.py` reads (`TRIGGER_FIELDS`, `SESSION_FIELDS`): a routine's row
   is mostly connectors. `include_completed` matters: without it the listing hides
   fired routines, and a fired coder still holds its slot.
3. `make blockers ARGS=--fix` and `make board`; re-read the board as `SKILL.md`,
   *Starting, and re-reading the board*, says. Write the briefs the board calls for
   and launch with `make coder ISSUE=N`.
4. `make queue-quiet ARGS="--once --since <last>"`: the drain pass (`make coders`;
   a failing pass is said), the shepherds owed a start, the rounds owed a relaunch,
   coders' comments (from the last fire's poll, `quiet.json`'s; `<last>` only when it
   holds none), new bugs, a red `main`. Act on what it says, as the bullets under *The queue on this
   machine* say.
5. Send the outbox: for each line `make orchestrator ARGS=outbox` prints, call
   `create_session` with its `create_session` object exactly, then `make orchestrator
   ARGS="outbox --sent <id>"`. Re-run step 4 if any coder slot freed meanwhile.
6. `make orchestrator ARGS="queue 30"` from a worktree of its own under
   `.claude/worktrees/`: the watch, `--no-check`, under the lock it renews; it is
   moved to the background at ten minutes, so wait for it to end before step 7.
   Read its hand-backs as *Per pull request*, step 2, says.
7. `make orchestrator ARGS=release`, always, then end with the fire's few lines.

Only step 6 renews the lock by itself; after any other step that ran past twenty
minutes, `make orchestrator ARGS=renew`.

The orchestrator writes no code: not a feature, not a fix
a review asked for, not a conflict's resolution. It picks work, writes issues
and briefs, reads diffs and labels; the one branch it writes itself is a small
documentation or doctrine PR (a skill, a `CLAUDE.md`, an agent definition).
GitHub is the batch's state: open PRs,
their comments, the routines and sessions lists and the lock resume a batch from
nothing, so nothing lives only in the scratchpad.

## The queue on this machine

**Start the queue once a draft exists, under its supervisor** (only while no cloud
orchestrator is scheduled, or with it stopped; it refuses while a fire holds the lock):
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

- **Watch it with `make queue-quiet`**, the one `Monitor` command, never a
  hand-typed `tail | grep` or a loop of your own. It follows the log
  `make queue-events` does (below) and handles a batch's routine itself: whenever
  a merge lands it fast-forwards the main checkout to `origin/main` (only on
  `main`, only forward) and runs `make reclaim`; once a minute it runs the drain
  pass (`make coders`; `ARGS=--no-drain` leaves that to you). It starts the
  **shepherd** (`make shepherd PR=N`, below) on a ready PR with no review
  decision or past its verdict's head, and relaunches the coder of a draft
  whose newest record is a `round` on its head, both found on every poll from
  GitHub alone, so what happened while the watch was down is acted on once it
  is back; each is kept by head or record in `quiet.json`, done once. It says only what someone must act on, each line beginning
  `QUIET:` or as the log said it: a shepherd's `round` sent or `hold` and why
  (never a `queue`), a shepherd that could not start, a PR reviewed with
  nothing that queues it or owing a `Needs:`; with `ARGS=--no-shepherd`, a PR
  ready with no review decision or past its verdict's head, once per head; a hand-back, a
  stop, a new `bug`, a red `main`, a coder's comment (the full list:
  `merging/view/quiet.py`). A merge, a draft opening and a PR closing say nothing; it exits 1
  when the supervisor is not running. **A limit:** a shepherd that ends with no record is not
  started again on that head, and nothing is said: run `make shepherd PR=N`, or review by hand.
  `make queue-events` is the unhandled stream,
  for reading the whole board: it replays the log from 30 lines back (a re-arm from
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

0. **The shepherd runs the review loop**, not you: a cloud routine on the
   latest Opus (`.claude/agents/shepherd.md`, its steps in
   `independent-review/shepherd.md`), one fresh session per turn, started by
   `make queue-quiet` or by hand with `make shepherd PR=N`. It reviews (or
   writes `No review: <why>`), posts the record and its verdict, writes a
   round's `Brief` and flips the PR to draft; the watch relaunches the coder.
   It sends you, word for word under a `hold`, what it may not decide. Keep
   the batch's notes a review needs in `independent-review/notes.md`. The
   steps below are yours when you run the loop by hand (`--no-shepherd`), and
   for every PR the go-ahead's overrides stay yours: the `queue` and `hold`
   labels and `Needs:`.
1. **Read the description and the diff, then give the go-ahead**: the review
   record's `Verdict: queue on <sha>` line takes it into the queue with no
   label (`ci-merge`, *The merge, one tested tree at a time*); add the `queue`
   label only to override a verdict, and `hold` to keep a PR out whatever it
   says. Check its decisions against the issue. Engine mechanism, a stored format,
   determinism, CI or the merge path, or a refactor gets the
   `independent-review` skill first; any other code PR gets a `No review: <why>`
   comment (`independent-review`, *Answering the findings*, step 2, says which
   PRs need neither), each with its `Verdict:` line; the queue hands back a PR
   without its record. What the read turns up goes back to the coder under a
   `Verdict: round on <sha>`; `hold` pulls a PR out of line. Its "follow-up" notes
   are filed only as the root `CLAUDE.md` allows (*The operator promotes; agents do
   not file*): folded into a round, a comment on *Findings awaiting the operator*,
   or dropped. If it asked for a scoped
   mutation run (a `Mutation:` line, which the queue dispatches after the merge),
   read the report when it lands on the merged PR (`ci-merge`, *Triaging a
   survivor*): each survivor is a question, so a follow-up names only the ones
   that show behaviour nothing asserts, and leaves the rest; no brief asks for
   zero survivors.
2. **A hand-back** skips that PR and the watch carries on. A mechanical fix (a
   conflict where both sides are kept, a one-line red) goes to the `Agent`
   tool's `merge-fixer`, its prompt the PR, its branch and the hand-back line;
   put `hold` on first and take it off after reading what it pushed (a merge of
   `main` keeps a `queue` verdict; its own commit reads `round`). A resolution
   that needs a compile to trust (a split, a dispatcher, system
   order, `Cargo.lock`), or one `merge-fixer` stopped on, goes to a fresh
   routine.
3. **After a merge:** `make reclaim`, which `make queue-quiet` runs itself (removes merged worktrees and deletes
   finished local branches; keeps unpushed work, a lock, and a worktree in use:
   a process standing in it or running a program from it, or a write in the
   last hour), then re-read the board. A helper's worktree survives it while
   something runs there or was written within that hour; a longer pause (a build
   kept to read results later, a session that may die) needs
   `git worktree lock --reason …`.
