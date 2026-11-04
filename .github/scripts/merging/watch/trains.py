"""The watch's merge trains across passes ([`Yard`], #1785): the halves waiting to be tested, and who was red in one.

[`Yard.form`] decides each round: a half of a red train first, else a new train
walked from the head of the line ([`train.group.form`]). [`Yard.ride`] runs one:
the local compile check unless `--no-check`, the push to its `merge-train/*`
branch, the gates dispatched on it, the wait, the branch deleted, then landing
([`train.land`]) or splitting. A dry run prints the train it would form and pushes
nothing; `--no-merge` stops a green train at green. What the members' own claims
say (`verdict.protocol`, the review decision) is asked before a pull request may
ride, through the same [`queue.taking.claims`] a take asks first.
"""

from __future__ import annotations

from merging.queue import outcomes, stopping
from merging.train import build, group, land, rails, ride
from merging.verdict import github
from merging.watch import board


def halves(train: build.Train) -> list[list[tuple[int, str]]]:
    """A red train's two halves, each `(number, head)` in landing order."""
    cars = [(car.number, car.head) for car in train.cars]
    middle = len(cars) // 2
    return [cars[:middle], cars[middle:]]


class Yard:
    """The trains of one watch. `fx` carries the effects: `start()` (an empty
    [`build.Train`] on a fresh `origin/main`, or `None`), `git(*args)`, `files(number,
    head)` (the paths it changes, or `None`), `green(number, head)` (whether its own
    head's gates passed), `claims(number)` (a hand-back result its description or
    review refuses it with, or `None`), `check(sha)` (why the train's tree does not
    compile, empty when it does or `--no-check`), `push(train)`, `dispatch(branch)`,
    `wait(sha, tick)` ([`train.ride.wait`]), `delete(branch)`, `look(number)`,
    `squash(pull, sha)`, `main_state(base)` (`main`'s tree and its first-parent count
    since `base`), `say`, `bots`, `dry_run` and `no_merge`."""

    def __init__(self, fx):
        self.fx = fx
        self.waiting: list[list[tuple[int, str]]] = []  # halves, tested before any new train
        self.red: dict[int, str] = {}  # the head each pull request had in a red train

    def refuse(self, pull: dict) -> str | None:
        """Why `pull` may not ride a new train, or `None`. Cheapest question first."""
        number, head = pull["number"], pull.get("headRefOid", "")
        if board.is_bot(pull, self.fx.bots):
            return "Dependabot rebases its own branch"
        if self.red.get(number) == head:
            return "this head was in a red train"
        files = self.fx.files(number, head)
        if files is None:
            return "GitHub did not list its files"
        why = group.alone(files)
        if why:
            return why
        if not self.fx.green(number, head):
            return "its own head's `ci-gate` and `lint-gate` are not both green"
        refused = self.fx.claims(number)
        return refused[2].rstrip(".") if refused else None

    def form(self, ranked: list[dict]) -> tuple[list[dict], build.Train | None, list[str]]:
        """This round's members and their train (`None`: the first goes alone), and the notes."""
        train = self.fx.start()
        if train is None:
            return [ranked[0]], None, ["watch: git could not read `origin/main`; taking the first alone."]
        by = {p["number"]: p for p in ranked}
        while self.waiting:
            half = [by[n] for n, head in self.waiting.pop(0) if n in by and by[n].get("headRefOid") == head]
            if half:
                return group.form(half, train, self.fx.git, lambda pull: None, cap=len(half))
        return group.form(ranked, train, self.fx.git, self.refuse)

    def ride(self, train: build.Train, tick=None) -> tuple[list[tuple[int, str, str]], str | None]:
        """Test and land (or split) one train: the members' results, and why the
        watch must stop ([`train.land.MISMATCH`]), or `None`."""
        name, fx = rails.branch(train), self.fx
        listed = ", ".join(f"#{n}" for n in train.numbers)
        if fx.dry_run:
            fx.say(f"watch: dry run: would test {listed} together on {name} (tree {train.tree[:12]}), and land them if it is green.")
            return [(n, outcomes.DRY, f"would ride {name}.") for n in train.numbers], None
        try:
            state, lines = self.test(train, name, tick)
        except (github.Unreachable, stopping.Stopped) as down:
            why = down.lines[0] if isinstance(down, stopping.Stopped) else stopping.unreachable(down)[0]
            return [(train.numbers[0], outcomes.STOPPED, why)], None
        fx.say(f"watch: train {listed}: {lines[0]}", *lines[1:])
        if state == outcomes.STOPPED:
            return [(train.numbers[0], outcomes.STOPPED, lines[0])], None
        if state != outcomes.GO:
            if state == ride.RED:
                self.red.update((car.number, car.head) for car in train.cars)
            self.waiting[:0] = halves(train)
            return [(n, outcomes.REQUEUED, f"its train was {state}; its half is tested next.") for n in train.numbers], None
        if fx.no_merge:
            return [(n, outcomes.GREEN, f"its train {train.tip[:7]} is green.") for n in train.numbers], None
        results, landed, unknown = land.land(train, fx.look, fx.squash)
        if landed or unknown:
            halt = land.check(train, landed, unknown, *fx.main_state(train.base))
            if halt:
                fx.say(f"watch: STOPPED: {halt}")
                return results, halt
        return results, None

    def test(self, train: build.Train, name: str, tick) -> tuple[str, list[str]]:
        """The local check, the push, the dispatch and the wait; the branch is deleted after."""
        broken = self.fx.check(train.tip)
        if broken:
            return ride.RED, [f"the train's tree does not pass the local checks: {broken[0]}", *broken[1:]]
        refused = self.fx.push(train) or None
        if refused:
            return outcomes.STOPPED, [refused]
        try:
            refused = self.fx.dispatch(name)
            if refused:
                return outcomes.STOPPED, [refused]
            self.fx.say(f"watch: testing {', '.join(f'#{n}' for n in train.numbers)} together on {name} ({train.tip[:7]}).")
            return self.fx.wait(train.tip, tick)
        finally:
            self.fx.delete(name)
