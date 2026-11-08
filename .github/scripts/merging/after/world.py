"""The queue's hook for scoped mutation after a merge, bound to GitHub ([`Mutations`], #1934)."""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

from merging.after import ask, slots

WORKFLOW = "mutants-on-request.yml"


def gh(*args: str, stdin: str | None = None) -> tuple[bool, str]:
    """`gh`'s success and output (its stderr on failure); never raises, never exits."""
    try:
        done = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, check=False)
    except OSError as failed:
        return False, str(failed)
    return done.returncode == 0, done.stdout if done.returncode == 0 else done.stderr.strip()


def parsed(answer: tuple[bool, str]) -> object | None:
    ok, text = answer
    if not ok:
        return None
    try:
        return json.loads(text)
    except ValueError:
        return None


class Mutations:
    """What the watch calls: [`merged`] after each merge, [`pump`] on each pass.

    Every effect fails soft: GitHub not answering leaves the requests as they
    are, to be tried on the next pass, and never stops a merge.
    """

    def __init__(self, repo: str, path: Path, say, gh=gh, clock=time.time):
        self.repo, self.path, self.say, self.gh, self.clock = repo, path, say, gh, clock
        self.state = slots.load(path)

    def merged(self, number: int) -> None:
        """Queue the run pull request `number` asked for, if it asked, then pump."""
        pull = parsed(self.gh("api", f"repos/{self.repo}/pulls/{number}"))
        if not isinstance(pull, dict) or not pull.get("merge_commit_sha"):
            self.say(f"after: could not read merged #{number}; no mutation run is asked for it.")
            return
        scope, commit = ask.requested(pull.get("body")), pull["merge_commit_sha"]
        if scope is None:
            return
        if scope == "diff":
            files = self.files(commit)
            if files is not None and not ask.rust(files):
                self.say(f"after: #{number} asked for a mutation run, but its commit changed no Rust; none dispatched.")
                return
        if slots.enqueue(self.state, number, scope, commit):
            self.say(f"after: #{number} asked for a mutation run ({scope}); in line.")
            self.keep()
        self.pump()

    def files(self, commit: str) -> list[str] | None:
        """The paths a commit changed (GitHub's first page of them), or `None` when unread."""
        got = parsed(self.gh("api", f"repos/{self.repo}/commits/{commit}"))
        if not isinstance(got, dict):
            return None
        return [f.get("filename", "") for f in got.get("files") or []]

    def runs(self) -> list[dict] | None:
        got = parsed(self.gh("api", f"repos/{self.repo}/actions/workflows/{WORKFLOW}/runs"
                                    "?branch=main&event=workflow_dispatch&per_page=50"))
        return got.get("workflow_runs") if isinstance(got, dict) else None

    def dispatch(self, entry: dict, request: str) -> str | None:
        inputs = ask.inputs(entry["scope"], entry["commit"], entry["pr"], request)
        ok, text = self.gh("api", "-X", "POST", f"repos/{self.repo}/actions/workflows/{WORKFLOW}/dispatches",
                           "--input", "-", stdin=json.dumps({"ref": "main", "inputs": inputs}))
        return None if ok else (text or "no reason given")

    def pump(self) -> None:
        """Free the finished runs' slots and dispatch into the open ones."""
        if not (self.state["waiting"] or self.state["running"]):
            return  # nothing asked: no call at all
        runs = self.runs() if self.state["running"] else []
        if slots.pump(self.state, runs, self.dispatch, self.clock(), self.say):
            self.keep()

    def keep(self) -> None:
        failed = slots.save(self.path, self.state)
        if failed:
            self.say(f"after: could not save the mutation requests: {failed}")
