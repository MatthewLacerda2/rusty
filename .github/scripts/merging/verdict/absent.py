"""Why a commit has no run at all, told apart ([`no_run`]).

## A commit with no run at all

Two causes, told apart in the output ([`no_run`]): scorsese#153 above, and a
branch that conflicts with `main` — GitHub cannot compute a merge ref, so it
creates no run, no check and no error (scorsese#429, 45 minutes lost to editing
workflow YAML). An invalid workflow *does* produce a run, a `startup_failure`;
no run whatsoever is a conflict.

Branch protection is not the alternative it looks like: GitHub treats a skipped
required check as satisfied in some configurations, and a draft's run is
exactly that — this asks the question directly.
"""

from __future__ import annotations


# What GitHub calls a branch it cannot merge, in the two fields that say so.
# Both are read, because either alone has been seen to lag the other.
CONFLICTED = ("CONFLICTING", "DIRTY")

# Not "no": *not computed yet*. A fresh push reads UNKNOWN for a moment, and
# asserting "not conflicted" from it is a confident wrong answer.
UNKNOWN = "UNKNOWN"


def in_rollup(pull: dict, workflow: str) -> bool:
    """Whether the pull request's own checks show `workflow` on its head.

    `statusCheckRollup` is a second witness to the runs listing: when it names
    the workflow and the listing does not, the listing is lagging (#592).
    """
    return any(c.get("workflowName") == workflow for c in pull.get("statusCheckRollup") or [])


def no_run(pull: dict, short: str, workflow: str) -> list[str]:
    """Why this commit has no `workflow` run at all — a lagging listing (#592),
    scorsese#153, or #429.

    Told apart here, because the reader is already looking at this output.
    """
    state, status = pull.get("mergeable"), pull.get("mergeStateStatus")
    if in_rollup(pull, workflow):
        return [
            f"no `{workflow}` run is listed for the head commit {short} — yet.",
            f"The pull request's checks show `{workflow}` on this commit, so"
            " the run exists: the runs listing is eventually consistent and"
            " is lagging (#592). Ask again in a moment.",
            "Do not push an empty commit: it would cancel the run that is there.",
        ]
    head = f"no `{workflow}` run exists for the head commit {short}."

    if state in CONFLICTED or status in CONFLICTED:
        return [
            f"{head} The branch conflicts with `main`.",
            "That is the cause and not a coincidence: GitHub cannot compute a"
            " merge ref for a conflicted branch, so it never evaluates the"
            " workflow's triggers and creates no run, no check and no error"
            " anywhere. The workflow file is fine — do not edit it.",
            "Rebase onto `main` and force-push; the merge routine asks for that"
            " rebase anyway.",
        ]

    lines = [
        head,
        "Marking a pull request ready right after a push can lose the run"
        " entirely (scorsese#153). The checks are not green, they are absent.",
        "Force one with an empty commit, or draft and ready it again with a"
        " pause in between.",
        "Before editing any workflow YAML: an invalid one still produces a run,"
        " a `startup_failure`. No run whatsoever means a conflict with `main`,"
        " not a syntax error.",
    ]
    if state == UNKNOWN:
        # Said out loud: the conflict branch above could not fire, and silence
        # would read as "checked, and it is not that".
        lines.append(
            "GitHub has not finished computing whether this branch merges"
            " cleanly. Ask again in a moment before believing the rest."
        )
    elif status and status != "CLEAN":
        lines.append(f"GitHub reports this branch as {status}.")
    return lines
