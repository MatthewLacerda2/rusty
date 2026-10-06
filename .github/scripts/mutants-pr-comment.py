#!/usr/bin/env python3
"""Post an on-request mutation report on the branch's pull request (#912).

`mutants-on-request.yml` runs this after rendering `report.md`. The report
already reaches whoever waits on the run (`make mutants-remote` prints it, a
cloud coder reads the log), but a cloud coder ends at *ready* and is gone by
the time the run finishes. The pull request is where the orchestrator and the
reviewer look, so the report goes there too: CLAUDE.md's *Gates vs. signals*
says a signal only earns its keep where the agent acts on it.

- **One comment per pull request, kept current.** The comment opens with
  [`MARKER`]; a later run on the same branch edits that comment instead of
  stacking another, so the pull request shows the latest answer once.
- **No open pull request, no comment.** A by-hand run on `main` or a scratch
  branch has nowhere to post, and that is not an error.
- **Never longer than GitHub takes.** A comment holds [`LIMIT`] characters; a
  longer report is cut at a line and points to the run, which has all of it.

A failure here is a failed comment, not a failed measurement: the workflow
runs this with `continue-on-error`, so it can never turn the run red.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

#: Hidden first line that marks the comment this script owns.
MARKER = "<!-- mutants-on-request -->"

#: GitHub refuses a comment body longer than this many characters.
LIMIT = 65536

#: Only comments by the workflow's own token are ever edited.
BOT = "github-actions[bot]"


def body(report: str, run_url: str, limit: int = LIMIT) -> str:
    """The comment: marker, report, and a line naming the run it came from."""
    head = f"{MARKER}\n"
    foot = f"\n\n---\nFrom [this run]({run_url}).\n"
    text = report.rstrip("\n")
    if len(head) + len(text) + len(foot) <= limit:
        return head + text + foot
    cut = f"\n\n*The report is cut here to fit a comment; [the run]({run_url}) has all of it.*"
    room = limit - len(head) - len(cut) - len(foot)
    kept = text[:room]
    if "\n" in kept:
        kept = kept[: kept.rfind("\n")]
    return head + kept + cut + foot


def ours(comments: list[dict]) -> int | None:
    """The id of the comment an earlier run posted, or `None` if there is none."""
    for comment in comments:
        if comment.get("user") == BOT and comment.get("body", "").startswith(MARKER):
            return comment["id"]
    return None


def gh(*args: str, stdin: str | None = None) -> str:
    done = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, check=False)
    if done.returncode != 0:
        sys.exit(f"mutants-pr-comment: gh {' '.join(args[:3])}: {done.stderr.strip()}")
    return done.stdout


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(prog="mutants-pr-comment.py", description=__doc__.split("\n")[0])
    parser.add_argument("report", type=Path, help="the rendered report.md")
    parser.add_argument("--repo", required=True, help="owner/name")
    parser.add_argument("--branch", required=True, help="the dispatched ref's branch name")
    parser.add_argument("--run-url", required=True)
    args = parser.parse_args(argv)

    prs = json.loads(gh("pr", "list", "--repo", args.repo, "--head", args.branch,
                        "--state", "open", "--json", "number"))
    if not prs:
        print(f"mutants-pr-comment: no open pull request from {args.branch}; nothing to post")
        return 0
    number = prs[0]["number"]
    lines = gh("api", "--paginate", f"repos/{args.repo}/issues/{number}/comments",
               "--jq", ".[] | {id: .id, user: .user.login, body: .body}")
    existing = ours([json.loads(line) for line in lines.splitlines() if line.strip()])
    payload = json.dumps({"body": body(args.report.read_text(), args.run_url)})
    if existing is None:
        gh("api", "-X", "POST", f"repos/{args.repo}/issues/{number}/comments",
           "--input", "-", stdin=payload)
        print(f"mutants-pr-comment: posted the report on #{number}")
    else:
        gh("api", "-X", "PATCH", f"repos/{args.repo}/issues/comments/{existing}",
           "--input", "-", stdin=payload)
        print(f"mutants-pr-comment: updated the report on #{number}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
