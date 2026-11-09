"""Whether a pull request carries its review decision (#1764).

On 2026-11-03 the orchestrator labelled #1753, #1754, #1757 and #1758 `queue`
on its own read of each diff, though each met `independent-review`'s triggers;
it pulled the labels before any merged, and the reviews then sent all four back
with must-fix findings (an audible blip on unpause, false API docs). Only a line
of the skill's *Common mistakes* guarded it. Now the queue's take refuses a pull
request that does not say what was decided ([`refusal`]), before any push:

- **A comment whose first line is `Independent review…`** ([`REVIEWED`]): the
  record `independent-review`, *Answering the findings*, has the orchestrator
  write; or **`No review: <reason>`** ([`WAIVED`]), the written exemption, with
  a reason on the same line (an empty one does not count). A path table cannot
  decide when a review is needed (`make review-level` sets only its level), so
  the written decision is what is read.
- **Only from someone who can push** ([`ghrest.can_push`]): the repository is
  public, and anyone may comment.
- **Not needed** ([`exempt`]) for what CI's `code` filter counts as non-code
  (Markdown, the docs tests parse aside: [`changes.inert`], held to `ci.yml` by
  `test_workflow_hygiene.py`), and for Dependabot's cargo bumps, which the
  skill says need no review: its author *and* its `dependabot/cargo/` branch,
  so a person's branch never qualifies, and its Actions bumps (workflows, a
  trigger that wins) still need the record.

Two limits, left to prose on purpose (PR #1767's review). **The outcomes are
not read**: any `Independent review` comment counts, open must-fix findings or
not, since a record lists findings in free prose and is routinely posted while
the coder is still fixing; so `independent-review`, step 2, has the label taken
off until they are answered. **A decision is not tied to a head**: a `No
review: data only` stays valid after a later push; the orchestrator's read
before labelling is the guard.

Pure but for [`comments`], which reads them over REST (#970).
"""

from __future__ import annotations

import re

import ghrest
from merging.verdict import changes, github


# The first non-blank line of a review record: the skill's heading, as a
# heading, bold or plain (`## Independent review`, `**Independent review**: …`,
# `Independent review (high: …)`). The words end the line or are followed by
# `(`, `:` or a dash, so `Independent review pending` is not a record.
REVIEWED = re.compile(r"^[#*_\s]*Independent review[*_]*[ \t]*(?:$|[(:—–-])", re.IGNORECASE)

# The written exemption, its reason on the same line: `No review: why`,
# `**No review:** why` or `**No review**: why`.
WAIVED = re.compile(r"^[#*_\s]*No review(?:\*\*|__)?:[*_]*[ \t]*(.*?)\s*$", re.IGNORECASE)

# Dependabot's cargo branches, beside its author ([`exempt`]). Its GitHub Actions
# bumps (`dependabot/github_actions/…`) edit `.github/workflows/` or
# `.github/actions/`, which
# `independent-review` reviews whoever wrote it, so they are not exempt.
BOT_BRANCH = "dependabot/cargo/"


def first_line(body: str) -> str:
    return next((line for line in (body or "").splitlines() if line.strip()), "")


def decides(body: str) -> bool:
    """Whether a comment body records the decision: a review, or a reasoned waiver."""
    line = first_line(body)
    waived = WAIVED.match(line)
    return bool(REVIEWED.match(line)) or bool(waived and waived.group(1).strip("*_ \t"))


def decided(comments: list[dict]) -> bool:
    """Whether any comment by someone who can push records the decision."""
    return any(ghrest.can_push(c) and decides(c.get("body") or "") for c in comments)


def exempt(pull: dict, bot: bool) -> bool:
    """Whether this pull request needs no decision: non-code only, or Dependabot's own."""
    files = github.paths(pull)
    if files and changes.inert(files):
        return True
    return bot and (pull.get("headRefName") or "").startswith(BOT_BRANCH)


def refusal(pull: dict, comments: list[dict], bot: bool = False) -> list[str]:
    """Why the queue may not take this pull request yet; empty when it may.
    The first line is the verdict, as with every refusal on the merge path."""
    if exempt(pull, bot) or decided(comments):
        return []
    unheard = [c for c in comments if decides(c.get("body") or "") and not ghrest.can_push(c)]
    lines = ["no comment records its review decision (#1764)."]
    if unheard:
        who = sorted({(c.get("user") or {}).get("login", "?") for c in unheard})
        lines.append(f"Not counted: by {', '.join(who)}, who cannot push here.")
    lines.append(
        "Before the `queue` label, someone who can push comments `Independent review` (the"
        " `independent-review` skill's record) or `No review: <why>` on its first line;"
        " the watch takes it again once one appears, no push needed."
    )
    return lines


def comments(repo: str, number: int, fetch=None) -> list[dict]:
    """Every comment on pull request `number`'s conversation (REST, #970)."""
    fetch = fetch or github.gh
    return ghrest.pages(lambda path: fetch("api", path), f"repos/{repo}/issues/{number}/comments") or []
