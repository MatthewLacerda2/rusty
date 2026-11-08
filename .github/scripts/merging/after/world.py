"""The queue's hook for scoped mutation after a merge, bound to GitHub ([`Mutations`], #1934)."""

from __future__ import annotations

import contextlib
import fcntl
import json
import subprocess
import time
from pathlib import Path

from merging.after import ask, slots
from merging.verdict import github

WORKFLOW = "mutants-on-request.yml"


def gh(*args: str, stdin: str | None = None) -> tuple[bool, str]:
    """`gh`'s success and output (its stderr on failure); never raises, never exits."""
    try:
        done = subprocess.run(["gh", *args], input=stdin, capture_output=True, text=True, check=False)
    except OSError as failed:
        return False, str(failed)
    return done.returncode == 0, done.stdout if done.returncode == 0 else done.stderr.strip()


def why(text: str) -> str:
    """A failed call's reason, [`slots.Busy`] when it was the network's (`verdict.github.transient`)."""
    text = text or "no reason given"
    return slots.Busy(text) if github.transient(text) else text


class Mutations:
    """What the watch calls: [`merged`] after each merge, [`pump`] on each pass.

    Every effect fails soft: GitHub not answering leaves the requests as they
    are, to be tried on the next pass, and never stops a merge.

    The file is the state, not this object: a named take (`make queue PRS=N`)
    beside a running watch shares it, and each kept its own copy until the
    independent review of PR #1935 ran three at once and lost a request. So
    every call takes the file's lock, reads it, changes it and writes it back.
    """

    def __init__(self, repo: str, path: Path, say, gh=gh, clock=time.time):
        self.repo, self.path, self.say, self.gh, self.clock = repo, path, say, gh, clock
        self.state = slots.load(path)  # each call reads it again, and says what it drops

    def merged(self, number: int) -> None:
        """Note that pull request `number` merged, then pump: its description
        is read there, so a failed read is retried like every other call."""
        with self.locked():
            if not slots.known(self.state, number):
                self.state["unread"].append({"pr": number})
                self.keep()
            self.pumped()

    def pump(self) -> None:
        """Read the merged pull requests, free the finished runs' slots and dispatch into the open ones."""
        with self.locked():
            self.pumped()

    def pumped(self) -> None:
        if not any(self.state[key] for key in slots.KEYS):
            return  # nothing asked: no call at all
        changed = self.read()
        runs = self.runs() if self.state["running"] else []
        if slots.pump(self.state, runs, self.dispatch, self.clock(), self.say) or changed:
            self.keep()

    def read(self) -> bool:
        """Turn each merged pull request into its request, or none; whether anything changed."""
        kept, changed = [], False
        for entry in self.state["unread"]:
            ok, text = self.gh("api", f"repos/{self.repo}/pulls/{entry['pr']}")
            pull = parsed((ok, text))
            if not isinstance(pull, dict) or not pull.get("merge_commit_sha"):
                changed = True
                if not slots.failed(entry, why(text if not ok else "no merge commit"), "description", self.say):
                    kept.append(entry)
                continue
            changed = True
            self.ask(entry["pr"], pull)
        self.state["unread"] = kept
        return changed

    def ask(self, number: int, pull: dict) -> None:
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

    def files(self, commit: str) -> list[str] | None:
        """The paths a commit changed (GitHub's first page of them), or `None` when unread."""
        got = parsed(self.gh("api", f"repos/{self.repo}/commits/{commit}"))
        if not isinstance(got, dict):
            return None
        return [f.get("filename", "") for f in got.get("files") or [] if isinstance(f, dict)]

    def runs(self) -> list[dict] | None:
        got = parsed(self.gh("api", f"repos/{self.repo}/actions/workflows/{WORKFLOW}/runs"
                                    "?branch=main&event=workflow_dispatch&per_page=50"))
        runs = got.get("workflow_runs") if isinstance(got, dict) else None
        return runs if isinstance(runs, list) else None

    def dispatch(self, entry: dict, request: str) -> str | None:
        inputs = ask.inputs(entry["scope"], entry["commit"], entry["pr"], request)
        ok, text = self.gh("api", "-X", "POST", f"repos/{self.repo}/actions/workflows/{WORKFLOW}/dispatches",
                           "--input", "-", stdin=json.dumps({"ref": "main", "inputs": inputs}))
        return None if ok else why(text)

    @contextlib.contextmanager
    def locked(self):
        """The file's lock held, and the state read fresh under it. A lock that
        cannot be taken is said and gone without: one signal, never a stopped queue."""
        try:
            self.path.parent.mkdir(parents=True, exist_ok=True)
            handle = open(self.path.with_suffix(".lock"), "a")
        except OSError as failed:
            self.say(f"after: could not lock the mutation requests: {failed}")
            handle = None
        try:
            if handle is not None:
                fcntl.flock(handle, fcntl.LOCK_EX)
            dropped: list[str] = []
            self.state = slots.load(self.path, lambda line: (dropped.append(line), self.say(line)))
            if dropped:
                self.keep()  # written back clean, so a bad entry is said once
            yield
        finally:
            if handle is not None:
                handle.close()  # closing releases the lock

    def keep(self) -> None:
        failed = slots.save(self.path, self.state)
        if failed:
            self.say(f"after: could not save the mutation requests: {failed}")


def parsed(answer: tuple[bool, str]) -> object | None:
    ok, text = answer
    if not ok:
        return None
    try:
        return json.loads(text)
    except ValueError:
        return None
