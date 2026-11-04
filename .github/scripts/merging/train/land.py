"""Landing a green train: each member in order, then `main`'s tree against the train's ([`land`], [`check`]).

Each member is merged through the queue's own REST call ([`queue.landing.squash`]:
`sha` = its head, its own title and message, the closing keywords in the title,
#966). One that moved since the train formed, or that GitHub refuses, ends the
landing there: the members after it are untested without it, so they go back in
line and are retested on the new `main` ([`outcomes.REQUEUED`]). A call that failed
in transit is asked about ([`queue.landing.confirm`]): merged, it counts; never
said merged, it is handed back as *unknown*, as today, and the rest go back in line.

GitHub can answer "Base branch was modified" to a merge call made right after
the one before it (#1785's review): [`queue.landing.squash`] asks again a few
times, and one still refused that way is no fault of the member's, so it and the
rest go back in line ([`outcomes.REQUEUED`]), never handed back.

GitHub failing outright partway through (a `look` that runs out of retries,
[`verdict.github.Unreachable`], or a `gh` refusal that `sys.exit`s, #1785's
review: #1 merged, then `look(2)` raised and the watch died with a traceback,
recording nothing) ends the landing too: what landed is landed and settled, the
member it failed on is a machine stop ([`outcomes.STOPPED`]), the rest go back
in line, and [`check`] still runs.

Then [`check`]: `main` must hold exactly the members merged, one squash commit
each, and the tree the train built for that many ([`build.Train.tree_after`]). A
merge of unknown fate allows one more. Anything else stops the watch
([`MISMATCH`]): it must never pass silently.
"""

from __future__ import annotations

from merging.queue import landing, outcomes, stopping
from merging.train import build
from merging.verdict import github, protocol
from merging.watch import board


MISMATCH = "main's tree is not the train's"

# GitHub failing during a landing: out of retries, or a `gh` refusal `verdict.github.gh` exits on.
DOWN = (github.Unreachable, SystemExit)


def down_why(down: BaseException) -> str:
    """One line for a [`DOWN`] failure."""
    if isinstance(down, github.Unreachable):
        return stopping.unreachable(down)[0]
    return f"GitHub refused a call: {down.code if isinstance(down, SystemExit) else down}"


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
    # What it claims, re-read now: a stray `closes #N` added while the train ran
    # would close the wrong issue (#777, #812), as the take's own wait re-judges.
    refused = protocol.refusal(pull)
    if refused:
        return f"its description refuses it now: {refused[0]}"
    return None


def land(train: build.Train, look, squash) -> tuple[list[tuple[int, str, str]], int, bool]:
    """Merge the members in order: the results, how many landed, and whether
    the last one tried has an unknown fate. `look(number)` is the pull request
    now; `squash(pull, sha)` is [`queue.landing.squash`]'s `(state, why)`."""
    results, landed, unknown = [], 0, False
    for at, car in enumerate(train.cars):
        state, why, unknown = step(car, look, squash)
        if state == outcomes.MERGED:
            results.append((car.number, outcomes.MERGED, f"landed with the train {train.tip[:7]}."))
            landed += 1
            continue
        results.append((car.number, state, why))
        results += [
            (later.number, outcomes.REQUEUED, f"not landed: #{car.number} did not land before it, so its train is untested without it.")
            for later in train.cars[at + 1:]
        ]
        break
    return results, landed, unknown


def step(car: build.Car, look, squash) -> tuple[str, str, bool]:
    """One member's landing: its state, why, and whether its fate is unknown."""
    try:
        pull = look(car.number)
    except DOWN as down:
        return outcomes.STOPPED, down_why(down), False
    why = moved(pull, car)
    if why:
        return outcomes.REQUEUED, f"not landed with its train: {why}.", False
    try:
        state, why = squash(pull, car.head)
    except DOWN as down:  # the call may have gone through: its fate is unknown
        return outcomes.STOPPED, down_why(down), True
    if state == outcomes.MERGED:
        return state, why, False
    if state == landing.BASE_MOVED:
        return outcomes.REQUEUED, f"not landed: {why}", False
    return outcomes.HANDED_BACK, why, state == landing.UNKNOWN


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
