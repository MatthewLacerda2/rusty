"""The merge train: test up to [`group.CAP`] pull requests on one tree, land them all if it is green (#1785).

## Why

The queue merged one pull request per CI round, about 25 minutes each (the median
of 12 merges on 2026-11-03/04), and with coding in the cloud the line grew faster
than that: 11 in line on 2026-11-04, over four hours of merging, and every merge
ahead made a wide branch conflict again (PR #1766 was handed back twice in one
night). The operator approved merging in groups on 2026-11-04.

## What keeps it sound

**What merges is exactly the tree CI proved.** The train is built the way GitHub
will land it: a chain of single-parent commits on `origin/main`, one per pull
request, each the tree `git merge-tree` writes for that head on the chain so far
([`build.couple`]). Because the chain carries no commit of any branch, each merge
base is the one GitHub's squash merge will use, so the chain's tip is the tree
`main` will hold once every member is squash-merged in order. CI is dispatched on
that tip ([`rails`]); a green verdict lands the members, each through today's REST
merge with `sha` = its own head ([`land`]); then `main`'s tree is compared with the
train's. A difference stops the watch ([`land.MISMATCH`], exit 9): it must never pass
silently. Nothing here asks a model anything: every rule is one the queue reads.

**Nothing is pushed to a pull request's own head.** A member must already merge
cleanly onto `main` and the chain so far, so GitHub's squash needs no update. A
conflict, even one the sorted-list resolver (#1723) would settle, keeps that pull
request out of the train: its resolution would have to be pushed to its head, so
it takes today's path alone ([`group.form`]), which merges `main` in, resolves,
pushes and waits.

## Red, and the worst case

A red train is split in half and each half is tested on its own, on top of the
latest `main` (the watch keeps the halves, [`watch.trains.Yard`]); a half that is
green lands, and a half of one is today's path, so a pull request red on its own is
handed back as today, its run linked. A flaky test heals on the rerun of its half.
With one real break in a train of k the line pays 1 + 2·log₂(k) rounds for those k
pull requests (5 for k = 4, against 4 one at a time); with every member broken,
2k − 1 (7 for k = 4). A pull request that was in a red train goes alone afterwards,
until its head moves.

## A restart

One train at a time, and only inside one take: the watch checks its own code,
its deadline and the stop file between takes, never during one. A watch killed
mid-train leaves a `merge-train/*` branch behind and maybe a run on it; the next
watch deletes every such branch before its first take ([`rails.sweep`]), since one
watch at a time holds the lock and none of them is anybody's. Killed mid-landing,
some members are merged and the rest are back in line on the new `main`, retested
there. No state about a train outlives the process.
"""
