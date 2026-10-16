"""`merge-queue.py --watch` — take each pull request given the go-ahead (#664, #925).

A cloud coder's only report is its pull request, so "done" is visible only as
the flip from draft to ready. Before this, the orchestrator noticed each flip
with a hand-rolled poll and chained one `make queue PRS=N` per pull request —
about 69 queue runs and dozens of ad-hoc watchers in the 2026-09-30 batch.
`--watch` does that noticing: it lists the open pull requests, takes the
highest-priority ready one through the queue's unchanged pipeline
([`queue.taking.take`]), and lists again.

Still invoked, never a service: somebody starts it, it runs on this machine,
and it **exits only when it cannot usefully go on**, so whoever started it in
the background is woken then. A hand-back is not one of those (#751):

- **a hand-back** (conflict, failed local check, red CI, no run, a refused
  merge) **skips that pull request and the watch keeps merging the rest**. Until
  #751 it exited, and every ready pull request behind it waited for the
  orchestrator to read the report and relaunch — one branch's problem stalled
  the batch. The handed-back head is remembered ([`handbacks.MEMORY`]) and passed over
  until it moves, so a fix pushed to it is the whole of re-queueing it, in this
  watch or a later one; `make queue PRS=N` ignores the memory. **A new run
  attempt on that head counts as moving** (#869): the runs on it are
  remembered too ([`handbacks.RUNS_MEMORY`], [`handbacks.rerun_heads`]), so `gh run rerun` after a
  runner outage, or a flaky job re-run by hand, re-queues it without a push. The starter still
  hears at once: each hand-back prints one line beginning [`ending.HANDED_BACK_LINE`]
  (a `Monitor` on the output can wake on it), and the exit status at the end
  says one happened ([`ending.status`]), as scorsese's does (scorsese#615, #690).

It exits on:

- **the machine failing** (#584's stop, or GitHub unreachable for [`ending.FAILS`]
  listings in a row);
- **nothing left**: no open pull request, or none ready and labelled, and no
  head among the rest (drafts, unlabelled, remembered hand-backs) has moved for `--idle` minutes;
- **its own code changing on `main`** (#967): before each take it compares the
  files it runs with `origin/main` ([`freshness.changed`]) and on a difference
  exits [`ending.RELAUNCH`], status 5, with nothing in hand, so a merge that fixes
  the queue is never followed by more merges under the old code;
- **its own deadline** (`--for MINUTES`, #697): past it, the watch takes no new
  pull request. It is checked only between takes, never during one, so the one
  in hand finishes (merged or handed back) and the exit always lands between
  pull requests. An exit with its own last line ([`ending.DEADLINE`]) and status 0
  unless something was handed back: whoever started the watch relaunches it. It exists because the orchestrator runs the
  watch as a background command, and those are killed at two hours wherever
  they are — mid-push or mid-merge included.

Which ready one goes first (#753): **the one that textually conflicts with the
fewest others in line**, then CLAUDE.md's priority, then age. Reordering cannot
make two conflicting pull requests merge cleanly (whichever lands second still
conflicts), but it decides *how many hand-backs* a set of them costs, and each
is a rebase session. On scorsese, 2026-10-03, one broad pull request touching
every registry landed first and handed back three small ones, rebased one after
another at about an hour each; small ones first would have cost one hand-back,
on the broad one (scorsese#738). So at each pick the watch counts, for every
ready pull request, how many of the other ready ones it conflicts with
([`order.clashing`]: `git merge-tree --write-tree` per pair, no checkout, no build).
A remembered hand-back is not in line, so it counts against nobody. It **fails
open**: a pair git cannot answer about counts as clean, so the worst case is
plain label order; the order is an optimisation and never stops the queue. When
the count changes who goes first, the watch says so in one line ([`order.reordered`]).

**Ready is not enough: the watch takes only a pull request labelled
[`board.QUEUE_LABEL`]** (#925). A cloud coder readies its own pull request the moment
its gates are green, before anyone has read the diff, and a green gate does not
read code (the 2026-09-30 batch caught an `unreachable!` in a `Display` impl
only by reading it). So the orchestrator reads the description and the diff and
then adds the label; that is the go-ahead, and removing it pulls a pull request
out of line. An approving review would say the same, but GitHub refuses
self-approval and the pull requests are opened under the owner's account
(scorsese#690 took the label for that reason). Dependabot's cargo bumps add the
label themselves (`dependabot.yml`); its GitHub Actions bumps do not, and are
[`board.held`] besides. `make queue PRS=N` names its pull requests, which is the
go-ahead, so it does not ask for the label.

The tiebreak is the label, read from the pull request's own labels and the
issues it closes (rusty labels issues, not pull requests): infrastructure →
architecture → bug → foundation → feature, then anything else, oldest number
first within a rank. Dependabot goes after every one of those, whatever its
conflicts, and its conflicts count against nobody: it rebases itself. A pull
request whose base is not `main` is never taken: the queue rebases onto `main`.

**A Dependabot pull request that edits `.github/workflows/` is never taken**
([`board.held`], #825). The actions entry in `dependabot.yml` only ever proposes
majors (the workflows pin `@v4`, which floats the rest), and those bumps edit
workflows a pull-request run never executes (the sweep, coverage, Windows,
main-health), so a green run proves nothing about them. The watch says so once
per head, in a line beginning [`board.HELD_LINE`], and goes on; the orchestrator
reviews the bump, dispatches the workflows it touches on the branch, and merges
it by name (`make queue PRS=N`, which does not ask). This rather than dropping
the bumps from `dependabot.yml`: the PR still arrives with its changelog and its
diff, and the hold costs nothing until one does.

**It also says what changed on the board** (#909): one line per pull request
that opened (as a draft or ready), turned ready, went back to draft, gained or
lost the [`board.QUEUE_LABEL`], or closed without this watch merging it ([`board.transitions`]). The first listing after a
launch says nothing, so a relaunch does not replay the board. Before this the
orchestrator ran a second monitor, a `gh pr list` poller, to hear of a coder's
draft or its flip to ready; with these lines one `Monitor` on the watch's
output is the whole view.

**No pull request starves, none waits to learn it conflicts with `main`, and
one watch runs at a time** (#928). The conflict-first order passes a pull
request with one conflict over for every clean one, so past [`aging.AGE_MINUTES`]
in line or [`aging.AGE_PASSES`] takes ahead of it, it goes first whatever it
conflicts with ([`aging.Line`]). A ready head that conflicts with `origin/main`
itself is handed back before any take, naming the files ([`mainline.conflicts`]).
The watch holds an `fcntl.flock` for its whole life ([`lock.hold`]), so a second
one exits naming the first's PID. During a take's CI wait the board is still
listed once a poll (the `tick` [`queue.waiting.wait_for`] calls), `--since`
keeps its lines to the batch's own pull requests ([`board.recent`]), `--for` with
`--deadline` must fit in a background command's life ([`ending.over_budget`]),
and the last line is always [`ending.RESUME`] and the command to start it again.

The decisions are pure functions over plain dictionaries; [`loop.run`] takes every
effect as an argument, so the loop is tested without a network.
"""
