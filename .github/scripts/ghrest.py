"""GitHub over REST, the way every script here reaches it (#970).

`gh pr …`, `gh issue …` and `gh repo …` speak GraphQL, and a Claude Code cloud
session's proxy answers GraphQL with a 403 — while cloud sessions are the
default coders (#924). So the scripts call `gh api repos/{o}/{r}/…` only, and
`tests/test_rest_only.py` refuses a GraphQL subcommand coming back.

Three things that GraphQL gave for free live here instead:

- [`slug`]: the repository, from `git remote get-url origin` (`gh repo view`).
- [`pages`]: every page of a list, asked for with an explicit `page=`, because
  `gh api --paginate` failed through the proxy too.
- [`pull`]: a REST pull request in the field names the merge path was written
  against (`isDraft`, `headRefOid`, …), so its decisions did not change.

Stdlib only; each function takes the `gh` caller its script already has.
"""

from __future__ import annotations

import functools
import re
import subprocess
import sys
from typing import Callable

# GitHub's closing keywords, then `#N` in this repository. GitHub matches them
# anywhere in a description, quoted or negated alike.
CLOSING = re.compile(r"\b(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\b:?\s+#(\d+)\b", re.IGNORECASE)

# The last two path parts of a remote URL: https, ssh, or a proxy's `/git/o/r`.
REMOTE = re.compile(r"[/:]([^/:]+)/([^/]+?)(?:\.git)?/?$")

PER_PAGE = 100


def slug_of(url: str) -> str | None:
    """`owner/name` from a remote URL, or `None` when it names no repository."""
    match = REMOTE.search(url.strip())
    return f"{match.group(1)}/{match.group(2)}" if match else None


@functools.cache
def slug() -> str:
    """This checkout's repository, `owner/name`, read once from `origin`."""
    done = subprocess.run(["git", "remote", "get-url", "origin"], capture_output=True, text=True, check=False)
    found = slug_of(done.stdout) if done.returncode == 0 else None
    if found is None:
        sys.exit(f"cannot tell the repository from `git remote get-url origin`: {done.stdout or done.stderr}".strip())
    return found


def pages(get: Callable[[str], object], path: str, key: str | None = None) -> list | None:
    """Every item of a REST list, page by page; `None` when a page did not come.

    `get` is the caller's `gh api` (it returns the parsed JSON, or `None` when it
    swallowed a failure); `key` names the list inside an object answer
    (`workflow_runs`, `check_runs`). Stops at the first short page.
    """
    items: list = []
    joiner = "&" if "?" in path else "?"
    for page in range(1, 1000):
        answer = get(f"{path}{joiner}per_page={PER_PAGE}&page={page}")
        if answer is None:
            return None
        batch = answer.get(key, []) if key else answer
        items += batch
        if len(batch) < PER_PAGE:
            return items
    return items


MERGEABLE = {True: "MERGEABLE", False: "CONFLICTING", None: "UNKNOWN"}


def pull(rest: dict, files: list[dict] | None = None, checks: list[dict] | None = None) -> dict:
    """A REST pull request in the GraphQL field names the merge path reads.

    REST has no `closingIssuesReferences`; the keywords in the body stand in for
    it (a sidebar-only link is not seen). `files` and `checks` are separate
    calls, passed in when the caller read them.
    """
    body = rest.get("body") or ""
    shaped = {
        "number": rest.get("number"),
        "title": rest.get("title", ""),
        "body": body,
        "state": "MERGED" if rest.get("merged_at") else str(rest.get("state", "")).upper(),
        "mergedAt": rest.get("merged_at"),
        "isDraft": bool(rest.get("draft")),
        "headRefOid": (rest.get("head") or {}).get("sha", ""),
        "headRefName": (rest.get("head") or {}).get("ref", ""),
        "baseRefName": (rest.get("base") or {}).get("ref", ""),
        "mergeable": MERGEABLE.get(rest.get("mergeable"), "UNKNOWN"),
        "mergeStateStatus": str(rest.get("mergeable_state") or "unknown").upper(),
        "createdAt": rest.get("created_at"),
        "author": {"login": (rest.get("user") or {}).get("login", "")},
        "labels": [{"name": label.get("name", "")} for label in rest.get("labels") or []],
        "closingIssuesReferences": [{"number": n} for n in sorted({int(m) for m in CLOSING.findall(body)})],
    }
    if files is not None:
        shaped["files"] = [{"path": f.get("filename", ""), "status": f.get("status"),
                            "additions": f.get("additions", 0), "deletions": f.get("deletions", 0)} for f in files]
    if checks is not None:
        shaped["statusCheckRollup"] = checks
    return shaped
