"""Each listed pull request's review standing, read from GitHub (#2628).

[`verdict.standing`] decides; this reads what it decides on: the pull request's
comments (REST, paged), and for a verdict given on an older head, the compare
of `main...head` that says whether only merges followed it: a `queue` needs it
to stand, and the quiet watch ([`view.quiet`]) to tell a round that came back.

**A comment read that fails fails the listing**, so a verdict is never read as
absent. **A compare that fails does not**: that pull request reads as not
queued by its verdict (`current` `None`, [`standing.effective`]), its label
still deciding, and the log names it once per head ([`Reader.unread`]); the
next listing asks again.

The watch lists the open pull requests on every poll, so the reads are kept
([`Reader`]): the comments by the pull request's `updatedAt` (a new comment
moves it), re-read after [`FRESH_SECONDS`] all the same; the compare by its
two heads, which a later read cannot change. Drafts and a pull request not
based on `main` are not read: the watch never takes them, and one that turns
ready is read on the listing that shows it ready.
"""

from __future__ import annotations

import time
from typing import Callable

import ghrest
from merging.queue import outcomes
from merging.verdict import standing

# How long a pull request's comments are trusted unchanged when its `updatedAt` has not moved.
FRESH_SECONDS = 600

# The standing of a pull request whose comments a re-read before a merge could
# not get ([`Reader.again`]): no go-ahead, label or not ([`watch.board.queued`]).
UNREAD = {"verdict": None, "said": None, "on": None, "current": None, "owed": [], "unread": True}


class Reader:
    """`fetch(path)` is `gh api path`, parsed, or `None` when it failed."""

    def __init__(self, repo: str, fetch: Callable[[str], object], clock: Callable[[], float] = time.monotonic,
                 say: Callable[[str], None] = outcomes.say):
        self.repo, self.fetch, self.clock, self.say = repo, fetch, clock, say
        self.comments: dict[int, tuple[str | None, float, list[dict]]] = {}
        self.compares: dict[tuple[str, str], bool] = {}
        self.unread: set[tuple[int, str]] = set()

    def read(self, pull: dict) -> dict | None:
        """[`standing.standing`] of `pull`, `None` when its comments could not be read."""
        comments = self.said(pull)
        if comments is None:
            return None
        verdict, current = standing.said(comments), None
        if verdict:
            number, head = pull.get("number"), pull.get("headRefOid", "")
            current = self.current(verdict[1], head)
            if current is None and (number, head) not in self.unread:
                self.unread.add((number, head))
                self.say(f"#{number}: GitHub did not show what followed its `{verdict[0]}` verdict on {verdict[1][:7]}"
                         " (the compare of main...head), so it is not queued by that verdict; asked again on the next listing.")
        return standing.standing(comments, current)

    def said(self, pull: dict) -> list[dict] | None:
        number, updated, now = pull.get("number"), pull.get("updatedAt"), self.clock()
        kept = self.comments.get(number)
        if kept and updated and kept[0] == updated and now - kept[1] < FRESH_SECONDS:
            return kept[2]
        listed = ghrest.pages(self.fetch, f"repos/{self.repo}/issues/{number}/comments")
        if listed is None:
            return None
        self.comments[number] = (updated, now, listed)
        return listed

    def current(self, since: str, head: str) -> bool | None:
        if head.startswith(since):
            return True
        if (since, head) not in self.compares:
            compared = self.fetch(f"repos/{self.repo}/compare/main...{head}")
            if not isinstance(compared, dict):
                return None
            self.compares[(since, head)] = standing.merges_only(
                compared.get("commits") or [], int(compared.get("ahead_by") or 0), since, head)
        return self.compares[(since, head)]

    def again(self, pull: dict) -> dict:
        """`pull` re-read before a merge, with its standing. When its comments
        cannot be read, it carries [`UNREAD`], which no label overrides: a
        `Needs:` posted since the last read would be missed, so it goes back in
        line and is read again on the next pass, never merged on a doubt."""
        shaped = self.attach([pull])
        if shaped:
            return shaped[0]
        return {**pull, "standing": UNREAD}

    def attach(self, pulls: list[dict]) -> list[dict] | None:
        """`pulls` with each ready one based on `main` carrying its `standing`;
        `None` when a pull request's comments could not be read."""
        shaped = []
        for pull in pulls:
            if not pull.get("isDraft") and pull.get("baseRefName", "main") == "main":
                read = self.read(pull)
                if read is None:
                    return None
                pull = {**pull, "standing": read}
            shaped.append(pull)
        return shaped

