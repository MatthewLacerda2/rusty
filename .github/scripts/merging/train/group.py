"""Who rides together ([`form`]), and who goes alone ([`alone`]).

The line is walked in the watch's own rank ([`watch.order.line`]: the aged first,
fewest conflicts, label), each candidate merged onto the train so far
([`build.couple`]). One that conflicts with it is passed over for this round and
keeps its place, so aging still counts it (#928). The first in line sets the
round: if it cannot ride, it is taken alone, through today's path, and so is a
train that found nobody to ride with it ("a group of one is just today's path").
The one exception (#2285) is a first refused only because its own CI is still
running ([`Unfinished`], the kind [`NOT_GREEN`] is): unless aging put it first,
it is passed over like a conflicting one, and the round takes those behind it;
it is taken alone only when nobody behind it can ride. A first whose own CI is
red goes alone, so today's path hands it back at once.

## The measurement behind the two #2285 rules

Over the queue log from 2026-11-04 (when trains began) to 2026-11-17, the queue
took 28 trains (85 pull requests) and 271 single takes:

| single take | count |
|---|---|
| the first in line refused (`goes alone`) | 232 |
| rideable, nobody else in line | 28 |
| rideable, others in line but all refused | 10 |
| a red train's half | 1 |

Of the 232, 127 were a first whose own `ci-gate` and `lint-gate` were not both
green, which set the round to a single take without walking the line behind it:
hence the pass-over. 28 were a manifest (24 `Cargo.lock`, 4 `Cargo.toml`): hence
a manifest change rides. At most one rides a train, held by
[`watch.board.lock_waits`], which keeps every manifest pull request but the
oldest out of the line, so a train never meets two. The take line names the line's size
and how many could ride (`watch.loop`), so the next measurement can say how many
single takes had a partner.
"""

from __future__ import annotations

from merging.train import build
from merging.verdict import changes


# The most pull requests one train carries. Measured 2026-11-04 over the 140
# `ci` runs of pull requests on 2026-11-03/04: 7 red, so p ≈ 0.05 a run (`lint`:
# none red). With a red half bisected, the expected CI rounds per pull request
# are 0.60 at k = 2, 0.44 at k = 4 and 0.40 at k = 8 for p = 0.05, and 0.69,
# 0.61 and 0.63 for p = 0.10: past 4 the gain is a few hundredths while a red
# train sends twice as many back through the halves, and the line of 11 that
# motivated this clears in three rounds at 4.
CAP = 4

# What a train's members may not touch (#1785): the merge path and CI, because the
# watch relaunches itself on its own code (`watch.freshness`) and a train would
# prove a workflow by running the other one.
# `tools/lint/` is the gate's own code, so it goes alone, but not the tables in it
# that feature branches edit as data (#1785's review: 2 of 9 ready pull requests
# on 2026-11-04 touched only these): the train's tree proves them like any file.
ALONE_PREFIXES = (".github/actions/", ".github/scripts/merging/", ".github/workflows/", "tools/lint/")
ALONE_FILES = ("Makefile", ".github/scripts/merge-queue.py", ".github/scripts/queue-daemon.py")
RIDES = ("tools/lint/src/layers/table.rs", "tools/lint/src/components/waivers.rs")

class Unfinished(str):
    """A refusal that passes the first in line over instead of taking it alone
    (#2285): the kind is what [`form`] keys on, never the words."""


# Why a pull request whose own head's CI is still running may not ride ([`watch.trains.Yard.refuse`]).
NOT_GREEN = Unfinished("its own head's `ci-gate` and `lint-gate` are not both green yet")


def alone(files: list[str]) -> str | None:
    """Why a change to `files` must take its turn alone, or `None`.

    A pull request CI's `code` filter counts as Markdown-only (every path
    Markdown, none a doc the tests read: [`verdict.changes.inert`], the same list)
    goes alone too: alone it merges in about a minute with its build steps
    skipped, where a train costs it a `cargo check` and a full CI round, and a
    broken code member could drag it through bisection (#1785's review)."""
    if files and changes.inert(files):
        return "it is Markdown-only, which merges in minutes alone"
    hit = next((f for f in files if f not in RIDES and (f in ALONE_FILES or f.startswith(ALONE_PREFIXES))), None)
    return f"it changes {hit}, which goes alone" if hit else None


def form(
    line: list[dict], train: build.Train, git, refuse, cap: int = CAP,
    aged: set[int] | frozenset[int] = frozenset(),
) -> tuple[list[dict], build.Train | None, list[str]]:
    """The members of this round, the train they ride (`None`: one goes alone,
    today's path), and one note per pull request passed over.

    `line` is the ready pull requests in rank order; `train` is empty, on the
    `main` to land on; `refuse(pull)` says why a pull request may not ride
    ([`alone`], [`NOT_GREEN`], red in a train before), or `None`; `aged` holds
    those aging put first ([`watch.aging.Line.aged`]).
    """
    first, notes, members = line[0], [], []
    held = refuse(first)
    passed = isinstance(held, Unfinished) and first["number"] not in aged
    if held and not passed:
        return [first], None, [f"watch: #{first['number']} goes alone: {held}."]
    for pull in line:
        if len(members) == cap:
            break
        number = pull["number"]
        if pull is first and passed:
            continue
        why = refuse(pull) if pull is not first else None
        if why:
            notes.append(f"watch: #{number} waits for a train of its own: {why}.")
            continue
        car = build.couple(train, number, pull.get("headRefOid", ""), git)
        if car is None:
            if not members and not passed:
                return [first], None, [f"watch: #{number} goes alone: it does not merge cleanly onto `main`."]
            clash = f"it conflicts with the train so far ({', '.join(f'#{n}' for n in train.numbers)})" if members else "it does not merge cleanly onto `main`"
            notes.append(f"watch: #{number} waits: {clash}.")
            continue
        train.cars.append(car)
        members.append(pull)
    if passed and not members:
        return [first], None, [*notes, f"watch: #{first['number']} goes alone: {held}, and nobody behind it can ride."]
    if passed:
        riders = ", ".join(f"#{p['number']}" for p in members)
        taken = f"{riders} is taken alone" if len(members) == 1 else f"{riders} ride without it"
        notes.insert(0, f"watch: #{first['number']} is passed over this round: {held}; it keeps its place, and {taken}.")
    return members, (train if len(members) > 1 else None), notes
