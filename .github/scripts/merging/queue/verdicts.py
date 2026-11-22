"""Each listed pull request's review standing, read from GitHub (#2628).

[`verdict.standing`] decides; this reads what it decides on: the pull request's
comments (REST, paged), and for a `queue` verdict given on an older head, the
compare of `main...head` that says whether only merges followed it.

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
from merging.verdict import standing

# How long a pull request's comments are trusted unchanged when its `updatedAt` has not moved.
FRESH_SECONDS = 600


class Reader:
    """`fetch(path)` is `gh api path`, parsed, or `None` when it failed."""

    def __init__(self, repo: str, fetch: Callable[[str], object], clock: Callable[[], float] = time.monotonic):
        self.repo, self.fetch, self.clock = repo, fetch, clock
        self.comments: dict[int, tuple[str | None, float, list[dict]]] = {}
        self.compares: dict[tuple[str, str], bool] = {}

    def read(self, pull: dict) -> dict | None:
        """[`standing.standing`] of `pull`, `None` when GitHub failed."""
        comments = self.said(pull)
        if comments is None:
            return None
        verdict = standing.said(comments)
        current = True
        if verdict and verdict[0] == standing.QUEUE:
            current = self.current(verdict[1], pull.get("headRefOid", ""))
            if current is None:
                return None
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

    def attach(self, pulls: list[dict]) -> list[dict] | None:
        """`pulls` with each ready one based on `main` carrying its `standing`; `None` when GitHub failed."""
        shaped = []
        for pull in pulls:
            if not pull.get("isDraft") and pull.get("baseRefName", "main") == "main":
                read = self.read(pull)
                if read is None:
                    return None
                pull = {**pull, "standing": read}
            shaped.append(pull)
        return shaped

