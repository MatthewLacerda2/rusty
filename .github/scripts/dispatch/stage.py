"""Where a pull request's review loop stands, and the record the shepherd posts (#2629).

The shepherd (`.claude/agents/shepherd.md`) is one fresh cloud session per
turn of a pull request's review loop, with nothing carried in its memory: each
start reads the pull request and its comments and works out what to do from
them alone ([`where`]), so a restart, or a second start on the same head, does
no harm. The answer is one of:

- [`REVIEW`]: no review record yet (`merging/verdict/reviewed.decides`): the
  first pass, a fresh review or a `No review: <why>`;
- [`REREAD`]: the newest record's verdict (`merging/verdict/standing.said`)
  was given on an older head, and commits of its own followed it (`current`
  `False`, `standing.merges_only`): a round came back, or code was pushed past
  a `queue`; the pass asks only whether it delivered what the record asked and
  made a new problem. [`Stage.rounds`] counts the `round` verdicts already
  given, since a third round is never sent (`Verdict: hold` instead);
- [`STOP`], with why: closed or merged, a draft (a coder holds it), the `hold`
  label, a `hold` verdict (a decision for the user), a verdict on this head (or
  one only merges followed) already, a record with no verdict line (someone
  decided by hand), a head whose history could not be read, or a pull request
  that needs no record (`reviewed.exempt`).

[`record`] composes what the shepherd posts: its text, then the verdict line
`standing.line` writes, on the head read now, so the machine line the queue
reads is never typed by a model. A `round` names its `Brief` comment on its own
line ([`ROUND_BRIEF`]), which the quiet watch reads to relaunch the coder
(`merging/view/shepherding.py`). It refuses a text that `reviewed.decides` would
not take as a record, and a verdict word outside `queue|round|hold`.

Pure; `shepherd.py` reads GitHub for it.
"""

from __future__ import annotations

import re
from dataclasses import dataclass

from merging.verdict import reviewed, standing

REVIEW, REREAD, STOP = "review", "reread", "stop"

#: The `hold` label: a decision someone made by hand (`watch.board.HOLD_LABEL`).
HOLD_LABEL = "hold"

#: The line a `round` record carries, naming the `Brief` that sends the coder back.
ROUND_BRIEF = re.compile(r"^[ \t]*Round brief:[ \t]*(https://github\.com/[\w.-]+/[\w.-]+/issues/(\d+)"
                         r"#issuecomment-\d+)[ \t\r]*$", re.M)

#: How many `round` verdicts a pull request may be given; the next pass holds instead.
ROUNDS = 2


@dataclass(frozen=True)
class Stage:
    what: str
    why: str
    rounds: int = 0


def rounds(comments: list[dict]) -> int:
    """How many trusted records ended in a `round` verdict."""
    return sum(1 for c in standing.records(comments)
               if (found := standing.VERDICT.findall(c.get("body") or "")) and found[-1][0].lower() == standing.ROUND)


def where(pull: dict, comments: list[dict], current: bool | None) -> Stage:
    """Where `pull`'s loop stands. `current` is `standing.merges_only` for the
    newest verdict's head against `pull`'s head (`None` unread or not asked)."""
    head = str(pull.get("headRefOid") or "")[:7]
    if pull.get("state") not in (None, "OPEN"):
        return Stage(STOP, f"it is {str(pull.get('state')).lower()}")
    if pull.get("isDraft"):
        return Stage(STOP, "it is a draft: a coder holds it")
    if HOLD_LABEL in {(label or {}).get("name") for label in pull.get("labels") or []}:
        return Stage(STOP, "it carries the `hold` label: someone decided by hand")
    sent = rounds(comments)
    verdict = standing.said(comments)
    if verdict is None:
        if standing.records(comments):
            return Stage(STOP, "its newest record has no `Verdict:` line: someone decided by hand", sent)
        if reviewed.exempt(pull, False):
            return Stage(STOP, "it changes no code (`reviewed.exempt`): it needs no record", sent)
        return Stage(REVIEW, f"no review record: review {head}", sent)
    word, on = verdict
    if word == standing.HOLD:
        return Stage(STOP, f"its verdict is `hold` on {on[:7]}: a decision for the user", sent)
    if current is None:
        return Stage(STOP, f"GitHub did not show what followed its verdict on {on[:7]}; start again later", sent)
    if current:
        return Stage(STOP, f"its `{word}` verdict on {on[:7]} holds for {head}: nothing to do", sent)
    return Stage(REREAD, f"commits of its own followed its `{word}` verdict on {on[:7]}: read {head}", sent)


def record(text: str, word: str, head: str, brief: str | None = None) -> str:
    """The record to post: `text`, the `Round brief:` line for a `round`, and the verdict line on `head`.

    Raises `ValueError` saying why for a text that is no record, a word that is
    no verdict, a `round` without its brief (or another word with one), or a
    text that already carries a verdict or brief line."""
    word = word.lower()
    if word not in (standing.QUEUE, standing.ROUND, standing.HOLD):
        raise ValueError(f"--verdict takes queue, round or hold, not {word!r}")
    if not reviewed.decides(text):
        raise ValueError("the record's first line must be `Independent review…` or `No review: <why>`"
                         " (`merging/verdict/reviewed.py`), or the queue does not read it")
    if standing.VERDICT.search(text) or ROUND_BRIEF.search(text):
        raise ValueError("the text carries its own `Verdict:` or `Round brief:` line; leave both to this script")
    if (word == standing.ROUND) != bool(brief):
        raise ValueError("a `round` names its `Brief` comment (--brief URL), and only a `round` does")
    lines = [text.rstrip()]
    if brief:
        lines.append(f"Round brief: {brief}")
        if not ROUND_BRIEF.search(lines[-1]):
            raise ValueError(f"{brief!r} is not the URL of a comment on an issue")
    lines.append(standing.line(word, head))
    return "\n\n".join(lines[:1] + ["\n".join(lines[1:])]) + "\n"
