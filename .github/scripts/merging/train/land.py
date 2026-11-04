"""Landing a green train: each member in order, then `main`'s tree against the train's ([`land`], [`check`]).

Each member is merged through the queue's own REST call ([`queue.landing.squash`]:
`sha` = its head, its own title and message, the closing keywords in the title,
#966). One that moved since the train formed, or that GitHub refuses, ends the
landing there: the members after it are untested without it, so they go back in
line and are retested on the new `main` ([`outcomes.REQUEUED`]). A call that failed
in transit is asked about ([`queue.landing.confirm`]): merged, it counts; never
said merged, it is handed back as *unknown*, as today, and the rest go back in line.

Then [`check`]: `main` must hold exactly the members merged, one squash commit
each, and the tree the train built for that many ([`build.Train.tree_after`]). A
merge of unknown fate allows one more. Anything else stops the watch
([`MISMATCH`]): it must never pass silently.
"""

from __future__ import annotations

from merging.queue import landing, outcomes
from merging.train import build
from merging.watch import board


MISMATCH = "main's tree is not the train's"


def moved(pull: dict, car: build.Car) -> str | None:
    """Why `pull` may no longer land with its train, or `None`."""
    if pull.get("state") != "OPEN":
        return f"it is {str(pull.get('state')).lower()} now"
    if pull.get("isDraft"):
        return "it went back to draft"
    if not board.queued(pull):
        return f"it lost the `{board.QUEUE_LABEL}` label"
    if pull.get("headRefOid") != car.head:
        return f"its head moved to {str(pull.get('headRefOid'))[:7]} after the train tested {car.head[:7]}"
    return None


def land(train: build.Train, look, squash) -> tuple[list[tuple[int, str, str]], int, bool]:
    """Merge the members in order: the results, how many landed, and whether
    the last one tried has an unknown fate. `look(number)` is the pull request
    now; `squash(pull, sha)` is [`queue.landing.squash`]'s `(state, why)`."""
    results, landed, unknown = [], 0, False
    for at, car in enumerate(train.cars):
        pull = look(car.number)
        why = moved(pull, car)
        if why:
            results.append((car.number, outcomes.REQUEUED, f"not landed with its train: {why}."))
        else:
            state, why = squash(pull, car.head)
            if state == outcomes.MERGED:
                results.append((car.number, outcomes.MERGED, f"landed with the train {train.tip[:7]}."))
                landed += 1
                continue
            unknown = state == landing.UNKNOWN
            results.append((car.number, outcomes.HANDED_BACK, why))
        results += [
            (later.number, outcomes.REQUEUED, f"not landed: #{car.number} did not land before it, so its train is untested without it.")
            for later in train.cars[at + 1:]
        ]
        break
    return results, landed, unknown


def check(train: build.Train, landed: int, unknown: bool, main_tree: str, squashes: int | None) -> str | None:
    """Why `main` is not what landing `landed` members of `train` must leave, or `None`.

    `main_tree` is `origin/main^{tree}` fetched after the landing; `squashes`, the
    first-parent commits from the train's base to it (`None`: git could not say).
    """
    counts = {landed, landed + 1} if unknown else {landed}
    trees = {train.tree_after(n) for n in counts if n <= len(train.cars)}
    if squashes not in counts:
        return (
            f"{MISMATCH}: {squashes} commit(s) landed on `main` since the train's base {train.base[:7]},"
            f" not the {landed} it merged; something else merged while it landed, and the tree it proved is not the tree `main` holds."
        )
    if main_tree not in trees:
        return (
            f"{MISMATCH}: after merging {', '.join(f'#{n}' for n in train.numbers[:landed])}, `main`'s tree is"
            f" {main_tree[:12]}, not the {train.tree_after(landed)[:12]} the train built. GitHub's merge and git's disagree;"
            " look at `main` before merging anything more."
        )
    return None
