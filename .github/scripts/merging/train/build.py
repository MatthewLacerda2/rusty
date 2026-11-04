"""The train's tree: a chain of squash commits on `main`, one per member ([`couple`], [`Train`])."""

from __future__ import annotations

from dataclasses import dataclass, field


# Who the train's throwaway commits are by: they never reach `main` (the squash
# merges do), so only `git commit-tree` reads it, on a machine without an identity.
IDENTITY = ("-c", "user.name=rusty merge queue", "-c", "user.email=merge-queue@localhost")


@dataclass(frozen=True)
class Car:
    """One member: its number and head, and the chain's tree and commit once it is on."""

    number: int
    head: str
    tree: str
    commit: str


@dataclass
class Train:
    """The members on `base` (`origin/main`), in landing order."""

    base: str
    base_tree: str
    cars: list[Car] = field(default_factory=list)

    @property
    def tip(self) -> str:
        return self.cars[-1].commit if self.cars else self.base

    @property
    def tree(self) -> str:
        return self.cars[-1].tree if self.cars else self.base_tree

    @property
    def numbers(self) -> list[int]:
        return [car.number for car in self.cars]

    def tree_after(self, landed: int) -> str:
        """The tree `main` holds once the first `landed` members are merged."""
        return self.cars[landed - 1].tree if landed else self.base_tree


def couple(train: Train, number: int, head: str, git) -> Car | None:
    """`head` merged onto the train's tip, as GitHub's squash would land it, or
    `None` when it conflicts or git cannot say: a car the train is not sure of
    stays off it (fail closed, unlike the order's fail-open count).

    `git(*args)` returns `(status, stdout)`. The tip descends from `main` by
    single-parent commits only, so the merge base `merge-tree` finds is the one
    GitHub's squash of `head` onto `main` uses.
    """
    status, out = git("merge-tree", "--write-tree", "--no-messages", train.tip, head)
    tree = out.split("\n", 1)[0].strip() if status == 0 else ""
    if not tree:
        return None
    status, out = git(*IDENTITY, "commit-tree", tree, "-p", train.tip, "-m", f"merge train: #{number} at {head[:12]}")
    commit = out.strip() if status == 0 else ""
    return Car(number, head, tree, commit) if commit else None


def start(git) -> Train | None:
    """An empty train on `origin/main` (fetched by the caller), `None` when git cannot say."""
    status, base = git("rev-parse", "--verify", "--quiet", "origin/main^{commit}")
    status_tree, tree = git("rev-parse", "--verify", "--quiet", "origin/main^{tree}")
    if status != 0 or status_tree != 0 or not base.strip() or not tree.strip():
        return None
    return Train(base.strip(), tree.strip())
