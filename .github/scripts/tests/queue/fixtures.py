"""A pull request as the queue reads it, and the helpers its tests share.

[`READY`] extends the verdict's fixture with the fields only the queue reads.
"""

from __future__ import annotations

import subprocess

from merging.queue import polling
from tests.verdict import fixtures
from tests.verdict.fixtures import CODE, built


READY = {
    **fixtures.READY,
    "headRefName": "486-mergeable-queue",
    "state": "OPEN",
    "title": "Port make mergeable and make queue (#486)",
    "author": {"login": "MatthewLacerda2"},
}


def asked(*runs: dict, waited: float = 0.0, jobs=None, pull=None, files=CODE):
    """[`polling.progress`] over these runs, each a genuine build by default."""
    jobs = jobs if jobs is not None else {r["id"]: built(r["name"]) for r in runs}
    return polling.progress(pull or READY, list(runs), jobs, files, waited)


SPENT = polling.RUN_APPEARS_SECONDS + 1


def completed(code: int, stderr: str = "") -> subprocess.CompletedProcess:
    return subprocess.CompletedProcess([], code, "", stderr)
