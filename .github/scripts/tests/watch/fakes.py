"""GitHub and the clock as a script, for driving [`loop.run`] without a network.

[`World`] returns its listings in turn and ends each take in the verdict it
was given; [`pr`] is one open pull request as `gh pr list` reports it.
"""

from __future__ import annotations

import argparse
from pathlib import Path

from merging.queue import cli, outcomes, pulls
from merging.watch import board, handbacks


def pr(number: int, draft=False, head=None, queued=True) -> dict:
    labels = [{"name": board.QUEUE_LABEL}] if queued else []
    return {"number": number, "isDraft": draft, "headRefOid": head or f"sha{number}", "author": {"login": "x"}, "labels": labels}


class World:
    """GitHub as a script: `listings` are returned in turn (the last repeats),
    and `verdicts[n]` is what taking pull request `n` ends in."""

    def __init__(self, tmp: str, listings: list, verdicts: dict[int, str]):
        self.listings, self.verdicts, self.now, self.taken = listings, verdicts, 0.0, []
        self.clashes: set[frozenset[int]] = set()
        self.fx = argparse.Namespace(
            pulls=self.pulls, issue_labels=dict, clashes=lambda pulls: self.clashes,
            turn=self.turn, head=lambda n: f"pushed{n}", attempts=lambda n, sha: "",
            clock=lambda: self.now, sleep=self.sleep, say=lambda *a: None,
            memory=Path(tmp, handbacks.MEMORY), bots=pulls.BOTS, merged=outcomes.MERGED,
            ends={outcomes.GREEN, outcomes.DRY}, stops={outcomes.STOPPED}, skips={outcomes.NOT_READY},
        )

    def pulls(self):
        listing = self.listings[0] if len(self.listings) == 1 else self.listings.pop(0)
        return [p for p in listing if p["number"] not in self.merged()] if listing is not None else None

    def merged(self):
        return {n for n, s in zip(self.taken, (self.verdicts[n] for n in self.taken)) if s == outcomes.MERGED}

    def turn(self, number):
        self.taken.append(number)
        return number, self.verdicts[number], "why"

    def sleep(self, seconds):
        self.now += seconds


def opts(*flags, **kw):
    return cli.parse(["--watch", "--no-check", *flags, *(f"--{k}={v}" for k, v in kw.items())])
