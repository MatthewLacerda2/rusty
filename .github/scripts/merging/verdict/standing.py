"""Where a pull request stands with its review: the verdict line and what is still owed (#2628).

Until #2628 a model added the `queue` label after a checklist a script can read,
and one hand-back that batch came from a record whose first line [`reviewed`]'s
pattern did not take: a clerical miss a machine would not make. So the record
says the decision in one machine line, and the watch reads it:

- **`Verdict: queue on <sha>`** (or `round`, `hold`), a line of its own anywhere
  in a review record (a comment whose first line [`reviewed.decides`] takes:
  `Independent review…`, the outcomes after a round headed the same way, or
  `No review: <why>`), from someone who can push ([`ghrest.can_push`]). `<sha>`
  is the head it was given on, seven to forty hex digits. The newest such line
  wins ([`said`]); a record with none leaves the pull request to its label, as
  before #2628.
- **It holds for that head only** ([`merges_only`]): commits after it that are
  all merges (of `main`, the queue's own update) keep it; any other commit is
  unreviewed code, so a `queue` there reads `round` ([`effective`]).
- **`Needs: <what>`** lines in a record are owed until a later trusted comment
  says **`Done: <what>`**, the same words ([`owed`]; case, spacing and a closing
  full stop aside). While one is owed the pull request is out of line, its
  label or verdict notwithstanding: a desktop measurement nobody took is not
  taken by a label.

A bare `No review: <why>` carries no head, so it is not a `queue` here (the
issue recommended it be): read as one, it would admit every later push
unreviewed, and every open pull request with an old waiver at the moment this
code started. With a verdict line it is one.

Pure; [`queue.verdicts`] reads GitHub for it.
"""

from __future__ import annotations

import re

import ghrest
from merging.verdict import reviewed

QUEUE, ROUND, HOLD = "queue", "round", "hold"

# The one machine line: its own line, nothing else on it.
VERDICT = re.compile(r"^[ \t]*Verdict:[ \t]*(queue|round|hold)[ \t]+on[ \t]+([0-9a-f]{7,40})[ \t]*$", re.I | re.M)
NEEDS = re.compile(r"^[ \t]*Needs:[ \t]*(\S.*?)[ \t]*$", re.I | re.M)
DONE = re.compile(r"^[ \t]*Done:[ \t]*(\S.*?)[ \t]*$", re.I | re.M)


def line(word: str, sha: str) -> str:
    """The verdict line as a record writes it."""
    return f"Verdict: {word} on {sha[:7]}"


def trusted(comments: list[dict]) -> list[dict]:
    """The comments that may instruct the queue, oldest first."""
    return sorted((c for c in comments if ghrest.can_push(c)), key=lambda c: c.get("created_at") or "")


def records(comments: list[dict]) -> list[dict]:
    return [c for c in trusted(comments) if reviewed.decides(c.get("body") or "")]


def said(comments: list[dict]) -> tuple[str, str] | None:
    """The newest record's verdict, `(word, sha)`, or `None` when no record has one."""
    for comment in reversed(records(comments)):
        found = VERDICT.findall(comment.get("body") or "")
        if found:
            word, sha = found[-1]
            return word.lower(), sha.lower()
    return None


def key(text: str) -> str:
    return " ".join(text.split()).rstrip(".").casefold()


def owed(comments: list[dict]) -> list[str]:
    """Each `Needs:` of a record no later trusted `Done:` answers, in the order asked."""
    asked: dict[str, str] = {}
    for comment in trusted(comments):
        body = comment.get("body") or ""
        for done in DONE.findall(body):  # before this comment's own asks: an answer comes later
            asked.pop(key(done), None)
        if reviewed.decides(body):
            for need in NEEDS.findall(body):
                asked.setdefault(key(need), need)
    return list(asked.values())


def merges_only(commits: list[dict], ahead: int, since: str, head: str) -> bool:
    """Whether every commit between `since` (a prefix) and `head` is a merge.

    `commits` and `ahead` are GitHub's compare of `main...head`: the commits on
    the branch and not on `main`, and how many there are. Walked from `head`
    through those parents, stopping at `since`, each commit met must have two
    parents; a parent on `main` is outside the list and ends that path. `since`
    never met (a force-push dropped it) or a list GitHub cut short is `False`:
    a doubt reads unreviewed."""
    if head.startswith(since):
        return True
    if ahead > len(commits):
        return False
    branch = {c.get("sha", ""): [p.get("sha", "") for p in c.get("parents") or []] for c in commits}
    seen, stack, met = set(), [head], False
    while stack:
        sha = stack.pop()
        if sha in seen or sha not in branch:
            continue
        seen.add(sha)
        if sha.startswith(since):
            met = True
            continue
        parents = branch[sha]
        if len(parents) < 2:
            return False
        stack.extend(parents)
    return met


def effective(verdict: tuple[str, str] | None, current: bool) -> str | None:
    """What the queue acts on: a `queue` on a head with unreviewed commits since is a `round`."""
    if verdict is None:
        return None
    return ROUND if verdict[0] == QUEUE and not current else verdict[0]


def standing(comments: list[dict], current: bool | None = None) -> dict:
    """The plain dictionary a listed pull request carries as `standing`.
    `current` is [`merges_only`]'s answer for the verdict's head (`None`: not asked)."""
    verdict = said(comments)
    return {"verdict": effective(verdict, bool(current)), "said": verdict and verdict[0],
            "on": verdict and verdict[1], "owed": owed(comments)}
