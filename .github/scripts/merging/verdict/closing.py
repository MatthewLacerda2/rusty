"""What a pull request closes, and whether the merge path may close it (#927).

CLAUDE.md *Overrides*: an issue labelled `planning` is never started while the
label is on it, the one rule the user cannot override; and *Issues, labels &
priority*: `planning` and `human` are the stage labels, the only absolute stop.
A pull request that closes such an issue started it. Until #927 prose held that.

So [`decide.judge`] and the queue's take both refuse one, naming the issue and
the label. The label is read as it stands **at merge time**: the operator may
remove it and greenlight the work, and the same pull request then merges.

The same pass refuses a closing keyword that points at a pull request, at an
issue already closed, or at a number that does not exist: a typo'd number
closes nothing, and the issue that was meant stays open after the merge. And it
refuses closing an issue with an open recorded blocker (#966): CLAUDE.md's
*Dependencies* makes only an unblocked issue fair game. `blocked_by` is read at
merge time too, so closing the blocker or removing the relationship unblocks it.

Which issues a pull request closes is read two ways and joined, because each
misses something: GitHub's `closingIssuesReferences` holds issues linked in the
sidebar too, never a pull request or an unresolved number; the body is where a
keyword pointing at the wrong thing is still visible. Ported from
scorsese's `closing.py` (MatthewLacerda2/scorsese#1221).

Pure but for [`closed_by`], which asks GitHub over REST (`issues/N` answers for a
pull request number too, and says so with a `pull_request` key).
"""

from __future__ import annotations

import ghrest
from merging.verdict import github


# The labels that mean *do not start* (CLAUDE.md, *Issues, labels & priority*).
STAGES = ("planning", "human")

# GitHub's closing keywords, then `#N` in this repository. GitHub matches them
# anywhere in a description, quoted or negated alike, so this does too: a pull
# request meaning to leave an issue open writes `Refs #N`.
KEYWORD = ghrest.CLOSING

# What [`closed_by`] records for a number GitHub does not know.
MISSING = {"missing": True}


def referenced(pull: dict) -> list[int]:
    """Every issue number this pull request closes, by keyword or by link."""
    numbers = {int(found) for found in KEYWORD.findall(pull.get("body") or "")}
    numbers |= {ref["number"] for ref in pull.get("closingIssuesReferences") or [] if ref.get("number")}
    return sorted(numbers)


def labels(issue: dict) -> list[str]:
    """An issue's label names, from REST (`name` objects) or plain strings."""
    return [label.get("name", "") if isinstance(label, dict) else str(label) for label in issue.get("labels") or []]


def refusal(closes: dict[int, dict]) -> list[str]:
    """Why this pull request may not close what it closes; empty when it may.

    `closes` maps each number from [`referenced`] to GitHub's REST answer for it
    ([`MISSING`] when there was none). The first line is the verdict, as with
    every refusal on the merge path.
    """
    wrong, staged = [], []
    for number, issue in sorted(closes.items()):
        if issue.get("missing"):
            wrong.append(f"it closes #{number}, which does not exist.")
        elif "pull_request" in issue:
            wrong.append(f"it closes #{number}, which is a pull request, not an issue.")
        elif issue.get("state") == "closed":
            wrong.append(f"it closes #{number}, which is already closed.")
        staged += [f"it closes #{number}, which is labelled `{stage}`." for stage in STAGES if stage in labels(issue)]
        staged += [f"it closes #{number}, which is blocked by #{b} (open)." for b in open_blockers(issue)]
    lines = staged + wrong
    if staged:
        lines.append(
            "`planning` and `human` mean do not start (CLAUDE.md, *Overrides*: never"
            " with `planning` on), and neither does an open blocker (*Dependencies*)."
            " Only the operator removes the label or the block; the watch takes it"
            " again once they have (#1279)."
        )
    if wrong:
        lines.append("Fix the number, or write `Refs #N` for an issue left open; editing the description needs no push.")
    return lines


def blocking(closes: dict[int, dict]) -> list[int]:
    """The issues among `closes` that refuse it whatever the description says:
    labelled `planning`/`human`, blocked by an open issue, or already closed.
    Only a change to the issue clears them (#1279), unlike a wrong number."""
    return sorted(
        number for number, issue in closes.items()
        if issue.get("state") == "closed" or any(s in labels(issue) for s in STAGES) or open_blockers(issue)
    )


def open_blockers(issue: dict) -> list[int]:
    """The open issues recorded as blocking this one ([`closed_by`] reads them)."""
    return sorted(b["number"] for b in issue.get("blocked_by") or [] if b.get("state") == "open")


def closed_by(repo: str, pull: dict) -> dict[int, dict]:
    """GitHub's answer for every number [`referenced`] finds, [`MISSING`] for a 404,
    each open issue with its recorded `blocked_by` beside it."""
    closes = {}
    for n in referenced(pull):
        issue = github.gh("api", f"repos/{repo}/issues/{n}", missing=MISSING)
        if issue.get("state") == "open" and "pull_request" not in issue:
            path = f"repos/{repo}/issues/{n}/dependencies/blocked_by?per_page=100"
            issue = {**issue, "blocked_by": github.gh("api", path, missing=[])}
        closes[n] = issue
    return closes
