"""The merge queue: do the waiting that serialized merging costs.

Merging stays serialized, one branch at a time, because rusty is one compiled
crate: two pull requests can each be green alone and break `main` together (a
rename, a changed signature, a moved module). Nothing here proposes otherwise.
What this automates is **who sits through the ten minutes** — merge `main`
into the branch ([`update.advance`], #1568: never a rebase, so the push is a
fast-forward, leased on the head it took), compile-check the updated tree ([`checks.verify`]), push it, wait
for the runs on the updated head, ask
[`verdict.decide.judge`], squash-merge, take the next one. Without it an agent sits
through every cold CI run of a batch holding a worktree open. Ported from
scorsese (#486); its incidents are cited as scorsese#N.

The command line, its flags and how to run it are in `merge-queue.py`.

## Why it does not skip a run instead

scorsese#492 weighed a cheaper answer — skip the fresh run when the rebase
(the queue rebased then) "changed nothing that matters". A run's verdict is
about a **tree**, and it carries only to the same tree; "main's delta touched
no module the branch did" is not that, and in one crate it is almost never even true. Measured over the
batch that motivated it, 23 re-runs, 0 of them on an already-tested tree. So
the one sound skip is kept: **an update that moves nothing** — the branch
already contains `main`'s tip, the run on record is a run on this exact commit
([`update.push_needed`]).

**And one that could move nothing that compiles.** When every commit `main`
gained since the branch's base touches only Markdown — [`verdict.changes.inert`],
so never `docs/api/`, which tests parse — the head is left where
it is: no merge, no push, and the run on record stands ([`update.docs_advance`],
#587). That is not the per-module guess above: those commits cannot change a
single compiled or tested byte, which is why CLAUDE.md lets them merge
unserialized. On 2026-09-30 merging the Markdown-only #586 cost #584, waiting
behind it, a rebase and a ten-minute CI round. The squash merge still lands on
the newer `main`; a branch editing the same `.md` file gets GitHub's merge
conflict, handed back as any refused merge is.

## The three things it must not do

1. **It never resolves a conflict.** Empty conflict set, or it hands the branch
   back naming the paths. rusty's conflict hot spots — `ComponentKind::ALL`'s
   hard-coded length, `app/registry.rs`'s system order — are exactly the ones
   where keeping both sides is wrong and no textual merge knows it.
2. **It never merges on a local result.** A local check may only *refuse* a
   push, never grant a merge. `make gates` is the author's job before
   readying; CI is the cross-platform claim (the macOS runner), and the only
   thing a merge consults is [`verdict.decide.judge`].
3. **It never reads the mutation or coverage signal.** Signals never hold a
   merge.
"""
