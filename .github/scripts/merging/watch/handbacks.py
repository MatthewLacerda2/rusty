"""The handed-back heads remembered across watches, and what re-admits each (#751, #869, #1279).

Every hand-back remembers its head ([`MEMORY`]) and is passed over until that
moves. Five things count as moving without a push:

- **a new run attempt on the head** ([`RUNS_MEMORY`], [`rerun_heads`], #869): `gh
  run rerun --failed` after a runner outage or a flake;
- **a changed description or labels** ([`CLAIMS_MEMORY`], [`reworded`], #1279), for
  a hand-back an edit fixes (a missing `Closes #N`, a stray keyword, an
  issue-less code change). #1275's hand-back said "editing the description needs
  no push", but only a head move re-admitted it, so the fixed pull request sat
  until the orchestrator restarted the daemon, and the memory survives a
  restart, so even that did not help until a CI re-run;
- **the issue it closes no longer blocking it** ([`ISSUES_MEMORY`], [`unblocked`],
  #1279): labelled `planning`/`human`, blocked or already closed, which no edit
  of the pull request fixes. Those issues are asked again each listing;
- **a comment recording its review decision** ([`REVIEWS_MEMORY`], [`reviewed`],
  #1764): a pull request queued before anyone wrote `Independent review` or
  `No review: <why>` on it. Its claim is kept too, so a label taken off and put
  back re-admits it like an edit;
- **the runs it outwaited going green** ([`STALLS_MEMORY`], [`stalls.returned`],
  #1831): a take that hit its deadline with nothing red and nothing green.
  #1820 and #1827 went green after such a hand-back during GitHub's Actions
  incidents of 2026-11-04 and sat parked, since a run *ending* changes no
  attempt. The check is on the runs' state, not a change of mark. A stall
  that ends cancelled or red is said once and then waits like a red run.

- **its gating runs reading green now** ([`RED_MEMORY`], [`greened`], #1951): a
  hand-back for a red `ci` or `lint` run. #1920 was handed back during
  GitHub's 504 outage of 2026-11-08; GitHub failed to give the #869 mark at
  the hand-back, the run was re-run green, and the first read after that
  *became* the mark, so nothing ever differed from it and a ready, green,
  labelled pull request sat passed over for seven hours. It was the fourth
  hand-back parking a fix that had already happened (#881, #1279, #1831), each
  fixed by teaching the change-based mark one more case. So a red hand-back
  is re-admitted on **state**: its unmoved head's gating runs, read now, all
  green, whatever the marks say. A stall that ended red joins it.

The mark itself is never taken from a later read (#1951): a red hand-back
carries the runs its verdict read ([`outcomes.Red`]), and any other kind whose
read failed at the hand-back has none, and waits for its own signal or a push.

A conflict, a refused description or a committed image has no state entry and
waits for its head (or its own kind's signal): green CI never re-admits them.
Every side memory is kept only for numbers still in [`MEMORY`].
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from merging.watch import board, stalls


# Where the remembered hand-backs live, under the checkout's git directory.
MEMORY = "merge-queue/handed-back.json"

# Beside it, the runs on each remembered head as [`attempts`] spells them, so a
# re-run of the same head reads as movement (#869).
RUNS_MEMORY = "handed-back-runs.json"


# Beside it too, for a hand-back an edit fixes, what the description and labels
# were ([`claim`]): a change re-admits it with no push (#1279).
CLAIMS_MEMORY = "handed-back-claims.json"

# And for a hand-back the issue refused, which issues to ask about again (#1279).
ISSUES_MEMORY = "handed-back-issues.json"


# And for a hand-back no review decision was recorded on, its number (#1764).
REVIEWS_MEMORY = "handed-back-reviews.json"

# And for a hand-back that outwaited its runs, its number (#1831, [`stalls`]):
# asked about until those runs end.
STALLS_MEMORY = "handed-back-stalls.json"

# And for a hand-back a red run caused, the runs the verdict read red (#1951,
# [`greened`]): asked each pass until they read green.
RED_MEMORY = "handed-back-red.json"

# And for every hand-back, when (seconds since the epoch) and why, so
# `make queue-status` can name one passed over for long with a green head
# (#1951, [`view.stranded`]).
WHY_MEMORY = "handed-back-why.json"


def claim(pull: dict) -> str:
    """What a pull request claims, as one comparable string: its body and labels."""
    said = json.dumps({"body": pull.get("body") or "", "labels": sorted(board.names(pull.get("labels")))}, sort_keys=True)
    return hashlib.sha256(said.encode()).hexdigest()[:16]


def reworded(pulls: list[dict], memory: dict[int, str], claims: dict[int, str]) -> list[int]:
    """The remembered edit hand-backs whose description or labels changed since (#1279).

    Only an entry in `claims` is asked: a red run or a conflict waits for its
    head. A pull request listed without a body is no news.
    """
    return [
        pull["number"] for pull in pulls
        if pull.get("number") in claims and memory.get(pull.get("number")) is not None
        and "body" in pull and claims[pull["number"]] != claim(pull)
    ]


def unblocked(pulls: list[dict], memory: dict[int, str], issues: dict[int, str], blocks) -> list[int]:
    """The remembered issue hand-backs whose issues no longer refuse them (#1279).

    `issues` holds the refusing issues as each was handed back, spaced
    ([`spell`]); `blocks(numbers)` asks GitHub which still refuse, `None` when it
    could not answer (no news). Asked only for ready pull requests.
    """
    return [
        pull["number"] for pull in pulls
        if not pull.get("isDraft") and pull.get("number") in memory and pull.get("number") in issues
        and blocks(read(issues[pull["number"]])) == []
    ]


def reviewed(pulls: list[dict], memory: dict[int, str], reviews: dict[int, str], decided) -> list[int]:
    """The remembered review hand-backs that now carry the decision (#1764).

    `decided(number)` asks GitHub ([`verdict.reviewed.decided`]), `None` when it
    could not answer (no news). Asked only for ready pull requests.
    """
    return [
        pull["number"] for pull in pulls
        if not pull.get("isDraft") and pull.get("number") in memory and pull.get("number") in reviews
        and decided(pull["number"]) is True
    ]


def spell(numbers) -> str:
    """Issue numbers as [`ISSUES_MEMORY`] keeps them."""
    return " ".join(str(n) for n in numbers)


def read(spelled: str) -> tuple[int, ...]:
    return tuple(int(n) for n in spelled.split())


def forget_closed(memory: dict[int, str], pulls: list[dict]) -> dict[int, str]:
    """The memory without pull requests that are no longer open."""
    open_ = {p.get("number") for p in pulls}
    return {n: sha for n, sha in memory.items() if n in open_}


def attempts(runs: list[dict]) -> str:
    """The runs on a head as one comparable string: each run's id and attempt.
    A re-run bumps an attempt; a new run (a dispatch, a fresh `ready`) adds an id."""
    return " ".join(sorted(f"{r.get('id')}.{r.get('run_attempt', 1)}" for r in runs))


def rerun_heads(pulls: list[dict], memory: dict[int, str], marks: dict[int, str], runs_now) -> list[int]:
    """The remembered hand-backs whose unmoved head has run again since (#869).

    `marks` holds [`attempts`] as each was handed back; `runs_now(number, sha)`
    asks GitHub for today's, `None` when it could not answer (no news, so
    still passed over). Asked only for ready pull requests whose head is the
    remembered one. An entry with no mark (GitHub failed at the hand-back) is
    not asked: a later read taken as its mark could already be the re-run that
    fixed it, and nothing would ever differ from it again (#1951, #1920). A red
    hand-back always has one ([`outcomes.Red`]) and is re-read on state too.
    """
    again = []
    for pull in pulls:
        number, sha = pull.get("number"), pull.get("headRefOid")
        if pull.get("isDraft") or memory.get(number) != sha or number not in marks:
            continue
        now = runs_now(number, sha)
        if now is not None and marks[number] != now:
            again.append(number)
    return again


def greened(pulls: list[dict], memory: dict[int, str], reds: dict[int, str], ask) -> list[int]:
    """The remembered red hand-backs whose unmoved head's gating runs read
    green now (#1951), whatever any mark says. `ask(pull)` is
    [`stalls.reading`] of those runs, `None` when GitHub failed (no news).
    Asked only for ready pull requests that carry the `queue` label, once each
    per pass: a red head that stays red costs a read a pass, nothing more."""
    return [
        pull["number"] for pull in pulls
        if pull.get("number") in reds and not pull.get("isDraft") and board.queued(pull)
        and memory.get(pull.get("number")) == pull.get("headRefOid")
        and (ask(pull) or ("",))[0] == stalls.GREEN
    ]


def recall(path: Path) -> dict[int, str]:
    """The remembered hand-backs; a missing or unreadable file is none."""
    try:
        return {int(n): sha for n, sha in json.loads(path.read_text()).items()}
    except (OSError, ValueError, AttributeError):
        return {}


def remember(path: Path, memory: dict[int, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({str(n): sha for n, sha in sorted(memory.items())}, indent=1) + "\n")
