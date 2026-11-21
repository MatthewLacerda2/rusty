#!/usr/bin/env python3
"""`blockers.py [--fix]` — diff prose "Blocked by #N" against GitHub's record (#624, #2489).

The dependency graph *is* the plan (CLAUDE.md), and every tool and agent that
plans reads it through GitHub's native `blocked_by` relationships. A blocker
written only in an issue's prose is invisible to all of them: on 2026-09-30 the
batch started #399 while its body said "Blocked by #396" and #396 was open, and
a sweep then found 21 such prose-only mentions across 13 open issues.

This lists every open issue whose body says `Blocked by #N` (any case, lists
like "#396 and #397" included) where N is an **open issue** that the issue's
recorded `dependencies/blocked_by` does not hold. Closed blockers, pull
requests and self-references are skipped: none of them blocks anything now.
With `--fix` it records each one through the REST API. Every call is REST, so
it runs from a cloud session too (#970).

The other direction is stale relationships (#2489): a recorded blocker that is
an **open** issue and that no body states. The merge queue reads the
relationship, not the prose, so a dependency lifted by editing the body still
handed back the PR that closed the issue, and the relationship had to be
deleted by hand through the REST API. `--fix` removes each stale one (`DELETE
…/dependencies/blocked_by/{issue_id}`).

**A deletion needs proof the relationship is stated nowhere; a doubt keeps it
and reports it** (the review of PR #2516): a blocker lost to a wording the
parser cannot read is a PR the queue merges too early. So:

- `stated` reads every list a person writes ("#7, #8, and #9", "#7 and by #9",
  a bulleted list under "Blocked by:", an issue URL of this repository), and a
  number inside a parenthesised reason is not an item ("#7 (see #3)" is #7).
- A "Blocked by" followed, in its clause, by a reference the parser did not
  take makes the issue's unmatched recorded blockers *could not be read*: exit
  1, and `--fix` deletes none of them.
- "Blocks #5" in the blocker's own body states the relationship too (CLAUDE.md,
  *Dependencies*, names both directions): each candidate's body is read first,
  one call per candidate, and candidates are rare.
- A recorded blocker in another repository (`repository_url`) is never stale
  nor matched against a bare `#N`: it is named and left alone.
- A dependency is lifted in the body by deleting its line, striking it through
  (`~~Blocked by #7~~`) or writing "no longer blocked by #7".

A relationship to a closed issue is history and blocks nothing, so it is left
alone; a sub-issue link is not a `blocked_by` relationship and is never read.
Only issues whose listing reports an open recorded blocker
(`issue_dependencies_summary.blocked_by`) or whose body states one are asked
for their record, so a board of unblocked issues costs one listing.

Recording and removing touch `planning` issues too, deliberately: making the
record match what the prose already states changes nobody's scope.

Exits 0 when prose and record agree (or `--fix` made them agree), 1 when an
audit found a difference or a recorded blocker could not be read (with `--fix`
too), so `make blockers` reads as a check.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from typing import NamedTuple

import ghrest

#: A reference to an issue: `#N`, this repository's issue URL, or another
#: repository's (`owner/name#N`, or its URL), optionally bold or italic.
REF = re.compile(r"[*_]*(?:#(\d+)|https?://github\.com/([\w.-]+/[\w.-]+)/issues/(\d+)(?:#[\w-]*)?"
                 r"|([\w.-]+/[\w.-]+)#(\d+))[*_]*", re.IGNORECASE)
#: A reason after an item: "#7 (the bake must land first, see #3)" is #7 alone.
REASON = re.compile(r"[ \t]*\([^)\n]*\)[*_]*")
#: Between two items: ", ", ", and ", " and by ", " & ", or plain spaces.
SEP = re.compile(r"[ \t]*(?:,[ \t]*(?:and[ \t]+)?|&[ \t]*|\band[ \t]+|[ \t]+)(?:by[ \t]+)?[*_]*", re.IGNORECASE)
#: Where a clause stops: a line's end, a sentence's, or a semicolon.
CLAUSE_END = re.compile(r"\n|\.(?=\s|$)|;")
BULLET = re.compile(r"[ \t]*(?:[-*+]|\d+[.)])[ \t]+(?:\[[ xX]\][ \t]+)?")
NEGATION = re.compile(r"\b(?:not|no longer|never)\s*[*_]*$", re.IGNORECASE)
STRUCK = re.compile(r"~~.*?~~")
VERBS = {
    "blocked by": re.compile(r"\bblocked[*_]*\s+by\b[*_:]*[ \t]*", re.IGNORECASE),
    "blocks": re.compile(r"\bblocks\b[*_:]*[ \t]*", re.IGNORECASE),
}


class Stated(NamedTuple):
    """What a body says after a verb: the issue numbers of this repository, in
    order, and whether a reference followed the verb that the parser did not take."""

    numbers: list[int]
    unread: bool


def items(text: str, pos: int, repo: str, into: list[int]) -> tuple[int, int]:
    """Read a list of references at `pos` into `into`; (where it stopped, how many)."""
    taken = 0
    while ref := REF.match(text, pos):
        number = ref.group(1) or (ref.group(3) if (ref.group(2) or "").lower() == repo.lower() else None)
        if number and int(number) not in into:
            into.append(int(number))
        taken, pos = taken + 1, ref.end()
        if reason := REASON.match(text, pos):
            pos = reason.end()
        sep = SEP.match(text, pos)
        if not sep or not REF.match(text, sep.end()):
            break
        pos = sep.end()
    return pos, taken


def leftover(text: str, pos: int) -> bool:
    """Whether a reference sits between `pos` and the end of its clause."""
    end = CLAUSE_END.search(text, pos)
    return bool(REF.search(text[pos:end.start() if end else len(text)]))


def lifted(text: str, at: int) -> bool:
    """Whether the verb at `at` is negated ("no longer blocked by") or struck through."""
    line_start = text.rfind("\n", 0, at) + 1
    line_end = text.find("\n", at)
    line = text[line_start:line_end if line_end >= 0 else len(text)]
    struck = any(m.start() <= at - line_start < m.end() for m in STRUCK.finditer(line))
    return struck or bool(NEGATION.search(text[:at]))


def stated(body: str | None, verb: str, repo: str) -> Stated:
    """Every issue `body` names after `verb` ("blocked by" or "blocks"), and whether
    any reference after the verb was left unread. The rule a caller keeps: a doubt
    is never read as "not stated"."""
    text, found, unread = body or "", [], False
    for match in VERBS[verb].finditer(text):
        if lifted(text, match.start()):
            continue
        pos, taken = items(text, match.end(), repo, found)
        if taken:
            unread |= leftover(text, pos)
        elif text[pos:pos + 1] in ("\n", ""):
            unread |= listed(text, pos, repo, found)
        else:
            unread |= leftover(text, pos)
    return Stated(found, unread)


def listed(text: str, pos: int, repo: str, into: list[int]) -> bool:
    """Read the bulleted list under a "Blocked by:" that ends its line; whether a
    reference was left unread. One blank line may come before the list."""
    lines = text[pos:].split("\n")[1:]
    if lines and not lines[0].strip():
        lines = lines[1:]
    unread = False
    for i, line in enumerate(lines):
        bullet = BULLET.match(line)
        if not bullet:
            return unread or (i == 0 and bool(REF.search(line)))
        end, taken = items(line, bullet.end(), repo, into)
        unread |= leftover(line, end) if taken else bool(REF.search(line))
    return unread


def mentioned(body: str | None, repo: str = "") -> list[int]:
    """The issue numbers `body` says it is blocked by, in order, without repeats."""
    return stated(body, "blocked by", repo).numbers


class Gh:
    """`gh` with JSON output parsed; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, *args: str) -> object:
        done = self.runner(["gh", *args], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            sys.exit(f"blockers: gh {' '.join(args)}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None


def has_record(issue: dict) -> bool:
    """Whether the listing says `issue` has an open recorded blocker; asked when it
    doesn't say. Called only when the body states none, so a closed blocker (which
    is history, never stale) is no reason to ask."""
    summary = issue.get("issue_dependencies_summary")
    return summary is None or bool(summary.get("blocked_by", summary.get("total_blocked_by")))


def ours(blocker: dict, repo: str) -> bool:
    """Whether a recorded blocker is an issue of this repository."""
    url = blocker.get("repository_url")
    return url is None or url.lower().endswith(f"/repos/{repo}".lower())


class Report(NamedTuple):
    """(issue, blocker) pairs: prose-only, stale, kept because the body could not
    be read, and recorded in another repository (left alone)."""

    gaps: list
    stale: list
    doubt: list
    foreign: list


def audit(gh: Gh, repo: str) -> Report:
    listed = ghrest.pages(lambda path: gh("api", path), f"repos/{repo}/issues?state=open") or []
    issues = [i for i in listed if "pull_request" not in i]
    cache: dict[int, dict] = {}

    def fetch(number: int) -> dict:
        if number not in cache:
            cache[number] = gh("api", f"repos/{repo}/issues/{number}")
        return cache[number]

    report = Report([], [], [], [])
    for issue in sorted(issues, key=lambda i: i["number"]):
        number = issue["number"]
        prose = stated(issue["body"], "blocked by", repo)
        wanted = [n for n in prose.numbers if n != number]
        if not wanted and not has_record(issue):
            continue
        recorded = gh("api", ghrest.blocked_by(repo, number)) or []
        report.foreign.extend((number, b) for b in recorded if not ours(b, repo))
        local = [b for b in recorded if ours(b, repo)]
        for blocker in local:
            if blocker.get("state") != "open" or blocker["number"] in wanted:
                continue
            if prose.unread:
                report.doubt.append((number, blocker))
                continue
            # "Blocks #N" in the blocker's own body states the relationship too.
            other = stated(fetch(blocker["number"]).get("body"), "blocks", repo)
            if other.unread:
                report.doubt.append((number, blocker))
            elif number not in other.numbers:
                report.stale.append((number, blocker))
        held = {b["number"] for b in local}
        for wanted_number in wanted:
            if wanted_number in held:
                continue
            blocker = fetch(wanted_number)
            if blocker["state"] == "open" and "pull_request" not in blocker:
                report.gaps.append((number, blocker))
    return report


def record(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "POST", f"repos/{repo}/issues/{issue}/dependencies/blocked_by",
       "-F", f"issue_id={blocker['id']}")


def remove(gh: Gh, repo: str, issue: int, blocker: dict) -> None:
    gh("api", "--method", "DELETE",
       f"repos/{repo}/issues/{issue}/dependencies/blocked_by/{blocker['id']}")


def main(argv: list[str], gh: Gh | None = None) -> int:
    gh = gh or Gh()
    fix = "--fix" in argv
    unknown = [a for a in argv if a != "--fix"]
    if unknown:
        sys.exit(f"blockers: unknown argument(s) {' '.join(unknown)}; usage: blockers.py [--fix]")
    repo = ghrest.slug()
    report = audit(gh, repo)
    for issue, blocker in report.gaps:
        if fix:
            record(gh, repo, issue, blocker)
        verb = "recorded" if fix else "in the body, not recorded on GitHub"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    for issue, blocker in report.stale:
        if fix:
            remove(gh, repo, issue, blocker)
        verb = "removed" if fix else "recorded on GitHub, not in either body"
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): {verb}")
    for issue, blocker in report.doubt:
        print(f"#{issue} blocked by #{blocker['number']} ({blocker['title']}): recorded on GitHub; "
              "a \"Blocked by\"/\"Blocks\" line could not be read, so it is kept. "
              "Write each blocker as `#N` (a list, a bullet each, or an issue URL)")
    for issue, blocker in report.foreign:
        where = blocker.get("repository_url", "").rsplit("/repos/", 1)[-1]
        print(f"#{issue} blocked by {where}#{blocker['number']}: in another repository, left alone")
    if not (report.gaps or report.stale or report.doubt):
        print("blockers: every open blocker in a body is recorded, and every recorded one is in a body")
        return 0
    if fix:
        return 1 if report.doubt else 0
    print(f"blockers: {len(report.gaps)} prose-only, {len(report.stale)} stale, "
          f"{len(report.doubt)} unreadable recorded blocker(s); `make blockers ARGS=--fix` "
          "records the first and removes the second, never the third")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
