"""Where a pull request stands with its review: the verdict line and what is still owed (#2628).

Until #2628 a model added the `queue` label after a checklist a script can read,
and one hand-back that batch came from a record whose first line [`reviewed`]'s
pattern did not take: a clerical miss a machine would not make. So the record
says the decision in one machine line, and the watch reads it:

- **`Verdict: queue on <sha>`** (or `round`, `hold`), a line of its own anywhere
  in a review record (a comment whose first line [`reviewed.decides`] takes:
  `Independent review…`, the outcomes after a round headed the same way, or
  `No review: <why>`), from someone who can push ([`ghrest.can_push`]). `<sha>`
  is the head it was given on, seven to forty hex digits. The newest record
  decides ([`said`]): its last verdict line is the verdict, and a newest record
  with none means no verdict, so an older record's `queue` does not survive it
  and the pull request is left to its label, as before #2628.
- **It holds for that head only** ([`merges_only`]): commits after it that are
  all merges (of `main`, the queue's own update) keep it; any other commit is
  unreviewed code, so a `queue` there reads `round` ([`effective`]).
- **`Needs: <what>`** lines in a record are owed until a later trusted comment
  says **`Done: <what>`**, the same words ([`owed`]; case, spacing and a closing
  full stop aside). While one is owed the pull request is out of line, its
  label or verdict notwithstanding: a desktop measurement nobody took is not
  taken by a label.

The lines accept a `\r` at their end (a comment written from Windows).

**A limit, left unchecked:** [`merges_only`] takes any two-parent commit for a
merge of `main` without reading what it brought in. A merge commit that also
carries a hand-made change (an evil merge) keeps a `queue` verdict; the queue's
own updates and `merge-fixer`'s are plain merges, and the reviewer of the next
round reads the whole diff.

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
VERDICT = re.compile(r"^[ \t]*Verdict:[ \t]*(queue|round|hold)[ \t]+on[ \t]+([0-9a-f]{7,40})[ \t\r]*$", re.I | re.M)
NEEDS = re.compile(r"^[ \t]*Needs:[ \t]*(\S.*?)[ \t\r]*$", re.I | re.M)
DONE = re.compile(r"^[ \t]*Done:[ \t]*(\S.*?)[ \t\r]*$", re.I | re.M)


def line(word: str, sha: str) -> str:
    """The verdict line as a record writes it."""
    return f"Verdict: {word} on {sha[:7]}"


def trusted(comments: list[dict]) -> list[dict]:
    """The comments that may instruct the queue, oldest first."""
    return sorted((c for c in comments if ghrest.can_push(c)), key=lambda c: c.get("created_at") or "")


def records(comments: list[dict]) -> list[dict]:
    return [c for c in trusted(comments) if reviewed.decides(c.get("body") or "")]


def said(comments: list[dict]) -> tuple[str, str] | None:
    """The newest record's verdict, `(word, sha)`, or `None` when there is no
    record or the newest has no verdict line: an older verdict never outlives it."""
    found = records(comments)
    found = VERDICT.findall(found[-1].get("body") or "") if found else []
    if not found:
        return None
    word, sha = found[-1]
    return word.lower(), sha.lower()


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


def effective(verdict: tuple[str, str] | None, current: bool | None) -> str | None:
    """What the queue acts on: a `queue` on a head with unreviewed commits since,
    or one whose head could not be checked (`current` `None`), is a `round`."""
    if verdict is None:
        return None
    return ROUND if verdict[0] == QUEUE and current is not True else verdict[0]


def standing(comments: list[dict], current: bool | None = None) -> dict:
    """The plain dictionary a listed pull request carries as `standing`.
    `current` is [`merges_only`]'s answer for the verdict's head: `True`, `False`,
    or `None` when it could not be read (or was not asked). It is kept, so a
    reader can tell a head that moved past its verdict (`False`) from one never
    checked."""
    verdict = said(comments)
    return {"verdict": effective(verdict, current), "said": verdict and verdict[0],
            "on": verdict and verdict[1], "current": current, "owed": owed(comments)}
