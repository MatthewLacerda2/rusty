"""What GitHub says about a commit, as plain dictionaries: the verdict's test fixtures.

A run, its jobs, and the pull request around them, each a genuine build by
default and overridden field by field.
"""

from __future__ import annotations

from merging.verdict import decide, runlist


SHA = "c23b7c4f0000000000000000000000000000000a"

READY = {
    "isDraft": False,
    "headRefOid": SHA,
    "number": 524,
    "mergeable": "MERGEABLE",
    "mergeStateStatus": "CLEAN",
}

CODE = ["src/app/mod.rs"]


def run(workflow: str = "ci", **fields: object) -> dict:
    """A completed, successful run of `workflow` on [`SHA`], overridden by field."""
    return {
        "name": workflow,
        "head_sha": SHA,
        "id": 1 if workflow == "ci" else 2,
        "created_at": "2026-09-30T01:00:00Z",
        "status": "completed",
        "conclusion": "success",
        "html_url": "https://example.invalid/run/1",
    } | fields


def job(name: str, conclusion: str, steps: list[dict] | None = None) -> dict:
    return {"name": name, "conclusion": conclusion, "steps": steps or []}


def built(workflow: str) -> list[dict]:
    """The jobs of a run that genuinely checked a code change."""
    if workflow == "ci":
        return [
            job("changes", "success"),
            job("build-test", "success", [{"name": "Test (engine, dev features)", "conclusion": "success"}]),
            job("build-test-cross (macos-latest)", "success"),
            job("deny", "success"),
            job("ci-gate", "success"),
            job("coverage", "skipped"),
        ]
    return [job("changes", "success"), job("lint", "success"), job("lint-gate", "success")]


def drafted(workflow: str) -> list[dict]:
    """A draft's run: `changes` and the gate green, every gated job skipped."""
    gate, gated = runlist.WORKFLOWS[workflow]
    return [job("changes", "success"), *(job(g, "skipped") for g in gated), job(gate, "success")]


def verdict(*runs: dict, jobs: dict | None = None, files=CODE, pull=None):
    """[`decide.judge`], each run's jobs defaulting to a genuine build."""
    jobs = jobs if jobs is not None else {r["id"]: built(r["name"]) for r in runs}
    return decide.judge(pull or READY, list(runs), jobs, files)


def both(**ci: object) -> tuple[dict, dict]:
    return run("ci", **ci), run("lint")


def gates_running() -> list[dict]:
    """A `ci` run whose gated jobs, and so its gate, are still going."""
    return [job("changes", "success"), job("build-test", None), job("deny", "success"), job("ci-gate", None)]


def signalling(conclusion: str | None) -> list[dict]:
    """A `ci` run whose gates concluded green beside `mutants-pr` in `conclusion`."""
    return [*built("ci"), job("mutants-pr", conclusion), job("coverage-pr", "success")]
