#!/usr/bin/env python3
"""`human-checks.py SINCE` — the unticked human checks of the PRs merged since then (#929).

A human check never holds a merge (issue-batch, *A human check is never a merge
hold*), so after a batch the things only the operator can verify (a real window,
speakers, how a change feels in play) are scattered across a dozen merged PRs.
scorsese's operator once waited while ~25 of them were gathered by hand from 14
PRs (MatthewLacerda2/scorsese#821); this gathers them in one call.

It reads one convention (`cloud-brief.md`, *What the description carries*): a
PR's human-only checks sit under a Markdown heading that starts **"For a
human"**, as `- [ ]` items. Every unticked item under such a heading, up to the
next heading of the same or a higher level, is listed. Ticked ones are done and
skipped. It also lists the open issues that carry both stage labels, `planning`
and `human`, because each is a stop on two counts and worth a look in the report.

SINCE is an ISO date or time (`2026-10-16`, `2026-10-16T18:00Z`) or an age
(`12h`, `3d`). The output is Markdown for the batch report; it exits 0 whatever
it finds, since it is a report, not a gate. REST only (`gh api`), so it runs
from a cloud session too, where GraphQL answers 403.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from datetime import datetime, timedelta, timezone

USAGE = "usage: human-checks.py SINCE   (2026-10-16, 2026-10-16T18:00Z, 12h or 3d)"
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")
UNTICKED = re.compile(r"^\s*[-*+]\s+\[ \]\s+(.*\S)")
STAGES = ("planning", "human")
PAGE = 100


class Gh:
    """`gh api` with JSON output parsed; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, path: str) -> object:
        done = self.runner(["gh", "api", path], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            sys.exit(f"human-checks: gh api {path}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None


def since(text: str, now: datetime) -> datetime:
    """SINCE as an aware UTC time: an age (`12h`, `3d`) back from `now`, or ISO."""
    age = re.fullmatch(r"(\d+)([hd])", text)
    if age:
        unit = "hours" if age.group(2) == "h" else "days"
        return now - timedelta(**{unit: int(age.group(1))})
    try:
        when = datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        sys.exit(f"human-checks: cannot read {text!r} as a time; {USAGE}")
    return when if when.tzinfo else when.replace(tzinfo=timezone.utc)


def stamp(text: str) -> datetime:
    return datetime.fromisoformat(text.replace("Z", "+00:00"))


def human_checks(body: str | None) -> list[str]:
    """The unticked `- [ ]` items under every heading that starts "For a human"."""
    items: list[str] = []
    level = 0  # the open "For a human" heading's level; 0 while outside one
    for line in (body or "").splitlines():
        heading = HEADING.match(line)
        if heading:
            depth, title = len(heading.group(1)), heading.group(2).strip("*_ ")
            if level and depth > level:
                continue
            level = depth if title.lower().startswith("for a human") else 0
        elif level and (item := UNTICKED.match(line)):
            items.append(item.group(1))
    return items


def merged_since(gh: Gh, repo: str, start: datetime) -> list[dict]:
    """PRs merged at or after `start`, oldest first. Closed PRs come newest-updated
    first, and a merge is an update, so paging stops at the first one older than it."""
    found: list[dict] = []
    page = 1
    while True:
        pulls = gh(f"repos/{repo}/pulls?state=closed&sort=updated&direction=desc"
                   f"&per_page={PAGE}&page={page}") or []
        found += [p for p in pulls if p.get("merged_at") and stamp(p["merged_at"]) >= start]
        if len(pulls) < PAGE or stamp(pulls[-1]["updated_at"]) < start:
            return sorted(found, key=lambda p: p["merged_at"])
        page += 1


def double_staged(gh: Gh, repo: str) -> list[dict]:
    """Open issues (not PRs) labelled both `planning` and `human`."""
    issues = gh(f"repos/{repo}/issues?state=open&labels={','.join(STAGES)}&per_page={PAGE}") or []
    return [i for i in issues if "pull_request" not in i]


def repo_name(runner=subprocess.run) -> str:
    """owner/name: `GITHUB_REPOSITORY` in CI, else the last two parts of origin's URL
    (a cloud session's origin is a proxy URL ending in the same two parts)."""
    if os.environ.get("GITHUB_REPOSITORY"):
        return os.environ["GITHUB_REPOSITORY"]
    url = runner(["git", "remote", "get-url", "origin"], capture_output=True, text=True,
                 check=False).stdout.strip()
    parts = re.split(r"[/:]", url.removesuffix(".git").rstrip("/"))
    if len(parts) < 2 or not all(parts[-2:]):
        sys.exit(f"human-checks: cannot tell the repository from origin {url!r}")
    return "/".join(parts[-2:])


def report(start: datetime, pulls: list[dict], issues: list[dict]) -> str:
    when = start.strftime("%Y-%m-%d %H:%M UTC")
    lines = [f"## For a human: checks from PRs merged since {when}", ""]
    checked = [(p, human_checks(p.get("body"))) for p in pulls]
    waiting = [(p, items) for p, items in checked if items]
    if not waiting:
        lines.append(f"None: {len(pulls)} PR(s) merged, no unticked human check among them.")
    for pull, items in waiting:
        lines += ["", f"### #{pull['number']} {pull['title']}", ""]
        lines += [f"- [ ] {item}" for item in items]
    lines += ["", "## Issues labelled both `planning` and `human`", ""]
    if not issues:
        lines.append("None.")
    lines += [f"- #{i['number']} {i['title']}" for i in issues]
    return "\n".join(lines)


def main(argv: list[str], gh: Gh | None = None, repo: str | None = None,
         now: datetime | None = None) -> int:
    if len(argv) != 1:
        sys.exit(USAGE)
    start = since(argv[0], now or datetime.now(timezone.utc))
    gh, repo = gh or Gh(), repo or repo_name()
    print(report(start, merged_since(gh, repo, start), double_staged(gh, repo)))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
