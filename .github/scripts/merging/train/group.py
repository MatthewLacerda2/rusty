"""Who rides together ([`form`]), and who goes alone ([`alone`]).

The line is walked in the watch's own rank ([`watch.order.line`]: the aged first,
fewest conflicts, label), each candidate merged onto the train so far
([`build.couple`]). One that conflicts with it is passed over for this round and
keeps its place, so aging still counts it (#928). The first in line sets the
round: if it cannot ride, it is taken alone, through today's path, and so is a
train that found nobody to ride with it ("a group of one is just today's path").
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
# prove a workflow by running the other one; the manifests, because two members
# resolving dependencies apart is exactly what a textual merge cannot see.
# `tools/lint/` is the gate's own code, so it goes alone, but not the tables in it
# that feature branches edit as data (#1785's review: 2 of 9 ready pull requests
# on 2026-11-04 touched only these): the train's tree proves them like any file.
ALONE_PREFIXES = (".github/actions/", ".github/scripts/merging/", ".github/workflows/", "tools/lint/")
ALONE_FILES = (
    "Makefile", "Cargo.toml", "Cargo.lock",
    ".github/scripts/merge-queue.py", ".github/scripts/queue-daemon.py",
)
RIDES = ("tools/lint/src/layers/table.rs", "tools/lint/src/components/waivers.rs")


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


def form(line: list[dict], train: build.Train, git, refuse, cap: int = CAP) -> tuple[list[dict], build.Train | None, list[str]]:
    """The members of this round, the train they ride (`None`: the first goes
    alone, today's path), and one note per pull request passed over.

    `line` is the ready pull requests in rank order; `train` is empty, on the
    `main` to land on; `refuse(pull)` says why a pull request may not ride
    ([`alone`], its own CI not green, red in a train before), or `None`.
    """
    first, notes, members = line[0], [], []
    why = refuse(first)
    if why:
        return [first], None, [f"watch: #{first['number']} goes alone: {why}."]
    for pull in line:
        if len(members) == cap:
            break
        number = pull["number"]
        why = refuse(pull) if members else None
        if why:
            notes.append(f"watch: #{number} waits for a train of its own: {why}.")
            continue
        car = build.couple(train, number, pull.get("headRefOid", ""), git)
        if car is None:
            if not members:
                return [first], None, [f"watch: #{number} goes alone: it does not merge cleanly onto `main`."]
            notes.append(f"watch: #{number} waits: it conflicts with the train so far ({', '.join(f'#{n}' for n in train.numbers)}).")
            continue
        train.cars.append(car)
        members.append(pull)
    if len(members) == 1:
        return [first], None, notes
    return members, train, notes
