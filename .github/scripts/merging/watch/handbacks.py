"""The handed-back heads remembered across watches, and the runs on them (#751, #869)."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


# Where the remembered hand-backs live, under the checkout's git directory.
MEMORY = "merge-queue/handed-back.json"

# Beside it, the runs on each remembered head as [`attempts`] spells them, so a
# re-run of the same head reads as movement (#869).
RUNS_MEMORY = "handed-back-runs.json"


# Beside it too, for a hand-back an edit fixes, what the description and labels
# were ([`claim`]): a change re-admits it with no push (#1279).
CLAIMS_MEMORY = "handed-back-claims.json"


def claim(pull: dict) -> str:
    """What a pull request claims, as one comparable string: its body and labels."""
    labels = sorted(label.get("name", "") if isinstance(label, dict) else str(label) for label in pull.get("labels") or [])
    said = json.dumps({"body": pull.get("body") or "", "labels": labels}, sort_keys=True)
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
    remembered one. An entry with no mark (remembered before #869, or GitHub
    failed at the hand-back) takes today's as its mark.
    """
    again = []
    for pull in pulls:
        number, sha = pull.get("number"), pull.get("headRefOid")
        if pull.get("isDraft") or memory.get(number) != sha:
            continue
        now = runs_now(number, sha)
        if now is None:
            continue
        if number not in marks:
            marks[number] = now
        elif marks[number] != now:
            again.append(number)
    return again


def recall(path: Path) -> dict[int, str]:
    """The remembered hand-backs; a missing or unreadable file is none."""
    try:
        return {int(n): sha for n, sha in json.loads(path.read_text()).items()}
    except (OSError, ValueError, AttributeError):
        return {}


def remember(path: Path, memory: dict[int, str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps({str(n): sha for n, sha in sorted(memory.items())}, indent=1) + "\n")
