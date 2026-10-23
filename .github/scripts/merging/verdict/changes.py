"""What a pull request's changed paths say: Markdown-only, inert, or a doc tests parse.

## A head behind `main` is not refused for being behind

A run's verdict is about the tree it built, and nothing here compares the head
with `main`'s tip: GitHub's squash merge puts the change on the newer `main`
anyway. Keeping a branch current is `merge-queue.py`'s job, and it leaves a
head alone when `main` moved only by [`inert`] Markdown (#587) — so a green
run on such a head is accepted here too, and a test pins that.
"""

from __future__ import annotations


# Markdown that is code for merge purposes: code or tests read it (CLAUDE.md).
# Path prefixes: the scripting-API reference is one file per namespace (#569); a
# prefix not ending in `/` is one file, the game guide `session-mcp` serves and the
# game CLAUDE.md template the build embeds (#1313).
CODE_DOCS = ("docs/api/", "docs/game-agent.md", "src/core/project/game_claude.md")


def code_doc_example(prefix: str) -> str:
    """A path under `prefix`: a namespace page in a folder, else the file itself."""
    return f"{prefix}Transform.md" if prefix.endswith("/") else prefix


def code_doc_filter(prefix: str) -> str:
    """`prefix` as ci.yml's `code` filter writes it: a folder's glob, or the file."""
    return f"{prefix}**" if prefix.endswith("/") else prefix

# Where the doc-drift tests run: a `build-test` step whose name says it runs
# the tests with the dev features (`Test (engine, dev features)` today). Matched
# on words rather than the exact name so a rename that keeps the meaning
# (#484's nextest) keeps working; a test holds ci.yml to it.
DRIFT_JOB = "build-test"

DRIFT_STEP_WORDS = ("test", "dev")


def markdown_only(files: list[str]) -> bool:
    """Whether every changed path is Markdown — CI's own `code` test."""
    return bool(files) and all(f.endswith(".md") for f in files)


def inert(files: list[str]) -> bool:
    """Whether a change to these paths cannot alter what any branch compiles
    or tests against: Markdown, [`CODE_DOCS`] aside (#587). Nothing changed
    is inert too."""
    return all(f.endswith(".md") and not f.startswith(CODE_DOCS) for f in files)


def needs_drift_run(files: list[str]) -> bool:
    """A Markdown-only change that touches a doc tests parse (#525)."""
    return markdown_only(files) and any(f.startswith(CODE_DOCS) for f in files)


def drift_ran(run: dict, jobs: dict[int, list[dict]]) -> bool:
    """Whether this `ci` run's `build-test` really ran the dev-feature tests."""
    for job in jobs.get(run.get("id"), []):
        if job.get("name") != DRIFT_JOB:
            continue
        for step in job.get("steps") or []:
            name = step.get("name", "").lower()
            if all(w in name for w in DRIFT_STEP_WORDS) and step.get("conclusion") == "success":
                return True
    return False
