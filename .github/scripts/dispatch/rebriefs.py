"""How many briefs an issue takes, and why each re-brief happened (#1724).

From 2026-10-29 to 2026-11-02 there were 113 briefs on 52 issues: 61 launches,
over half, were re-briefs, each one a cold session reading everything again,
and nothing recorded why. So a re-brief now carries a `Round:` line
(`issue-batch`, the re-brief template) naming its cause, and this module counts
them from the `Brief` comments themselves, which keep the data: no log to keep.

- A brief is a comment [`brief.is_brief`] recognises, by someone who can push
  ([`brief.TRUSTED`]): an untrusted one is never launched, so it never cost a
  session.
- An issue's first brief is its brief; every later one is a re-brief, even when
  the first falls before `since` (it is read anyway, so a window never turns a
  re-brief into a first brief).
- A re-brief with no `Round:` line, or one naming no known cause, counts as
  `unstated`. `brief.read` never asks for the line, so it can never refuse a
  launch, and the briefs written before it existed still count.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field

from dispatch import brief

#: The causes a re-brief's `Round:` line may name, in the order they print.
ROUNDS = ("review", "ci", "conflict", "other")
UNSTATED = "unstated"

# `Round: ci`, tolerating Markdown around the key and the value (`**Round:** ci`).
ROUND = re.compile(r"^[\s>*_-]*Round[*_]*\s*:[*_\s`]*([A-Za-z]+)", re.IGNORECASE | re.MULTILINE)
ISSUE = re.compile(r"/issues/(\d+)$")


def round_of(body: str) -> str:
    """The cause a re-brief names, or `unstated`."""
    match = ROUND.search(body or "")
    cause = match.group(1).lower() if match else ""
    return cause if cause in ROUNDS else UNSTATED


def issue_of(comment: dict) -> int | None:
    match = ISSUE.search(comment.get("issue_url") or "")
    return int(match.group(1)) if match else None


def briefs_by_issue(comments: list[dict]) -> dict[int, list[dict]]:
    """Each issue's trusted briefs, oldest first."""
    found: dict[int, list[dict]] = {}
    for comment in sorted(comments, key=lambda c: c.get("created_at", "")):
        number = issue_of(comment)
        if number is None or comment.get("author_association") not in brief.TRUSTED:
            continue
        if brief.is_brief(comment.get("body", "")):
            found.setdefault(number, []).append(comment)
    return found


@dataclass
class Tally:
    briefs: dict[int, int] = field(default_factory=dict)  # issue -> briefs in the window
    rounds: dict[str, int] = field(default_factory=lambda: dict.fromkeys((*ROUNDS, UNSTATED), 0))

    @property
    def total(self) -> int:
        return sum(self.briefs.values())

    @property
    def rebriefs(self) -> int:
        return sum(self.rounds.values())

    @property
    def in_one(self) -> int:
        """Issues whose only brief so far is the first: none was sent back."""
        return sum(1 for count in self.briefs.values() if count == 1)


def tally(comments: list[dict], since: str = "") -> Tally:
    """Count the briefs created at or after `since` (an ISO date or time; empty is all)."""
    counted = Tally()
    for number, briefs in briefs_by_issue(comments).items():
        for position, found in enumerate(briefs):
            if found.get("created_at", "") < since:
                continue
            counted.briefs[number] = counted.briefs.get(number, 0) + 1
            if position > 0:
                counted.rounds[round_of(found.get("body", ""))] += 1
    return counted


def report(counted: Tally, since: str = "", most: int = 5) -> str:
    """The numbers, as the batch report quotes them."""
    window = f"since {since}" if since else "all time"
    issues = len(counted.briefs)
    if not issues:
        return f"briefs {window}: none"
    lines = [
        f"briefs {window}: {counted.total} on {issues} issues, {counted.total / issues:.2f} per issue",
        f"one brief: {counted.in_one} of {issues} issues",
        f"re-briefs: {counted.rebriefs}",
        *(f"  {cause:<9} {n}" for cause, n in counted.rounds.items()),
    ]
    ranked = sorted(counted.briefs.items(), key=lambda item: (-item[1], item[0]))
    heavy = [f"#{number} ({n})" for number, n in ranked[:most] if n > 1]
    if heavy:
        lines.append("most briefed: " + ", ".join(heavy))
    return "\n".join(lines)
