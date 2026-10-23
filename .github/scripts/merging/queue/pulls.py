"""The pull request as GitHub describes it, and the reads that tolerate a blip."""

from __future__ import annotations

import json
import subprocess

import ghrest
from merging.queue import outcomes
from merging.verdict import closing, github, runlist
from merging.watch import board, handbacks


# How GitHub names Dependabot as an author (REST says `dependabot[bot]`).
BOTS = ("app/dependabot", "dependabot[bot]", "dependabot")


def is_bot(pull: dict) -> bool:
    """Whether Dependabot owns this branch (and so rebases it itself)."""
    return (pull.get("author") or {}).get("login") in BOTS


def subject(pull: dict) -> str:
    """The squash commit's title, house style: `Title (#issue) (#pr)`.

    Each issue it closes that the title lacks is added (#966: `Bump rustls …
    (#943)` came from branch `919-…`), then the pull request's own number, once.
    """
    tag = f"(#{pull.get('number')})"
    title = pull.get("title", "").strip().removesuffix(tag).rstrip()
    for n in closing.referenced(pull):
        if f"(#{n})" not in title:
            title += f" (#{n})"
    return f"{title} {tag}"


def look(number: int) -> dict:
    """The pull request, in the fields every step below reads (REST, #970)."""
    return github.pull(ghrest.slug(), number)


def quietly(*args: str) -> object | None:
    """`gh`'s parsed JSON, or `None` when it fails: a watch outlives a blip."""
    done = subprocess.run(["gh", *args], capture_output=True, text=True, check=False)
    if done.returncode != 0:
        outcomes.say(f"gh {args[0]} {args[1]} failed: {done.stderr.strip()}")
        return None
    try:
        return json.loads(done.stdout)
    except json.JSONDecodeError:
        return None


def issue_labels() -> dict[int, set[str]]:
    """Every open issue's labels: the priority [`watch.order.rank`] reads."""
    issues = ghrest.pages(lambda path: quietly("api", path), f"repos/{ghrest.slug()}/issues?state=open") or []
    return {i["number"]: board.names(i.get("labels")) for i in issues if "pull_request" not in i}


def open_pulls(repo: str) -> list[dict] | None:
    """The open pull requests as the watch lists them, `None` when GitHub failed.

    Shaped by [`ghrest.pull`]. Only a bot's carries `files`: [`watch.board.held`]
    is all that reads them, and a call per pull request per poll is not free.
    """
    listed = ghrest.pages(lambda path: quietly("api", path), f"repos/{repo}/pulls?state=open")
    if listed is None:
        return None
    shaped = []
    for rest in listed:
        pull = ghrest.pull(rest)
        if is_bot(pull):
            files = ghrest.pages(lambda path: quietly("api", path), f"repos/{repo}/pulls/{pull['number']}/files")
            if files is None:
                return None
            pull = ghrest.pull(rest, files)
        shaped.append(pull)
    return shaped


def issues_block(repo: str, numbers: tuple[int, ...]) -> list[int] | None:
    """Which of `numbers` still refuse a merge ([`verdict.closing.blocking`]),
    `None` when GitHub failed: what re-admits an `AWAITS_ISSUE` hand-back (#1279)."""
    closes = {}
    for n in numbers:
        issue = quietly("api", f"repos/{repo}/issues/{n}")
        if issue is None:
            return None
        if issue.get("state") == "open":
            blockers = quietly("api", f"repos/{repo}/issues/{n}/dependencies/blocked_by?per_page=100")
            if blockers is None:
                return None
            issue = {**issue, "blocked_by": blockers}
        closes[n] = issue
    return closing.blocking(closes)


def head_of(repo: str, number: int) -> str | None:
    """The head commit of pull request `number`, `None` when GitHub failed."""
    return ((quietly("api", f"repos/{repo}/pulls/{number}") or {}).get("head") or {}).get("sha")


def head_runs(repo: str, sha: str) -> str | None:
    """[`watch.handbacks.attempts`] of the gating runs on `sha`, `None` when GitHub
    failed: what tells a re-run of a handed-back head from no news (#869)."""
    listed = quietly("api", f"repos/{repo}/actions/runs?head_sha={sha}&per_page=100")
    if listed is None:
        return None
    runs = listed.get("workflow_runs", [])
    return handbacks.attempts([r for w in runlist.WORKFLOWS for r in runlist.runs_for(runs, sha, w)])
