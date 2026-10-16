"""The pull request as GitHub describes it, and the reads that tolerate a blip."""

from __future__ import annotations

import json
import subprocess

from merging.queue import outcomes
from merging.verdict import github, runlist
from merging.watch import board, handbacks


# How `gh pr view --json author` names Dependabot, and the command it obeys.
BOTS = ("app/dependabot", "dependabot[bot]", "dependabot")


def is_bot(pull: dict) -> bool:
    """Whether Dependabot owns this branch (and so rebases it itself)."""
    return (pull.get("author") or {}).get("login") in BOTS


def subject(pull: dict) -> str:
    """The squash commit's title: the PR title carrying `(#N)`, once."""
    title, tag = pull.get("title", "").strip(), f"(#{pull.get('number')})"
    return title if title.endswith(tag) else f"{title} {tag}"


def look(number: int) -> dict:
    """The pull request, in the fields every step below reads."""
    return github.gh(
        "pr", "view", str(number), "--json",
        github.PULL_FIELDS + ",state,title,author",
    )


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
    issues = quietly("issue", "list", "--state", "open", "--limit", "1000", "--json", "number,labels") or []
    return {i["number"]: board.names(i.get("labels")) for i in issues}


def head_runs(repo: str, sha: str) -> str | None:
    """[`watch.handbacks.attempts`] of the gating runs on `sha`, `None` when GitHub
    failed: what tells a re-run of a handed-back head from no news (#869)."""
    listed = quietly("api", f"repos/{repo}/actions/runs?head_sha={sha}&per_page=100")
    if listed is None:
        return None
    runs = listed.get("workflow_runs", [])
    return handbacks.attempts([r for w in runlist.WORKFLOWS for r in runlist.runs_for(runs, sha, w)])
