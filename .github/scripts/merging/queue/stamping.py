"""Dating a pull request's Gates line when the queue rebases it (#968).

A coder's description says what ran green, on which commit ("all green on
`79e180a`"). Every rebase the queue pushes makes that a claim about a commit
that will not merge: 31 of 33 merged pull requests sampled for #968 named a
head other than the one that landed. The claim was true when made; it needs a
date, and the coder never sees the rebase. So after each push the queue adds
one line to the description saying which `main` it rebased onto and that CI,
not the local gates, judged the new head. A second rebase replaces the line,
never adds another. A signal, not a refusal: a failure to write it is said and
the take goes on.
"""

from __future__ import annotations

import json
import subprocess

from merging.queue import gitio, outcomes, pulls


# How the line begins, which is how a later rebase finds and replaces it.
PREFIX = "rebased by the queue onto"


def line(main: str, head: str) -> str:
    """The line itself: which `main`, and which head CI judges."""
    return f"_{PREFIX} `{main[:7]}`; local gates not re-run; CI judged `{head[:7]}`._"


def stamped(body: str | None, new: str) -> str:
    """`body` with `new` as its one queue line, at the end; an earlier one is
    replaced, so two rebases leave one line naming the latest."""
    kept = [row for row in (body or "").rstrip().split("\n") if not row.strip().strip("_").startswith(PREFIX)]
    return "\n".join(kept).rstrip() + "\n\n" + new + "\n"


def stamp(repo: str, pull: dict, fresh: str, root: str) -> None:
    """Write [`line`] into the pull request's description after a push of
    `fresh`. Dependabot's descriptions are its own, and are left alone."""
    if pulls.is_bot(pull):
        return
    number = pull.get("number")
    main = gitio.git("rev-parse", "--verify", "origin/main", cwd=root).stdout.strip()
    current = pulls.quietly("api", f"repos/{repo}/pulls/{number}")
    if not main or current is None:
        outcomes.say(f"#{number}: could not date the Gates line; the description still names its old head.")
        return
    body = stamped(current.get("body"), line(main, fresh))
    done = subprocess.run(
        ["gh", "api", "-X", "PATCH", f"repos/{repo}/pulls/{number}", "--input", "-"],
        input=json.dumps({"body": body}), capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        outcomes.say(f"#{number}: could not date the Gates line: {done.stderr.strip()}")
