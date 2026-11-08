"""GitHub as a script for [`after.world.Mutations`]: pull requests, commits and runs."""

from __future__ import annotations

import json


class GitHub:
    """Answers `gh api` the way GitHub would; records every dispatch."""

    def __init__(self, pulls: dict[int, dict], files: dict[str, list[str]] | None = None):
        self.pulls, self.files, self.runs, self.dispatched, self.refuse = pulls, files or {}, [], [], 0

    def __call__(self, *args: str, stdin: str | None = None) -> tuple[bool, str]:
        path = next(a for a in args if a.startswith("repos/"))
        if path.endswith("/dispatches"):
            if self.refuse:
                self.refuse -= 1
                return False, "HTTP 422: no"
            sent = json.loads(stdin)
            self.dispatched.append(sent)
            self.runs.insert(0, {"display_title": f"mutants: x [{sent['inputs']['request']}]", "status": "queued"})
            return True, ""
        if "/actions/workflows/" in path:
            return True, json.dumps({"workflow_runs": self.runs})
        if "/commits/" in path:
            sha = path.rsplit("/", 1)[1]
            return True, json.dumps({"files": [{"filename": f} for f in self.files.get(sha, ["src/a.rs"])]})
        number = int(path.rsplit("/", 1)[1])
        return (True, json.dumps(self.pulls[number])) if number in self.pulls else (False, "HTTP 404")

    def finish(self, number: int) -> None:
        """The dispatched run for pull request `number` completes."""
        for run in self.runs:
            if f"[pr{number}-" in run["display_title"]:
                run.update(status="completed", conclusion="success")


def merged(number: int, body: str = "Mutation: diff", sha: str | None = None) -> dict:
    return {"number": number, "body": body, "merge_commit_sha": sha or f"c{number}"}
