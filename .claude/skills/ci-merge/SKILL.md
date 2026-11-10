---
name: ci-merge
description: Take a finished branch through to merged — the gates, CI, verifying a run happened on the head commit, updating a branch from `main` and its conflicts, and triaging the mutation and coverage signals. Use when a branch is ready, when a pull request has gone red, when a mutation report needs triage, or when merging anything into `main`.
---

# Getting a branch merged

The steps, and the traps. `CLAUDE.md` carries why these exist; this is how.

**The gate list is `make gates`.** The `Makefile`'s `GATES` list mirrors
`.github/workflows/{ci,lint}.yml`, and `make scripts` fails when a gate's command
is missing from them (`test_gate_mirror.py`, #964); `make help` names each gate.

## Before marking a pull request ready

All of these must be green — one command, fastest-failing first:

    make gates

It refuses to start when cargo's target dir is outside this worktree, and checks
its own gate list is complete before running it.

**Both feature sets, always.** Half this crate is behind `dev` — the harness, the
session, the MCP bridge, `Debug.*` — and a default-features-only run compiles none
of it. The two clippy passes and the two test passes are not redundancy. The third
clippy pass (`--no-default-features`) is the editor-free player build (#431): the
only compile without egui.

**Some hard gates hide inside `cargo test`.** `tests/api_doc_drift.rs` and
`tests/callback_doc_drift.rs` fail the build when `docs/api/` (the API reference)
disagrees with the live Lua surface *in either direction* — an undocumented
binding and a documented-but-absent one both redden CI. They are dev-only, so
only the `--features dev` run sees them.

**The commit hook is not a substitute.** `.githooks/pre-commit` runs formatting
and the size gate only — no clippy, rustdoc, tests or dev feature set — and only
once `make setup` (or the cloud session-start hook) has pointed
`core.hooksPath` at it. `git rebase --continue` and `git commit --no-verify` skip it; `.githooks/pre-push`
(#1096) runs rustfmt on both crates, `tools/lint` included, as a push of `HEAD`
leaves (`make resolved` runs the same check by hand). Assume nothing heavy was caught for you.

The full list is deliberately **not** run before every push. Checkpoint commits
stay cheap.

**Human-only checks go under a `For a human` heading** in the description, as
`- [ ]` items (a real window, speakers, a repository setting). They never hold
the merge; `make checks SINCE=<time>` gathers the unticked ones from every PR
merged since then (#929), and it reads only a heading that starts "For a
human". Nothing for a human: no heading, or a sentence under it, never a
`- [ ]` saying so.

**Failures land in `.lint/report.txt`** — read it rather than re-running the tool
to see what it said.

## The merge, one tested tree at a time

`make queue ARGS=--watch` (#664) is how a batch runs it, under its supervisor
`make queue-daemon` (#908; `issue-batch` says how): started once, it takes
every open pull request that is **ready and labelled `queue`** — the
orchestrator's go-ahead, added once it has read the description and the diff,
and removed to pull a PR out of line (#925; Dependabot's cargo bumps label
themselves, its Actions bumps don't); `make queue PRS=N` takes what it names,
label or not — **the one that textually
conflicts with the fewest others in line first** (#753: `git merge-tree
--write-tree` per pair of ready heads, no checkout). Reordering cannot make a
conflict go away, but it decides how many hand-backs a set costs: the broad pull
request lands last and takes the one conflicted update. A tie goes to the label —
infrastructure → architecture → bug → foundation → feature, read from the pull
request's labels and the issues it closes, a `bug` touching only tooling
(`.github/`, `.githooks/`, `tools/`, `.config/`, `.claude/`, `Makefile`,
`clippy.toml`, `deny.toml`, `.gitignore`) ranking as infrastructure (#969) — then unlabelled, oldest first;
Dependabot after all of them, whatever it conflicts with. Of two open pull
requests editing `Cargo.toml`/`Cargo.lock` (drafts included), the younger waits
for the older to merge or close (#969, `watch: LOCKFILE …`). When the count
overrides the label the watch says so in one line (`watch: #A goes before #B,
…`). A pair git cannot answer about counts as no conflict: the order is an
optimisation and never stops the queue. A **hand-back skips that pull request and the
watch keeps merging the rest** (#751): it prints one line,
`queue: HANDED BACK #N: <why> …`, the moment it happens, and the handed-back
head is remembered (under the checkout's git directory) and passed over, by
this watch and later ones, until it moves — pushing a fix is the whole of
re-queueing it. **A new run attempt on that head counts as moving** (#869): `gh
run rerun --failed ID` re-queues it, no push, no relaunch. **A hand-back for a
red `ci` or `lint` run also comes back by itself once its head's runs read
green** (#1951): the watch re-reads them each pass while it is ready and
labelled, so a re-run that went green is taken again whatever the run marks
say (#1920 sat seven hours green because the mark had been read after its
re-run). Green CI never brings back a conflict, a description or a review
hand-back. **A hand-back for
outwaiting its runs** (the deadline passed with nothing red and nothing green,
runners busy or GitHub down; #1831) **comes back by itself while a watch
runs**: the watch asks about its runs each listing and takes it again once
every one is green (a run missing from one listing is still out, #592). Re-run
nothing for it, which would spend a runner during the outage that stalled it.
If those runs end cancelled or red instead, the watch says so once, as a
hand-back (`HANDED BACK #N: the runs its hand-back outwaited ended without going
green: …`, with the run's URL), and from there it is a red hand-back: it comes
back once its runs read green, or on a re-run or a push. `make queue-status`
names every ready, labelled pull request the watch has passed over for more
than an hour on a green head (`STRANDED #N: … last handed back for: <why>`),
so a parked one is seen without reading the log (#1951). A hand-back remembered
*before* #1951 that went green afterwards has no red entry and no failed job, so
nothing brings it back but a new attempt: re-run one cheap job, `gh run rerun
--job <deny's id>`, to make one. Never do that for a stall. **A hand-back no push
fixes waits for what does** (#1279). One for what the description or labels
claim (a missing `Closes #N`, a stray keyword, a wrong number, an issue-less code
change) says "no push needed", and the watch takes it again once the pull
request's body or labels change. One the issue refuses (it is labelled
`planning`/`human`, has an open blocker, or is already closed) says "the issue
blocks it": the watch asks about that issue each listing and takes the pull
request again once it no longer refuses it (or the description changes). One with
no review decision on record (#1764; what counts: `independent-review`, *Answering
the findings*, step 2) says so before any push; the watch takes it again once
such a comment appears, or its body or labels change. One for
red CI, a conflict or an image says "push a fix, or re-run the failed jobs" and
waits for the head to move or run again. A run is judged by its **latest attempt** (#881): when the runs listing still shows an older attempt than the run's jobs do, the jobs' attempt is the run, so a green re-run that landed before the verdict is not handed back. The watch **exits** on the machine failing (below), on its
`--for` deadline, or when nothing is left (no open pull request, or only drafts,
unlabelled and handed-back heads, none moved for `--idle` minutes, default 90). Its exit
status says what to do: **0** nothing to read, **1** something was handed back
(grep the output for `HANDED BACK`), **3** the machine or GitHub failed and
nothing is known about the branches; the most urgent wins. A transient `gh`
failure (a TLS timeout, a reset, a 502) is retried for about a minute first, so
only a GitHub that stays down ends a watch, and then with **3** (#847). The queue's own `git fetch` and push retry the same way, each retry the same plain push (#875, #1568): a git SSL timeout is a **3**, never a hand-back. It composes with every flag below; a
batch runs `ARGS="--watch --no-check --for 70"`. **`--for MINUTES`** (#697) is
the watch's own deadline: once it passes, the watch takes no new pull request,
finishes the one in hand (merged or handed back) and exits with a last
line `watch ended: deadline reached (…); N merged, nothing in hand.` That exit
is not a report — **relaunch the watch** with the same arguments (the supervisor
does, by itself). It exists
because a background command is killed at two hours wherever it is, a push or
a merge included. The deadline is checked only before taking a pull request,
so the take in hand can outlast it by up to the per-PR `--deadline` (45 min),
which a Dependabot pull request's rebase wait and CI wait share (#825):
`--for 70` keeps 70 + 45 under that cap, so every exit lands between pull
requests. When many coders push at once, runs wait for GitHub runners
(2026-10-02: a run's first job started 13 minutes in, and #699 was handed back
green-but-unfinished at 45); then raise `--deadline` and lower `--for` together,
keeping the sum under two hours: `--for 50 --deadline 60`; the watch refuses to
start on a pair past 120 (#928). Its last line is always `watch: resume:
<command>`, the exact command that starts it again. **Only one watch runs at a
time** (#928): it holds an `fcntl.flock` on `.git/merge-queue.lock` (the common
git directory, so every worktree shares it), and a second exits with status
**4**, naming the holder's PID; the kernel drops the lock however the first
dies. **No pull request starves** (#928): one that has waited 60 minutes in line,
or seen three others taken ahead of it, goes first whatever it conflicts with
(`watch: #N has waited …`). **A ready pull request that already conflicts with
`main`** (`git merge-tree` against `origin/main`) is handed back at once, naming
the files, instead of waiting its turn to find out, unless every conflicted file
is one the take resolves by itself (a `sorted` list both sides only added to;
see **`make queue` resolves one kind by itself**, under
*Updating a branch from `main`, and its conflicts*). The board lines keep coming
during a take's CI wait, one listing per poll, and `--since <ISO time>` limits
them to pull requests opened since then: the batch's own. `make queue PRS="a b c"` takes named
pull requests instead, in the order given. Either way, the loop is one tested
tree at a time — so no agent sits through a ten-minute run holding a worktree
open. Under `--watch` that tree is a **merge train** when it can be (below);
otherwise, and always for named pull requests, it is one pull request:

1. Merge the latest `origin/main` into the branch (`git merge --no-edit`, never
   a rebase, #1568), in a throwaway detached worktree it creates and removes
   (never one an agent is standing in).
2. Compile-check the updated tree before pushing it (#571): `cargo check
   --locked --all-targets` with `dev` and with `--no-default-features`, then
   the size gate, building into the queue's own `target/merge-queue`. A clean
   textual merge still breaks when a merge ahead changed a signature. Skipped
   for a Markdown-only branch, Dependabot, and `ARGS=--no-check`. Under
   `--no-check` the merged tree still gets every build-free pass of `tools/lint`
   CI's `lint` job runs (size, `--rules`, `--layers`, `--determinism`,
   `--direction`, `--components`, `--parity`; `merging/queue/lints.py`, #1956):
   `tools/lint` is built once per `tools/lint` tree into the queue's
   `target/merge-queue-lint` (a relaunch reuses the kept copy), and a violation
   hands the head back unpushed: its first line names the pass, the rule and
   the file (what `make queue-events` shows), then the report's first lines and
   the pull requests merged since the branch's base that touched the file,
   re-admitted by a push. A tool that cannot build, crashes or cannot start is
   said once per `tools/lint` tree and the tree is left to CI, as before; a
   failure the machine caused (out of disk or memory, a killed process) is said
   and tried again on the next take.
3. Push it as a fast-forward leased on the head the take began from
   (`--force-with-lease=refs/heads/<branch>:<head>`, a compare-and-swap: the
   queue's own push is the one deliberate lease in the normal flow) — both
   skipped when the merge moved nothing (the branch already held `main`),
   because the run on record is then a run on this exact commit. The lease is
   the guard: a plain push would accept a branch reset backwards or deleted
   mid-take (an author dropping a leaked secret) and squash the dropped commit
   into `main`. If anything pushed, reset or deleted the branch since the take
   began, the push is refused. Under `--watch` the branch is then taken again on
   its new head (`not ready — its branch moved during the take`); named pull
   requests are never retaken, so there it is reported *not merged* and counts
   as a hand-back in the exit status. A refusal whose remote head could not be
   read is a plain hand-back, never "moved".
4. Wait for the runs **on the updated head** to exist and conclude. A push that
   lands during this wait (a head this queue never pushed) is not a refusal:
   nothing was judged about it. Under `--watch` the pull request goes **back in
   line**, remembered against nothing (`watch: #N goes back in line: …`, #1799),
   and its next take merges `main` in and waits on the new head's own CI; a
   named pull request is handed back. A pushed head GitHub still hasn't shown
   after the grace is a hand-back either way.
5. Ask `make mergeable`'s judgement of that head — see the next section.
6. Squash-merge with the house-style title (`Title (#issue) (#pr)`; the queue
   adds each closed issue the title lacks, #966), a body of one bullet per
   commit the branch made with its merges of `main` left out (so `main` reads
   no `Merge remote-tracking branch` lines), and
   `--match-head-commit`, so a push after the verdict makes GitHub refuse.
   GitHub's "Base branch was modified" is asked again three times, then the
   pull request is taken again rather than handed back (#1785's review).

It **hands back** — names why, skips that entry and carries on, named pull
requests and `--watch` alike — on a
conflict (naming the paths; it resolves only a conflict inside a `sorted` list
where both sides only added entries, below), an updated head that fails
the local check (never pushed), a red run, a run that never
appears, a head somebody else moved (named pull requests only; the watch puts it back in line), a draft, a description that claims what it
may not (next section; checked before any push), or a merge GitHub refused. A 502 on
the merge call is followed by asking whether it merged. Exit status is non-zero
if anything was handed back. **A runner outage is not a red run** (#869): when the
only jobs not green are `cancelled` or failed in a setup step before anything
built (installing a tool, the toolchain, the cargo-deny image: #1932,
`merging/verdict/setup.py`), or a gate failed because such a job did, nothing
compiled and nothing failed, so the queue re-runs those runs once
(`gh run rerun --failed`) inside the same `--deadline`, and hands back only if they
come back that way again, as `runners unavailable …`; both lines name each job
cancelled or the setup step it failed in. A train red only this way is
re-run, not split. A pull request that edits what its setup runs
(`rust-toolchain.toml`, `.github/workflows/`, `.github/actions/`; for a train, any member) is judged
on its setup failures: they are its code's, handed back at once. That hand-back needs no
push: once GitHub's runners are back, re-run the run and the watch retakes it.

It **stops the whole queue** — one message, the rest named *not taken* — when a
check fails for the machine's reasons: out of disk or memory, or rustc or the
linker killed (#580). Nothing is known to be wrong with that branch, and every
later one would fail the same way. **Run the queue from a worktree under
`.claude/worktrees/`, never the scratchpad**: the scratchpad is a tmpfs, a cold
`target/merge-queue` build filled it and three healthy branches came back as
broken. The queue refuses to start when its target directory is on a tmpfs.

What it leaves to you: its local check only ever *refuses* a push, never grants
a merge (`make gates` before readying is the author's job), it never reads the mutation or coverage signal, and **removes no
worktree and deletes no branch** — the summary lists the merged ones. Remove each
worktree (~2.5–4.5 GB of `target/` each) and its branch once nobody is standing in it.

- `ARGS=--dry-run` reads everything and computes the merge of `main` locally, but
  pushes, comments and merges nothing — use it to see what a queue *would* do.
  `ARGS=--no-merge` does the real update and wait, and stops at green.
- **The queue never pushes to a Dependabot branch** — Dependabot stops maintaining a
  branch someone else pushed to, and it rebases (and cancels its own runs) by
  itself. The queue comments `@dependabot rebase` when one is behind, waits for a
  head on `main`'s tip, and judges that, all inside one `--deadline` (#825).
- **The watch never takes a Dependabot pull request that edits
  `.github/workflows/` or `.github/actions/`** (#825, #1973). The actions entry only proposes majors, and
  they edit workflows a pull-request run never executes (`mutants-sweep.yml`,
  `coverage.yml`, `windows.yml`, `main-health.yml`), so its green run proves
  nothing about them. The watch prints `queue: HELD #N: Dependabot edits …`
  once per head and goes on; it is not a hand-back and does not change the exit
  status. Read the bump's changelog for breaking changes, dispatch each workflow
  it touches on the branch (`gh workflow run <file> --ref <branch>`, or MCP
  `actions_run_trigger`; for `mutants-sweep.yml` only when no batch needs the
  runners), and once they are green merge it by name: `make queue PRS=N` does
  not hold it. A cargo bump (lockfile only) is taken as before.
- **A watch ends itself when its own code changes on `main`** (#824, #967):
  before each take it compares the files it runs with `origin/main` and exits
  `relaunch: …` with status **5**; update its worktree to `origin/main` and start it again (the supervisor does both, by itself).
- **Exit 9: `main`'s tree is not what a merge train proved** (#1785): the
  supervisor gives up rather than relaunching. Compare `main` with the train's
  members before merging anything more, by hand or with the queue.
- **The supervisor's own exits** (#1450, `merging/watch/supervisor.py`): it
  passes a watch's status through when it gives up, returns **1** for a checkout
  it refuses to move, **7** when its worktree is gone (`supervisor: worktree <path>
  is gone; stopped.`), **8** when something unexpected was raised (`supervisor:
  crashed: <error>; stopped.`) and **11** when it could not lock its worktree
  (`supervisor: could not lock …; stopped.`, or `… is already locked (…), not by a
  supervisor`; #1983): `make queue-status` and `make reclaim` read that lock, so it
  never runs without it. Started from the main working tree, `make queue-daemon`
  refuses before forking with **10** and one `queue-daemon:` line giving the command
  to run from `.claude/worktrees/merge-queue` (or how to make that worktree). `make queue-daemon` exits non-zero, printing the
  log's tail, when the supervisor it started exits in its first few seconds; one
  still running but never seen holding its lock is reported as *unconfirmed*
  (exit 0): ask `make queue-status` (the supervisor's lock PID, checked alive, the
  log's last line, and any `STRANDED` pull request) before trusting it; `make queue-events` is the stream to watch (#1876). Stop a supervisor with `make
  queue-stop` (the watch exits **6** between pull requests and the supervisor
  passes it through), or `kill <pid>` (its worktree lock is released either way).
- By hand, the same loop is: `git merge origin/main` in the branch's worktree,
  `cargo check` both feature sets and `make size`, a plain `git push`, wait,
  `make mergeable PR=N`, squash-merge. Prefer the queue. Never rebase a pushed
  PR branch: a rewrite needs a force-push, which permission checks refuse
  (#1568). `--force-with-lease` is kept for the rare branch that must be
  rewritten (a secret committed by mistake) and for the queue's own leased
  push (step 3); never anyone else's push in the normal flow.

### The merge train (#1785)

At each take the watch walks the line in its own order and builds a train of up
to **4** (`train.group.CAP`, measured: about 5% of pull-request `ci` runs were red
on 2026-11-03/04, where 4 is the best of 2, 4 and 8): a chain of single-parent
commits on `origin/main`, one per member, each the tree `git merge-tree` writes for
its head on the chain so far — what squash-merging them in order will leave. A pull
request rides only if it is ready, labelled `queue`, its review decision recorded
and its description passing (the take's own first checks), **its own head's
`ci-gate` and `lint-gate` green**, and it merges cleanly onto the chain. One that
conflicts with the chain, or whose own CI is not green, is passed over this round
and keeps its place (`watch: #N waits …`); it is never updated for the train. These
**go alone**, through the steps above: the first in line when it cannot ride (it
conflicts with `main`, even in a `sorted` list the take resolves; its own CI is not
green; Dependabot), a Markdown-only pull request as CI's `code` filter counts it
(the Markdown code reads ride; alone it merges in about a minute), a pull request touching `.github/scripts/merging/`,
`.github/workflows/`, `.github/actions/`, `tools/lint/` (its code: the layer table and the component
waivers ride, `train.group.RIDES`), `Makefile`, `merge-queue.py`, `queue-daemon.py`,
`Cargo.toml` or `Cargo.lock` (`watch: #N goes alone: it changes …`), one whose head
was in a red train, and a train that found nobody to ride with its first. A train's
take counts as one pass for those behind it in the aging count, none for its members.

The train's tip is pushed to `merge-train/<main>-<a>-<b>…` (never forced: one left
behind is deleted first) and `ci.yml` and `lint.yml` are dispatched on it
(`workflow_dispatch` runs every gating job; `lint.yml` first, so a dispatch that
fails halfway leaves only the cheap run going); the tip is checked first, as
step 2: compiled unless `--no-check`, and given `tools/lint`'s passes either way,
whose violation splits the train like a red one and hands no member back (#1956);
a checkout that fails stops the watch as a
machine failure, not a red train. Its runs are read by the same rules as a pull
request's (the grace, the runner re-run, the lagging jobs; a runner outage is
stalled, never red). The branch is deleted once they settle. Then:

- **Green:** if `main` moved while it ran, the train is stale: its members are
  rebuilt first, on the new `main`, and none is marked red. If git cannot read
  `main` (a fetch failed through its retries), nothing lands and the watch stops
  as a machine failure (exit 3). Then every member is re-read before the first
  squash: one that moved (its head, its label, draft, a description that refuses it
  now) goes back in line (`watch: #N goes back in line: …`, said as it happens) and
  the rest are rebuilt without it, nothing landed. Otherwise each member is squash-merged in order,
  exactly as step 6, with `sha` = its own head; then `main`'s tree must equal the
  train's and hold exactly that many new commits. If it does not, the watch prints
  `watch: STOPPED: main's tree is not the train's …` and exits **9**, which the
  supervisor never relaunches: look at `main` before merging anything more.
- **A member refused, or whose merge was lost in transit**, ends the landing
  there: it is handed back as today (an unknown merge says so), and every member
  after it goes back in line (`watch: #N goes back in line: …`), to be retested on
  the new `main`. One that moved in the seconds between the pre-landing read and its
  squash goes back in line the same way. What did land is checked against the tree
  the train built for it. A member whose description now refuses it (a stray
  `closes #N`) goes back in line, and its next take hands it back. GitHub answering
  "Base branch was modified" is asked again three times, then the member goes back
  in line, never handed back. GitHub failing outright mid-landing settles what
  landed, still checks `main`'s tree, and stops the watch with exit 3. If `main`
  cannot be read for that check, it is asked again for about five minutes
  (`watch.trains.CHECK_TRIES`); still unread, the watch exits 3 saying that landing
  **was not compared** with the train's tree, and the relaunched watch remembers
  nothing of it: compare `main` with the tree the stop names by hand. A poll that
  fails while the train's runs are awaited is waited past, until the deadline.
- **Red:** the train is split in half and each half is retested before any new
  train, on the latest `main`; a green half lands, and a half of one goes alone, so
  a pull request red on its own is handed back with its run linked. A flaky test
  heals on the rerun. One real break in a train of 4 costs 5 rounds for those 4
  (1 + 2·log₂ k); four breaks, 7 (2k − 1). A train that only **stalled** (no run,
  the deadline, the runners twice) splits the same way without marking anyone red,
  so its members ride again once their halves are done.

A dry run prints the train it would form and pushes nothing; `--no-merge` stops a
green train at green. A watch killed mid-train leaves a `merge-train/*` branch:
the next watch deletes every one before its first take (none can be in flight, it
holds the lock), and nothing about a train outlives the process. `main-health`
reads only `main`'s runs, so a red train files nothing.

Merging is serialized because rusty is **one compiled crate**: two branches can
each be green alone and break `main` together. A rename, a changed signature, a
moved module — no textual conflict catches any of them, and being a single crate
makes it *more* likely, not less, because everything is in scope of everything.
The queue does not change that: what merges is exactly a tree CI proved, one
tree at a time. A train only lets several pull requests share that tree.

### The Markdown exception, and the file it does not cover

CI's `changes` filter starts `code: - '!**/*.md'`. A pull request touching only
Markdown skips the heavy *steps*, but the gating jobs still **run** and report
`success` — a job whose steps all skip still succeeds. That is deliberate, and the
workflow comments say why: a ruleset reads a *skipped* required check as
unsatisfied, not as a pass, so skipping the job outright would wedge a docs-only
pull request. So a docs-only pull request needs no serialization and merges
freely.

**Three Markdown paths are code, not docs.** Hard-gate tests parse `docs/api/` (the drift, layout and
doc-example tests), `session-mcp` serves `docs/game-agent.md`, and the build
embeds `src/core/project/game_claude.md`, so a Markdown-only edit to one of them
can break `main` on its own. CI's `code` filter lists all three explicitly (#525,
#569, #1313; `merging/verdict/changes.py`'s `CODE_DOCS` is the queue's copy), so a
pull request touching one runs the tests like any code change, and it is
serialized like code.

## Verify the run happened on the head commit

    make mergeable PR=N

Exit 0 means CI genuinely ran on the head commit and passed; anything else prints
why. It is the last thing before any merge, and the queue asks it too.

**GitHub enforces the coarse half** (#491). `main`'s ruleset is code,
`.github/rulesets/main.json`: besides refusing deletion and force-pushes, it
requires `ci-gate` and `lint-gate` to pass on the pull request's head, from
GitHub Actions only, and not strict (a head behind a Markdown-only `main`
merges, #587). So GitHub itself refuses a red pull request, or one whose
gates never reported. `make ruleset` says whether the live ruleset matches the
file (read-only; exit 1 names each drift), and `make ruleset ARGS=--apply`
writes the file to GitHub. Applying changes how the queue merges, so the
orchestrator does it between merges, never a coder; a change to the rules is a
pull request on the file first.

**The ruleset does not replace `make mergeable`.** A required check is
satisfied by the newest check run of that name on the head commit, and a
draft's run leaves both gates green with nothing built. When a draft is
readied without a push, its `ready_for_review` run lists no gate until its
gated jobs finish: on 2026-10-30 #1603's `ci-gate` from the draft run was the
only one on the commit for 14 minutes, and GitHub would have merged it. Nor
does the ruleset read the description (closing keywords, `planning` issues,
open blockers). Everything below is still asked on every merge.

What it asks, so its answers read plainly (`.github/scripts/mergeable.py` has the
incidents behind each):

- **What the pull request claims, before any run** (#927, `merging/verdict/closing.py`
  and `protocol.py`). It refuses a pull request that closes a `planning` or `human`
  issue, with the labels read **at merge time** (the operator removing the label
  greenlights it, no push needed); a closing keyword pointing at a pull request, a
  closed issue or a number that does not exist; and a head branch `N-…` whose
  description neither closes #N nor says `Refs #N`. Since #966 (`carried.py`) also:
  a closing keyword anywhere but on a line of its own (quoted back); an issue it
  closes with an open recorded blocker (`blocked_by`, read at merge time); an
  issue-less pull request whose diff touches anything but Markdown, or one of the
  Markdown files code reads (`CODE_DOCS`, above), unless labelled `bug` or Dependabot's; and an image (`png`, `jpg`, `hdr`, `exr`,
  `ktx2`) the diff adds outside `IMAGES_ALLOWED`. Each is fixed by editing the
  description, a label or a relationship (a stray image by a deleting commit). A
  description with no **Gates** line passes with a warning.
- **Both workflows, on the head SHA.** `ci-gate` (from `ci.yml`) and `lint-gate`
  (from `lint.yml`) each collapse their workflow's gating jobs; a green `ci` beside
  a missing `lint` is half a check.
- **A gate that passed is not evidence anything ran.** Both gates run
  whenever the workflow was not cancelled (`!cancelled()`, #515) and pass over
  *skipped* jobs, so a draft's run is gate-green with
  nothing compiled. A run counts only when a gated job itself succeeded.
  `changes` succeeding proves nothing — it runs on drafts too.
- **Every run on the commit, not the first listed.** A red run beside a green one
  refuses. A **cancelled** run is ignored only when a run of the same workflow
  on the same commit that was *not* cancelled is still going (then the answer is
  "wait") or was created no earlier — same second included, since neither
  timestamp nor run id orders two same-second runs (#562; #482 cancels
  superseded runs). A cancelled run nothing replaced refuses, because nothing
  has answered.
- **No run at all** is told apart: a branch that conflicts with `main` gets no run
  (GitHub cannot build a merge ref, so it creates no run, check or error — do not
  edit the workflow), versus a pull request readied moments after a push that lost
  its run (force one with an empty commit). An invalid workflow *does* produce a
  run, a `startup_failure`.
- **A Markdown-only change to one of `CODE_DOCS` is refused** unless
  `build-test`'s dev-feature test step really ran. CI's `code` filter includes them
  (#525, #1313), so on a fresh run it does; the check stays as the guard.

It ignores what is not a gate: `main-health` (a `workflow_run` on `main`),
the coverage, mutation and Windows (#1395) workflows, none of which runs on a pull request
(#750). A job inside a gated run but outside its gate's `needs:` decides nothing:
a `ci` run still in progress counts as settled once `ci-gate` has concluded
(#555). Its own tests run as `make scripts`, a gate, and in `lint.yml`.

`gh pr checks N` is **not** a substitute. It has no commit column, so it cannot
answer "did this run build the head I am about to merge?", and it blends runs — a
skipped job sits in the same list as a real one, and `changes` appears **twice**,
once per workflow. To look by hand, ask about the commit:

    SHA=$(gh api repos/:owner/:repo/pulls/N --jq .head.sha)
    gh api "repos/:owner/:repo/commits/$SHA/check-runs" \
      --jq '.check_runs[] | "\(.name)\t\(.status)\t\(.conclusion)"' | sort

**Never hand-roll a "wait for CI" loop that treats zero checks as success.**
Absent and passing are different states; a loop counting non-completed checks
finds zero of each. `make queue` is that loop, written so it cannot.

*(GitHub's **native** merge queue would replace `make queue`, but it needs an
organization-owned repository, and moving the repo is the user's call, #486.
The workflows' `merge_group` triggers were removed as dead config in #930, so
adopting it means adding them back. `make mergeable` stays useful either way.)*

## A red ready pull request stays ready

Fixed in the next commit; it does not go back to draft. Draft is for work that is
genuinely unfinished, blocked, or handed over — including the hand-back after ≈3
attempts at the same failure, which does go to draft, with the reason in the
description.

## Updating a branch from `main`, and its conflicts

Expect conflicts wherever every feature appends in an order that matters:
`Entity`'s `Option<…Component>` fields, `ComponentKind`'s variant list,
`app/registry.rs`'s ordered `register` calls, the Add Component menu,
`docs/api/index.md`'s namespace list and `scripting/callbacks.rs`. Lists under a `sorted` marker (#911:
`api/mod.rs`, the Lua surface, the lint rules) mostly merge clean; when two
entries do meet, keep both, in order.

**`make queue` resolves one kind by itself** (#1723): when every conflicted hunk
of every conflicted file lies inside a list under a `sorted` marker and both
sides only added entries (no entry edited or removed, no comment or attribute
line in the hunk or directly above it, no key added twice with different text),
it keeps both sides' entries in key order, commits the merge and pushes it like
a clean one, then CI judges it. Both sides' lists being sorted makes the union
sorted; the `sorted` gate checks it where it runs (`lint.yml` skips it on a
Markdown-only pull request, and a marked file outside the rule's scan is
resolved too). The marker is the author saying in
writing that order is free there. One conflicted file outside the rule hands
the whole merge back, so a branch is never half-resolved by machine.
`Cargo.lock` never qualifies (below), and `app/registry.rs` carries no marker,
so it never does either. Everything else is handed back rather than resolved.

**Every conflict the queue meets is recorded** (#1723), once per `(main, head)`
pair, in `merge-queue/conflicts.jsonl` under the common git directory: the
pull request, its conflicted files, whether the rule could resolve each, and
whether it was resolved or handed back, written once that is known (after the
check and the push, not before). The watch prints the totals for its own
run in its closing summary; `make conflicts SINCE=24h` prints them on demand.
Read them before building any more conflict automation: they say how much
conflict work a batch carries and how much of it this rule takes.

**Two authors both being right is the common case**, and the resolution is usually
to keep both sides, ordered deliberately rather than by merge accident. One of
those needs more than that: `app/registry.rs`'s `build()` is the **only** place
per-frame system order is defined. Merging it by textual proximity silently
reorders the schedule.

Mechanical resolutions (a `mod` list, an import) are fine to do directly. The
`pre-push` hook runs rustfmt on both crates before the push leaves
(#1096); with hooks not set up, run `make resolved` first. It is rustfmt only: a lost `#[cfg(test)]` or a file pushed past its cap still waits for CI. Hand a
conflict back to the branch's author when resolving it needs to know *why* the code
is shaped as it is — a new variant that should join a documented grouping, two
prose paragraphs that need ordering, a signature that has grown a parameter, a
system whose stage placement was argued for.

**A `Cargo.lock` conflict is never hand-merged** (#824). Two lockfiles merged
hunk by hunk can resolve to a dependency graph neither side ever built. The
procedure, mid-merge of `origin/main` into the branch: take `main`'s lockfile
(`git checkout --theirs Cargo.lock`; during a merge *theirs* is `main` and *ours*
the branch, the opposite of a rebase), let cargo re-add the branch's own
edges with `cargo update -p <only the crates this branch moves>` (never a blanket
`cargo update`, which drags every other crate forward too), then `cargo check
--locked --all-targets` and `make deny` before pushing. An update that touched
`Cargo.lock` is one whose resolution needs a compile, so under `--no-check` it
goes back to a coder, never a quick orchestrator fix. On 2026-10-04 the wgpu
upgrade (#333, PR #767) was handed back three times on `Cargo.lock`; each
coder resolved it by exactly these steps, which until then lived only in their
briefs.

After an update from `main`, re-check any claim the branch made **about the base it measured
against** — the "Gates" paragraph in the description, a survivor count, a coverage
number, a byte-identical-bake proof. A measurement taken against an older `main`
is stale, and citing it is worse than not having run it. **The queue dates the
Gates line itself** (#968): each update it pushes appends (or replaces) one line
in the description, `updated by the queue to main at <main>; local gates not
re-run; CI judged <head>`, so a reader knows which commit the claim measured.

## The same-commit obligations

Four things a branch owes in the *same* change, each backed by a gate that will
otherwise fail at merge time:

- **A binding change updates its namespace's page in `docs/api/`.** Existence parity, both
  directions, dev feature set. Signatures are out of scope.
- **A new lifecycle callback updates the callback section of `docs/api/index.md`**, against
  `src/scripting/callbacks.rs`.
- **A new first-class component satisfies all four axes** — the `Entity` field,
  the Add Component entry, an inspector card, an `src/api/<x>.rs` namespace
  registered and documented — or gets a `WAIVERS` row in
  `tools/lint/src/components/waivers.rs` with a written rationale. `--components`
  discovers it from `Entity` itself, so it cannot be slipped past.
- **A migrated inspector card keeps routing through `scene::authoring`.**
  `--parity` fails on a direct field write. Nothing is grandfathered: no gate
  reads a baseline file (#926).

A new top-level `src/<dir>/` owes the coverage ratchet nothing: the step derives
its lists from `src/` and `coverage-baseline.txt` (`.github/scripts/coverage-ignore.py`,
#965), so the module is left out of every floored number the day it lands. (When
the lists were typed by hand, `audio`, `procgen`, `shadergen` and `preview` each
leaked into all of them.) A new *floor* is one line in the baseline.

rusty has no format version and no bake version to bump. The only version stamps
are the import cache's: a converter's `version` (`asset::convert::converter`; a
test holds its bump, `asset::convert::fingerprint`) and
`asset::texture_import::ENCODER_VERSION`, each bumped when its output changes so
cached files are re-made. Beyond those, what plays a version's role is the **determinism guard**: a wall-clock read
or an unseeded RNG anywhere in the sim modules (CLAUDE.md, *Determinism*)
breaks replay for every harness run, and the gate refuses it rather than letting it
land quietly.

## The signals

Mutation and coverage are **signals, never gates**. Neither can fail a build,
**neither holds a merge**, and **neither runs on a pull request** (#750): the
operator's policy is about once a week, or when someone asks, over what they
want measured.

| | on request | weekly |
|---|---|---|
| mutation | a merged PR's `Mutation: diff` (or globs) line, dispatched by the queue after the merge (below, #1934); by hand, `make mutants-remote SCOPE=diff` (or path globs): `mutants-on-request.yml`, one runner, report and survivors' diffs printed in the terminal, and the report posted as one comment on the branch's open PR, or its merged one when the queue merged it first (#912); one run per branch, so a newer dispatch cancels the older, an unmerged close of the PR cancels what is left, and a merged PR's run finishes and reports (`mutants-cancel.yml`, #1530); on `main` every run stands alone, so re-asks after a merge never cancel each other | `mutants-sweep.yml`, Saturdays, full sim in as many shards as `mutants-plan` sizes from the mutant count (#1684), `mutants-report` summary + artifact; skipped when `main` has not moved since the last finished sweep |
| coverage | `gh workflow run coverage.yml --ref <branch>` | `coverage.yml`, Mondays, ratchet table |

**Ask for a scoped mutation run on a branch that adds mechanism** — arithmetic,
a state machine, a boundary — and triage what it reports. Nothing runs it
unasked, so a branch nobody asked about is unmeasured, not clean. **The ask is a
line in the PR description, `Mutation: diff` or `Mutation: <globs>`; the merge
queue dispatches it after the merge** (#1934, `merging/after/`), never the coder
at ready: on 2026-11-08 six runs dispatched at ready held a third of the
account's runners, and every gating `ci` run waited 20-40 minutes behind them
with eight PRs in the queue. So a signal never keeps a green branch out of the
queue, nor slows the CI the queue waits on:

- **What runs:** `mutants-on-request.yml` on `main`, with the squash commit
  checked out (input `commit`). `diff` is that commit's Rust change against its
  parent (`mutants-scope.py --commit`): the branch's diff as it landed, the same
  mutants. A `diff` ask whose commit changed no Rust is not dispatched. Globs are
  passed as written. An unreadable line (a sentence, a shell character) asks for
  nothing.
- **The cap:** at most two at once (`merging/after/slots.CAP`). The rest wait in
  `merge-queue/mutants.json` beside the hand-back memory, so a relaunched watch
  keeps them; the watch dispatches the next as a slot frees, on each pass and
  each poll of a take's CI wait. A watch that ended idle leaves them waiting for
  the next watch (or the next `make queue PRS=N`, which asks for its own merges'
  runs too).
- **Where the report lands:** the merged PR, named by the run's `pr` input, as
  one comment edited in place (#912). Lines starting `after:` in the queue's log
  say each dispatch, each run's end and any refusal (a request GitHub refuses
  three times is dropped, and said). A network failure is not a refusal: the
  request keeps its place and its tries, and a merged PR whose description could
  not be read is read again on the next pass. `make queue-events` passes the
  lines that mean a request was lost (`dropped`, `could not …`, `never
  appeared`), so a lost report is told from a slow one. A named take beside the
  watch shares the file under its lock, and the cap with it.

By hand, `make mutants-remote SCOPE=...` still dispatches on your branch and
prints the report; keep that for a question you are waiting on, not for every
branch. A scoped run takes 30 minutes or more on one runner (51 mutants for
#771's diff on 2026-10-04 were still running past the half hour). The report
arrives after the branch has merged, so survivors go where *Once ready, the
branch is the queue's* sends everything found late: a comment on the issue, and
a follow-up pull request off `main` when one is worth fixing.

Three things about scope, all of which change how a clean report reads:

- **The weekly sweep only touches `src/app`, `src/scripting`, `src/physics`,
  `src/navigation`.** A survivor in `render`, `editor`, `api` or `asset` is
  reported only if someone names it in a scoped request. Silence there means
  unmeasured, not clean.
- **A scoped run names what it planned.** Its first line says how many mutants
  the scope held and whether any came from outside it. A `diff` scope with
  nothing to mutate (a branch that touched only tests) says so; it is not a pass.
- **A run stopped at its budget says what it never reached.** Those mutants are
  unmeasured. Ask again with a narrower scope.

### Triaging a survivor

Read the report when it lists survivors in code **this branch wrote**. A report
with nothing in it, or whose survivors sit in untouched code, needs no reading.

**Distrust any mutation report from before #785 for code the GPU tests reach.**
Until then neither mutation workflow installed lavapipe, so every GPU test
skipped and every mutant only they would catch read `MISSED` — `render`,
anything a capture or `Debug.Preview` reaches, editor drawing. That covers
every `mutants-on-request` run and every weekly sweep before #785 merged
(e.g. `[333-a]`, run 37182346071: 66 missed, mostly `render::readback` and
`RenderView`, all caught on lavapipe). Re-ask before acting on one. Runs
after it set `RUSTY_REQUIRE_GPU=1`, so a lost driver fails the baseline and the
run instead of quietly repeating this.

**Code behind GPU timestamps is never measured on CI.** Lavapipe has no
`TIMESTAMP_QUERY`, so the `gpu_ms` / `gpu_passes` path (#835) and the bench's
clock normalisation (#862) run only on a real GPU. Every mutant there reads
`MISSED` on any runner. Pin the pure arithmetic (exclusive time, the report
delta) with ordinary tests. Treat the rest as Metal-only: re-measure it on the
operator's Mac (`make bench`, alternating before and after) whenever it changes,
and don't file its survivors as gaps (#889).

**A capture that answers "no adapter" fails under `RUSTY_REQUIRE_GPU=1`** since
#885 (PR #899). Before that, a mutant that made `CaptureHost::draw` or
`capture_into` report no adapter turned every screenshot test into a silent
skip, and it read `MISSED` on runs that otherwise rendered.

Sort by cost:

- **Fix what is cheap** while the code is still in hand.
- **File a real bug** and fix it *after* the current branch merges.
- **File an architectural or foundational crack** — and do not start it without
  the user's judgement.
- Or **exclude it**, with a written reason, **line-qualified** so an unqualified
  entry cannot also swallow a real gap next door.

Stop the queue only for a report saying a **module** has nothing asserting its
mechanism at all. That is one finding about the tests, not a list of survivors.

**Establish equivalence by applying the mutation and running the suite** — never
by reasoning about symmetry. Reasoning has been wrong repeatedly; measurement has
not. Re-ask with a narrower scope (`make mutants-remote SCOPE=src/physics/query/through.rs`),
or reproduce a survivor locally with the same shape the job uses:

    git diff origin/main > branch.diff
    cargo mutants --test-tool nextest --in-diff branch.diff \
      -- --features dev --fail-fast --test-threads 2

That is CI's runner (#1844): nextest, a caught mutant stopping at its first
failing test, and two test processes at a time, the bound that keeps the jobs'
per-process `ulimit -v` within the runner's memory.

`cargo mutants --list` names the functions and builds nothing — reach for it before
believing a structural explanation of a bad report.

Two shapes worth recognising before triaging:

**A measurement that discards sign cannot test an operation that changes it.**
`Vec3::length`, a distance, a magnitude, a squared term, a count, an absolute
value, a `Quat` dot — every one of them reads like a real assertion and every one
is blind to a `+` → `-`. In a physics and navigation sim that is most of what the
tests assert on.

**A boundary comparison surviving both `<=` and `>=` means neither end is
exercised** — a fixed-timestep accumulator that never lands exactly on a step, a
nav agent exactly on its arrival radius, a contact exactly at the epsilon, a
trigger overlap beginning and ending on the same tick. Each of those is a real gap
until measured otherwise.

Two caveats that are the reason this is not a gate: `--in-diff` line matching can
drift after an update from `main`, and a mutant that induces a slow loop times out and reports
as a survivor. Neither should redden CI, and neither should stop a merge.

### The coverage floors

`coverage-baseline.txt` holds per-module line floors for `app`, `scripting`,
`physics`, `navigation` and `api`. It is a **ratchet, not a gate**: the job that
reads it runs weekly (`coverage.yml`) and prints a table flagging a drop
without ever exiting non-zero. Raise a floor as coverage improves;
**never lower one silently to make a table green** — that is the one move it
exists to make visible.

The platform layer (`shell`, `render`, `dev`) is deliberately unfloored. GPU
tests run on all three CI OSes — Linux through Mesa's **lavapipe** software
Vulkan driver (#489) — and `RUSTY_REQUIRE_GPU=1` fails the run if an adapter
ever goes missing, so a green run means they rendered. Two of the three
adapters are software (lavapipe, WARP): a change that depends on real-GPU
behaviour is proven on macOS's Metal, not by a green Linux job alone.
