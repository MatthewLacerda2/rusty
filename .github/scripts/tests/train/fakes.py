"""A [`watch.trains.Yard`]'s world as a script: git, GitHub and the clock faked.

[`git`] answers `merge-tree` and `commit-tree` the way [`build.couple`] reads
them: a head in `clashing` conflicts with any tip, every other merge is clean,
and the tree after head `h` on tip `t` is named `t+h`, so a train's tree spells
its members in order. [`Fx`] is the rest.
"""

from __future__ import annotations

import argparse

from merging.queue import outcomes, pulls
from merging.train import build, ride
from merging.watch import board, trains


def git(clashing=()):
    def run(*args):
        args = [a for a in args if a not in build.IDENTITY]
        if args[0] == "merge-tree":
            tip, head = args[-2], args[-1]
            return (1, "") if head in clashing else (0, f"{tip}+{head}\n")
        if args[0] == "commit-tree":
            return 0, f"{args[1]}\n"  # the commit is named after its tree
        if args[0] == "rev-parse":
            return 0, "main\n"
        return 0, ""
    return run


def pr(number: int, head=None, files=("src/a.rs",)) -> dict:
    return {
        "number": number, "headRefOid": head or f"h{number}", "author": {"login": "x"}, "state": "OPEN",
        "isDraft": False, "labels": [{"name": board.QUEUE_LABEL}], "files": [{"path": f} for f in files],
    }


class Fx:
    """The effects, each recorded or scripted. `verdicts` is what each train's
    wait ends in, in turn; `squashes[n]` what merging pull request `n` ends in."""

    def __init__(self, pulls_=(), clashing=(), red_own=(), verdicts=(outcomes.GO,)):
        self.by = {p["number"]: p for p in pulls_}
        self.verdicts, self.squashes, self.merged, self.said = list(verdicts), {}, [], []
        self.pushed, self.deleted, self.main_moved, self.tree_override = [], [], 0, None
        self.red_own, self.broken = set(red_own), []
        self.ns = argparse.Namespace(
            start=lambda: build.Train("main", "main"), git=git(clashing),
            files=lambda n, h: [f["path"] for f in self.by[n]["files"]],
            green=lambda n, h: n not in self.red_own, claims=lambda n: None, check=lambda sha: self.broken,
            push=lambda t: self.pushed.append(t.numbers), dispatch=lambda name: None, wait=self.wait,
            delete=self.deleted.append, look=lambda n: self.by[n], squash=self.squash,
            main_state=self.main_state, say=lambda *a: self.said.append(a[0]), bots=pulls.BOTS,
            dry_run=False, no_merge=False,
        )
        self.trains: list[list[int]] = []

    def yard(self) -> trains.Yard:
        return trains.Yard(self.ns)

    def wait(self, sha, tick):
        verdict = self.verdicts.pop(0) if len(self.verdicts) > 1 else self.verdicts[0]
        return verdict, [f"the train {sha} is {verdict}."]

    def squash(self, pull, sha):
        state = self.squashes.get(pull["number"], outcomes.MERGED)
        if state == outcomes.MERGED:
            self.merged.append(pull["number"])
        return state, f"{state}."

    def main_state(self, base):
        tree = "main" + "".join(f"+h{n}" for n in self.merged)
        return self.tree_override or tree, len(self.merged) + self.main_moved


RED = ride.RED
