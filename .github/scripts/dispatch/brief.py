"""The newest `Brief` comment on an issue, and the model it names (#1569).

A coder's brief is a comment on its issue whose first line is `Brief` (#910);
a later one supersedes an earlier one, so the newest wins. Its `Model:` line
(`issue-batch`, *Which model*) is the one place the model is written down: a
label like `feature` or `bug` says nothing about the shape of the task, so no
label stands in for it (the decision on #1569).

The repository is public, so a comment that starts with `Brief` from someone
who cannot push is not a brief: the coder would follow it, so the launch is
refused instead.
"""

from __future__ import annotations

import re

#: Models a brief may name, to the id a routine takes. A new model generation
#: changes the ids here, nothing else.
MODELS = {"opus": "claude-opus-5-5", "sonnet": "claude-sonnet-5-5"}

#: Who may brief a coder: people who can push to the repository.
TRUSTED = {"OWNER", "MEMBER", "COLLABORATOR"}

#: Stage labels: `planning` and `human` mean *not yet* (CLAUDE.md).
STOPS = {"planning", "human"}

FIRST_LINE = re.compile(r"^\W*Brief\b")
MODEL_LINE = re.compile(r"^\W*Model\W*:\W*([A-Za-z]+)", re.MULTILINE)


class Refused(ValueError):
    """Why this issue cannot be launched; said to the operator as is."""


def is_brief(body: str) -> bool:
    lines = [line for line in (body or "").splitlines() if line.strip()]
    return bool(lines) and bool(FIRST_LINE.match(lines[0]))


def newest(comments: list[dict]) -> dict | None:
    """The newest comment that is a brief, by creation time."""
    briefs = [c for c in comments if is_brief(c.get("body", ""))]
    return max(briefs, key=lambda c: c.get("created_at", ""), default=None)


def model_of(body: str) -> str:
    """The model id the brief's `Model:` line names; [`Refused`] without one."""
    found = MODEL_LINE.findall(body or "")
    if not found:
        raise Refused("the brief has no `Model:` line (Opus or Sonnet, issue-batch *Which model*)")
    name = found[-1].lower()
    if name not in MODELS:
        raise Refused(f"the brief's `Model:` line names {found[-1]!r}; it must be one of "
                      + ", ".join(m.capitalize() for m in MODELS))
    return MODELS[name]


def read(issue: dict, comments: list[dict]) -> tuple[dict, str]:
    """The brief to launch on and its model id, or [`Refused`] saying why not."""
    number = issue.get("number")
    if "pull_request" in issue:
        raise Refused(f"#{number} is a pull request; brief its issue")
    if issue.get("state") != "open":
        raise Refused(f"#{number} is {issue.get('state')}; a coder works an open issue")
    stops = STOPS & {label.get("name") for label in issue.get("labels") or []}
    if stops:
        raise Refused(f"#{number} is labelled {', '.join(sorted(stops))}: not startable (CLAUDE.md, stage labels)")
    brief = newest(comments)
    if brief is None:
        raise Refused(f"#{number} has no comment starting with `Brief` (issue-batch, the brief template)")
    if brief.get("author_association") not in TRUSTED:
        who = (brief.get("user") or {}).get("login", "?")
        raise Refused(f"the newest brief on #{number} is by {who}, who cannot push here; "
                      "post the brief again yourself")
    return brief, model_of(brief.get("body", ""))
