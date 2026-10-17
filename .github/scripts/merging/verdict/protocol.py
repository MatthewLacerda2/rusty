"""What a pull request's description must say (#927).

The cloud brief (`.claude/skills/issue-batch/cloud-brief.md`, *Protocol* and
*What the description carries*) asks two things of every description, and in
scorsese both were missed: two of one batch's nine pull requests left out
`Closes #N`, so their issues stayed open after the merge, and five had no
**Gates** line. Ported from scorsese's `protocol.py` (MatthewLacerda2/scorsese#1229).

- **A branch named for an issue names it.** A head branch `N-…` is work on
  issue N, so the description either closes it (`Closes #N`, or any keyword
  GitHub honours) or says it deliberately leaves it open (`Refs #N`). Neither is
  a hand-back; [`closing`] says what a closing keyword may point at.
- **A Gates line says what ran.** Missing it is a warning, not a refusal (the run
  CI just made is the proof a merge needs), but it is said every time, so the
  habit is corrected where it is formed.

- **A closing keyword means it only on its own line** ([`stray`], #966), and what
  the diff carries is judged in [`carried`] (#966).

Pure functions of the pull request's `headRefName` and `body`. A pull request
read without those fields is not judged on them.
"""

from __future__ import annotations

import re

from merging.verdict import carried, closing


# A head branch named for an issue: its leading number.
BRANCH = re.compile(r"^(\d+)-")

# The keywords GitHub closes an issue on, and `refs`, the brief's way of saying
# an issue is deliberately left open.
KEYWORDS = r"(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?|refs)"

# A line that is a closing keyword and its number, and nothing else (#966).
OWN_LINE = re.compile(r"^\s*(?:[-*]\s+)?(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?):?\s+#\d+\s*$", re.IGNORECASE)

# A Gates line: `Gates:`, `**Gates**`, or a `Gates` heading, at a line start.
GATES = re.compile(r"^\s*(?:#+\s*)?(?:[-*]\s+)?\**Gates\b", re.MULTILINE)


def issue_of(branch: str) -> int | None:
    """The issue a head branch is named for, or `None`."""
    match = BRANCH.match(branch or "")
    return int(match.group(1)) if match else None


def names(body: str, issue: int) -> bool:
    """Whether `body` closes, or deliberately refers to, issue `issue`."""
    return re.search(rf"\b{KEYWORDS}\s*:?\s+#{issue}\b", body or "", re.IGNORECASE) is not None


def unnamed(pull: dict) -> list[str]:
    """Why the description does not name the branch's issue, or nothing.

    A pull request that forgets the keyword leaves its issue open after the merge,
    and nobody notices until the board is read again. The hand-back costs one
    edit of the description: no push, no new CI run.
    """
    if "body" not in pull or "headRefName" not in pull:
        return []
    issue = issue_of(pull["headRefName"])
    if issue is None or names(pull["body"], issue):
        return []
    return [
        f"the branch is {pull['headRefName']}, but the description does not close #{issue}.",
        f"Write `Closes #{issue}` on its own line (or `Refs #{issue}` if the issue is"
        " deliberately left open); editing the description needs no push and no new run.",
    ]


def stray(pull: dict) -> list[str]:
    """The lines where a closing keyword sits inside a sentence, or nothing (#966).

    GitHub closes on the keyword wherever it stands, so "close #769 with this
    diagnosis" closed #769 (#777), and quoting "its `Closes #769`" closed it again
    (#812). Only a line that is nothing but the keyword and its number counts as
    meaning it; any other is handed back, quoted, for a one-edit fix.
    """
    lines = (pull.get("body") or "").splitlines()
    found = [line.strip() for line in lines if closing.KEYWORD.search(line) and not OWN_LINE.match(line)]
    if not found:
        return []
    return [
        f"a closing keyword sits inside a sentence: \"{found[0]}\".",
        *(f"Also: \"{line}\"." for line in found[1:]),
        "GitHub closes on it wherever it stands. Put `Closes #N` on a line of its own"
        " to mean it; otherwise reword (`Refs #N`, or drop the keyword).",
    ]


def gates(pull: dict) -> list[str]:
    """A warning when the description has no Gates line, or nothing."""
    if "body" not in pull or GATES.search(pull["body"] or ""):
        return []
    return ["Warning: the description has no **Gates** line saying what ran green on which head (cloud brief)."]


def refusal(pull: dict) -> list[str]:
    """Every reason what this pull request claims refuses it; empty when none does.

    `pull["closes"]`, when present, is [`closing.closed_by`]: labels and states
    as they stand now. A caller that did not read them is not judged on them.
    """
    return (
        closing.refusal(pull.get("closes") or {})
        + unnamed(pull)
        + stray(pull)
        + carried.issueless(pull)
        + carried.images(pull)
    )
