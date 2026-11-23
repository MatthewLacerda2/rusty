"""Which model a cloud coder runs on, and why (#1726, #2490, #2630).

The operator's rule (#2630): almost every issue is Opus's; a minor one can go to
a smaller model; a documentation-only one is the orchestrator's own (CLAUDE.md,
*The orchestrator writes issues and briefs, never code (a doc or doctrine PR
aside)*). A label is visible on the board and set where the issue is triaged,
so the labels decide, and the body's `Model:` line, the operator's decision for
one job (#2490), overrides them:

- **A `Model:` line wins** ([`model_lines`]): a whole line `Model: <word>`
  outside a fenced block, the word one of [`MODELS`]. Any other word, or two
  lines, is refused: a guess would launch the wrong coder.
- **[`MINOR`] runs on Sonnet** (*small, mechanical, no design choice*), named
  for the reason rather than the model, so it stays true when the models change.
  A `minor` issue whose round came back twice ([`ESCALATE`] round briefs by
  someone who can push, `rebriefs.rounds`) runs on Opus again, and says so:
  a small model that missed twice is the wrong model for it.
- **Everything else runs on [`MODEL`]**, the latest Opus.
- **Documentation-only is not dispatched** ([`stop`]): an issue whose only kind
  label ([`KINDS`]) is `documentation`, with no `Model:` line, is refused
  unless forced (`make coder ARGS=--force`). With `minor` on it too the two
  signals disagree (a Sonnet coder, or none), and it is refused naming both.

[`choose`] answers the model and the reason, which `make coder` prints (its
dry run too) and the launch says.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from merging.watch import board
from merging.watch.order import PRIORITY

#: The model a coder runs on when nothing asks for another (#1726). A new generation
#: changes this line and [`MODELS`], nothing else.
MODEL = "claude-opus-5-5"
SONNET = "claude-sonnet-5-5"

#: What an issue's `Model:` line may ask for (#2490).
MODELS = {"opus": MODEL, "sonnet": SONNET}

#: The label for a small, mechanical issue with no design choice: its coder runs on Sonnet.
MINOR = "minor"

#: How many round briefs move a `minor` issue back to Opus: its review sent it back twice.
ESCALATE = 2

#: The kind labels of CLAUDE.md's table; an issue whose only one is `documentation` is the
#: orchestrator's to write.
DOCUMENTATION = "documentation"
KINDS = (DOCUMENTATION, *PRIORITY)

#: A line of its own in the issue body, `Model: <name>`, ignoring case and spaces.
MODEL_LINE = re.compile(r"[ \t]*Model:[ \t]*(\S*)[ \t]*", re.IGNORECASE)
FENCE = re.compile(r"[ \t]*(```|~~~)")


@dataclass(frozen=True)
class Choice:
    model: str
    why: str


def model_lines(issue_body: str) -> list[str]:
    """The word of each whole `Model: <word>` line outside a fenced block; a line
    with more after the word is prose, not the line."""
    asked, fenced = [], False
    for line in issue_body.splitlines():
        if FENCE.match(line):
            fenced = not fenced
        elif not fenced and (found := MODEL_LINE.fullmatch(line)):
            asked.append(found.group(1))
    return asked


def asked(issue: dict) -> str | None:
    """The model the issue's `Model:` line names, or `None` without one.

    Raises `ValueError` naming the accepted values for a line that names no model
    in [`MODELS`], or for two lines."""
    lines = model_lines(issue.get("body") or "")
    if not lines:
        return None
    accepted = ", ".join(sorted(MODELS))
    if len(lines) > 1:
        raise ValueError(f"the issue has {len(lines)} `Model:` lines; keep one, naming one of {accepted}")
    name = lines[0].strip().lower()
    if name not in MODELS:
        raise ValueError(f"the issue's `Model: {lines[0].strip()}` names no model; write one of {accepted}, "
                         "or drop the line for the labels' choice")
    return name


def choose(issue: dict, back: int = 0) -> Choice:
    """The model #`issue`'s coder runs on and why: its `Model:` line, else its labels; `back` is
    how many round briefs it has had (`rebriefs.rounds`)."""
    if (name := asked(issue)) is not None:
        return Choice(MODELS[name], f"the issue's `Model: {name}` line")
    if MINOR not in board.names(issue.get("labels")):
        return Choice(MODEL, "the default")
    if back >= ESCALATE:
        return Choice(MODEL, f"labelled `{MINOR}`, but its review sent it back {back} times: back on Opus")
    return Choice(SONNET, f"labelled `{MINOR}`")


def stop(issue: dict) -> str | None:
    """Why #`issue` is not dispatched (documentation-only, no `Model:` line), or `None`."""
    labels = board.names(issue.get("labels"))
    if set(KINDS) & labels != {DOCUMENTATION} or model_lines(issue.get("body") or ""):
        return None
    if MINOR in labels:
        return (f"labelled `{DOCUMENTATION}` alone and `{MINOR}`: one says the orchestrator writes it, the "
                f"other a Sonnet coder; drop one label, or pass --force for the `{MINOR}` coder")
    return (f"its only kind label is `{DOCUMENTATION}`: the orchestrator writes it (CLAUDE.md, the orchestrator "
            "writes a doc PR itself); --force launches a coder anyway")
