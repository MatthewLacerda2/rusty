#!/usr/bin/env python3
"""`board.py [--hours N]` — what on the board has gone stale unnoticed (#971).

CLAUDE.md opens a PR on a branch's first commit "so a branch is never a stray
… and never goes stale unnoticed", but nothing noticed: the enforcement audit
(#933) found two `claude/*` branches three months old with commits and no PR
ever. This is the noticing. It lists:

- **stray**: a branch ahead of `main` that never had a pull request, once its
  last commit is older than `--hours` (a coder pushes before it opens the PR);
- **merged, not deleted**: a branch whose PR merged and that still exists
  (GitHub's *automatically delete head branches* setting removes the class);
- **name**: a branch that is neither `{issue}-short-slug` nor a short slug,
  lowercase and hyphenated (Dependabot's own names are exempt);
- **stale assignment**: an open issue assigned more than `--hours` (24) ago
  with no branch `{issue}-…` and no open PR naming it, since an assignee means
  "in progress" (CLAUDE.md, *Assign yourself when you start*);
- **no type label**: an open issue carrying none of the type labels the
  priority order sorts by. `planning` issues are exempt: still being shaped;
- **kind** (#1173): an open `bug` issue with no `Kind:` line, a `Kind: case`
  that names no search, or a `Kind: #N` whose sub-issue parent is not N. About
  twelve patches of one kind of bug landed before anyone filed its root fix
  (#1170), because nothing made the filer look for earlier cases. `planning`
  bugs are exempt, as above.

What/Why headings are not checked: CLAUDE.md leaves how an issue reads to
Claude (#922 retired the template). The one exception is the `Kind:` line on a
bug issue (#1173), which the operator decided.

A **signal**, not a gate: it is about the board, and no merge waits on it. It
runs weekly (`.github/workflows/board.yml`, into the job summary) and as `make
board`. It exits 1 on findings and 2 when it could not run; the workflow goes
red only on 2, so a red run means the audit broke, as with `coverage.yml`. Exceptions are
inline with a reason: a branch in `KEEP` below, an issue whose body carries
`<!-- board: keep, <reason> -->`. Both print their reason, so none is silent.
REST only (`gh api`), so it runs from a cloud session too.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from urllib.parse import quote

USAGE = "usage: board.py [--hours N]"
PAGE = 100
NAME = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")
TYPES = {"infrastructure", "architecture", "bug", "foundation", "feature", "documentation"}
ISSUE_KEEP = re.compile(r"<!--\s*board:\s*keep,\s*(.+?)\s*-->")
KIND = re.compile(r"^Kind:[ \t]*(.*)$", re.MULTILINE)
KIND_ROOT = re.compile(r"#(\d+)\b")
KIND_CASE = re.compile(r"case\b.*\bsearched\b\W*\w")

# Branches the board never flags, each with why. Empty is the goal.
KEEP: dict[str, str] = {}


@dataclass
class Branch:
    name: str
    ahead: int = 0
    behind: int = 0
    prs: list[str] = field(default_factory=list)  # "open" | "merged" | "closed"
    last_commit: datetime | None = None  # its newest commit not on main


@dataclass
class Issue:
    number: int
    title: str
    labels: set[str]
    body: str = ""
    assigned_at: datetime | None = None  # the latest `assigned` event; None: unassigned
    parent: int | None = None  # its sub-issue parent's number


def fail(why: str) -> None:
    """Exit 2: the audit could not run, which is not a finding (exit 1)."""
    print(f"board: {why}", file=sys.stderr)
    raise SystemExit(2)


def name_ok(name: str) -> bool:
    return name == "main" or name.startswith("dependabot/") or NAME.fullmatch(name) is not None


def branch_findings(branches: list[Branch], now: datetime, hours: int) -> list[str]:
    found = []
    for b in branches:
        if b.name == "main":
            continue
        if b.name in KEEP:
            print(f"board: keeping {b.name}: {KEEP[b.name]}", file=sys.stderr)
            continue
        settled = b.last_commit is None or now - b.last_commit > timedelta(hours=hours)
        if b.ahead and not b.prs and settled:
            found.append(f"stray: `{b.name}`: {b.ahead} ahead, {b.behind} behind main, no PR ever")
        if "merged" in b.prs and "open" not in b.prs:
            found.append(f"merged, not deleted: `{b.name}`")
        if not name_ok(b.name):
            found.append(f"name: `{b.name}` is not `{{issue}}-short-slug` or a lowercase-hyphenated slug")
    return found


def issue_findings(issues: list[Issue], branches: list[Branch], open_prs: list[dict],
                   now: datetime, hours: int) -> list[str]:
    found = []
    for i in issues:
        if keep := ISSUE_KEEP.search(i.body or ""):
            print(f"board: keeping #{i.number}: {keep.group(1)}", file=sys.stderr)
            continue
        if "planning" not in i.labels and not i.labels & TYPES:
            found.append(f"no type label: #{i.number} {i.title}")
        if i.assigned_at and now - i.assigned_at > timedelta(hours=hours) \
                and not worked(i.number, branches, open_prs):
            age = int((now - i.assigned_at).total_seconds() // 3600)
            found.append(f"stale assignment: #{i.number} {i.title}: assigned {age}h ago, "
                         f"no branch `{i.number}-…` and no open PR naming it")
        if "bug" in i.labels and "planning" not in i.labels and (why := kind_problem(i)):
            found.append(f"kind: #{i.number} {i.title}: {why}")
    return found


def kind_problem(i: Issue) -> str | None:
    """What is wrong with a bug issue's `Kind:` line, or None (#1173)."""
    lines = KIND.findall(i.body or "")
    if len(lines) != 1:
        return "no `Kind:` line" if not lines else f"{len(lines)} `Kind:` lines"
    kind = lines[0].strip()
    if root := KIND_ROOT.match(kind):
        if i.parent != int(root.group(1)):
            return f"`Kind: #{root.group(1)}` but its sub-issue parent is " + \
                (f"#{i.parent}" if i.parent else "none")
        return None
    if re.match(r"root\b", kind):
        return None
    if kind.startswith("case"):
        return None if KIND_CASE.match(kind) else "`Kind: case` names no search"
    return f"`Kind: {kind}` is not `#N`, `case — searched <terms>` or `root`"


def worked(number: int, branches: list[Branch], open_prs: list[dict]) -> bool:
    """Is there a branch or an open PR for this issue?"""
    prefix = f"{number}-"
    mention = re.compile(rf"#{number}\b")
    return any(b.name.startswith(prefix) for b in branches) or any(
        p["head"]["ref"].startswith(prefix)
        or mention.search(f"{p.get('title') or ''}\n{p.get('body') or ''}")
        for p in open_prs)


class Gh:
    """`gh api` with JSON output parsed; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, path: str) -> object:
        done = self.runner(["gh", "api", path], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            fail(f"gh api {path}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None

    def pages(self, path: str) -> list[dict]:
        found, page = [], 1
        while True:
            batch = self(f"{path}{'&' if '?' in path else '?'}per_page={PAGE}&page={page}") or []
            found += batch
            if len(batch) < PAGE:
                return found
            page += 1


def stamp(text: str) -> datetime:
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def gather(gh: Gh, repo: str) -> tuple[list[Branch], list[Issue], list[dict]]:
    owner = repo.split("/")[0]
    branches = []
    for raw in gh.pages(f"repos/{repo}/branches"):
        b = Branch(raw["name"])
        if b.name != "main":
            ref = quote(b.name, safe="")
            diff = gh(f"repos/{repo}/compare/main...{ref}")
            b.ahead, b.behind = diff["ahead_by"], diff["behind_by"]
            if diff.get("commits"):
                b.last_commit = stamp(diff["commits"][-1]["commit"]["committer"]["date"])
            pulls = gh(f"repos/{repo}/pulls?state=all&head={owner}:{ref}&per_page={PAGE}") or []
            b.prs = ["merged" if p.get("merged_at") else p["state"] for p in pulls]
        branches.append(b)
    issues = []
    for raw in gh.pages(f"repos/{repo}/issues?state=open"):
        if "pull_request" in raw:
            continue
        issue = Issue(raw["number"], raw["title"], {lb["name"] for lb in raw["labels"]},
                      raw.get("body") or "")
        if parent := raw.get("parent_issue_url"):
            issue.parent = int(parent.rsplit("/", 1)[1])
        if raw.get("assignees"):
            events = gh.pages(f"repos/{repo}/issues/{issue.number}/events")
            times = [stamp(e["created_at"]) for e in events if e["event"] == "assigned"]
            issue.assigned_at = max(times) if times else None
        issues.append(issue)
    return branches, issues, gh.pages(f"repos/{repo}/pulls?state=open")


def repo_name(runner=subprocess.run) -> str:
    """owner/name: `GITHUB_REPOSITORY` in CI, else the last two parts of origin's URL
    (a cloud session's origin is a proxy URL ending in the same two parts)."""
    if os.environ.get("GITHUB_REPOSITORY"):
        return os.environ["GITHUB_REPOSITORY"]
    url = runner(["git", "remote", "get-url", "origin"], capture_output=True, text=True,
                 check=False).stdout.strip()
    parts = re.split(r"[/:]", url.removesuffix(".git").rstrip("/"))
    if len(parts) < 2 or not all(parts[-2:]):
        fail(f"cannot tell the repository from origin {url!r}")
    return "/".join(parts[-2:])


def main(argv: list[str], gh: Gh | None = None, repo: str | None = None,
         now: datetime | None = None) -> int:
    hours = 24
    if argv:
        if len(argv) != 2 or argv[0] != "--hours" or not argv[1].isdigit():
            fail(USAGE)
        hours = int(argv[1])
    gh, repo = gh or Gh(), repo or repo_name()
    branches, issues, open_prs = gather(gh, repo)
    now = now or datetime.now(timezone.utc)
    found = branch_findings(branches, now, hours) + issue_findings(issues, branches, open_prs, now, hours)
    print(f"## Board: {len(found)} finding(s)\n")
    print("\n".join(f"- {line}" for line in found) if found else
          f"Nothing stale: {len(branches)} branch(es), {len(issues)} open issue(s).")
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
