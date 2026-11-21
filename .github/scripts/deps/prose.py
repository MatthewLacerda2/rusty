"""What an issue body says about its dependencies: the `Blocked by #N` it states
(`stated`), and every issue it names at all (`named`).

The two answer different questions, on purpose (#2489). `stated` decides what to
**record**: a grammar over prose, reading every list a person writes and saying
when it met a reference it could not read. `named` decides what may be
**deleted**: a grammar cannot prove a blocker absent (seven wordings slipped past
one in the review of PR #2516: a CRLF list, a wrapped line, `owner/name#N`, a
"not" on the line above, a reason, "e.g.", a loose list), so a recorded blocker
is only removed when its number appears nowhere in the body.

Both read the body after `clean`: line ends normalised and struck-through text
(`~~Blocked by #7~~`) dropped, which is how a dependency is lifted in prose.
"""

from __future__ import annotations

import re
from typing import NamedTuple

#: A reference to an issue: `#N`, an issue or pull-request URL, or `owner/name#N`,
#: optionally bold or italic. Groups: bare number; URL repo, number; repo, number.
REF = re.compile(r"[*_]*(?:#(\d+)(?!\d)|https?://github\.com/([\w.-]+/[\w.-]+)/(?:issues|pull)/(\d+)"
                 r"(?:#[\w-]*)?|([\w.-]+/[\w.-]+)#(\d+)(?!\d))[*_]*", re.IGNORECASE)
#: A reason after an item, which may wrap: "#7 (the bake must land first, see #3)" is #7.
REASON = re.compile(r"[ \t]*\((?:[^()\n]|\n(?![ \t]*\n))*\)[*_]*")
#: Between two items on a line: ", ", ", and ", " and by ", " & ", " + ", bold, or spaces.
SEP = re.compile(r"(?:[ \t]+|,|&|\+|\band\b|\bby\b|[*_]+)*", re.IGNORECASE)
BULLET = re.compile(r"[ \t]*(?:[-*+]|\d+[.)])[ \t]+(?:\[[ xX]\][ \t]+)?")
LEAD = re.compile(r"[ \t]*")
BLANK = re.compile(r"\n(?:[ \t]*\n)+")
#: What a line ends with when the clause goes on below it.
OPEN_END = re.compile(r"(?:,|&|:|\band|\bor|\bby)[ \t*_]*$", re.IGNORECASE)
#: What a line starts with when it begins something new: a sentence, a heading,
#: a bullet, a quote, a table or a fence.
NEW_LINE = re.compile(r"[ \t]*(?:[A-Z]|#{1,6}[ \t]|(?:[-*+]|\d+[.)])[ \t]|>|\||```)")
#: Candidates for a clause's end; `ends` throws out the ones that are not.
STOP = re.compile(r";|[.!?](?=\s|$)|\n")
ABBREVIATION = re.compile(r"\b(?:e\.g|i\.e|cf|etc|vs|viz|approx|incl|resp|fig|no)$", re.IGNORECASE)
NEGATION = re.compile(r"\b(?:not|no longer|never)[ \t]*[*_]*[ \t]*$", re.IGNORECASE)
STRUCK = re.compile(r"~~(?:(?!\n[ \t]*\n).)+?~~", re.DOTALL)
VERBS = {
    "blocked by": re.compile(r"\bblocked[*_]*\s+by\b[*_:]*[ \t]*", re.IGNORECASE),
    "blocks": re.compile(r"\bblocks\b[*_:]*[ \t]*", re.IGNORECASE),
}


class Stated(NamedTuple):
    """The issue numbers of this repository a body states after a verb, in order,
    and the references it met in those clauses and could not read."""

    numbers: list[int]
    unread: list[str]


def clean(body: str | None) -> str:
    """`body` with CRLF (the web editor's) as LF and struck-through text dropped."""
    return STRUCK.sub("", (body or "").replace("\r\n", "\n").replace("\r", "\n"))


def number(ref: re.Match, repo: str) -> int | None:
    """The issue number `ref` names in `repo`, or `None` for another repository's."""
    if ref.group(1):
        return int(ref.group(1))
    other, value = (ref.group(2), ref.group(3)) if ref.group(2) else (ref.group(4), ref.group(5))
    return int(value) if other.lower() == repo.lower() else None


def named(body: str | None, repo: str) -> set[int]:
    """Every issue of `repo` the body names, in any sentence: the test a deletion
    rests on. A reference inside struck-through text is not named."""
    return {n for ref in REF.finditer(clean(body)) if (n := number(ref, repo)) is not None}


def stated(body: str | None, verb: str, repo: str) -> Stated:
    """Every issue `body` names after `verb` ("blocked by" or "blocks"), and the
    references in those clauses it could not read (a doubt, for the caller to
    report: never read as "not stated")."""
    text, found, unread = clean(body), [], []
    for match in VERBS[verb].finditer(text):
        line = text[text.rfind("\n", 0, match.start()) + 1:match.start()]
        if not NEGATION.search(line):
            scan(text, match.end(), repo, found, unread, sibling(line))
    # A number read elsewhere as a blocker is not lost where it was not.
    lost = [ref for ref in unread if number(ref, repo) not in found]
    return Stated(found, list(dict.fromkeys(ref.group(0).strip("*_") for ref in lost)))


def sibling(line: str) -> int | None:
    """When the verb sits in a bullet, the indent a bullet below needs to be part of
    its list (deeper than this one's); `None` when it does not."""
    return len(LEAD.match(line).group()) if BULLET.match(line) else None


def scan(text: str, pos: int, repo: str, found: list[int], unread: list[re.Match],
         nested: int | None = None) -> None:
    """Read the items after a verb at `pos`: an inline list, one wrapped over lines,
    or a bulleted one below (loose or not), into `found`; what it meets and cannot
    read, into `unread`. With `nested`, the verb is in a bullet (`sibling`), and
    only a bullet deeper than it is its list's: "- #401 — blocked by #398\n- #402"
    is one blocker, the next bullet another issue's line."""
    taken, in_list = 0, False
    while True:
        pos = SEP.match(text, pos).end()
        if text.startswith("\n", pos):
            pos, in_list = below(text, pos, taken, in_list, nested)
            if pos is None:
                return
            continue
        if ref := REF.match(text, pos):
            if (n := number(ref, repo)) is not None and n not in found:
                found.append(n)
            taken, pos = taken + 1, ref.end()
            if reason := REASON.match(text, pos):
                pos = reason.end()
            continue
        if not in_list:
            unread.extend(REF.finditer(text, pos, ends(text, pos)))
            return
        eol = text.find("\n", pos)
        eol = len(text) if eol < 0 else eol
        unread.extend(REF.finditer(text, pos, eol))
        pos = eol
        if pos == len(text):
            return


def below(text: str, pos: int, taken: int, in_list: bool,
          nested: int | None) -> tuple[int | None, bool]:
    """Where the list goes on after the line break at `pos`, and whether it is a
    bulleted list now; `None` where it stops."""
    blank = BLANK.match(text, pos)
    start = blank.end() if blank else pos + 1
    if bullet := BULLET.match(text, start):
        if nested is not None and len(LEAD.match(text, start).group()) <= nested:
            return None, in_list
        # A bullet right below an item, or any bullet under a bare "Blocked by:",
        # continues it; one after a blank line continues only a bulleted list.
        if not blank or in_list or not taken:
            return bullet.end(), True
        return None, in_list
    if REF.match(text, LEAD.match(text, start).end()):
        return start, in_list
    if in_list and not blank and text[start:start + 1] in (" ", "\t"):
        return start, in_list  # a wrapped bullet's next line
    return None, in_list


def ends(text: str, pos: int) -> int:
    """Where the clause holding `pos` ends: a semicolon, a sentence's full stop (not
    an abbreviation's), a blank line, or a line break before something new."""
    for stop in STOP.finditer(text, pos):
        at = stop.start()
        if text[at] == "\n":
            if BLANK.match(text, at) or (NEW_LINE.match(text, at + 1)
                                         and not OPEN_END.search(text[pos:at])):
                return at
        elif text[at] == "." and ABBREVIATION.search(text[pos:at]):
            continue
        else:
            return at
    return len(text)
