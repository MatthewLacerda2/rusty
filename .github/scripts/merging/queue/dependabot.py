"""Dependabot's branches: asked to rebase themselves, never force-pushed ([`bot_head`]).

**Dependabot rebases itself** (and cancels its own runs as it does). A
force-push from here would make Dependabot stop maintaining the branch, so a
bot pull request that is behind is asked to rebase — `@dependabot rebase` — and
the queue waits for a head that sits on `main`'s tip ([`bot_head`]), then
judges that head like any other. The rebase wait and the CI wait share **one**
`--deadline` ([`left`], #825): two deadlines back to back would let one bot pull
request outlast the watch's `--for` budget (`ci-merge` has the arithmetic).
"""

from __future__ import annotations

import argparse
import subprocess
import time

import ghrest
from merging.queue import outcomes, pulls, update


BOT_REBASE = "@dependabot rebase"


def left(deadline_minutes: float, spent: float) -> float:
    """Seconds of a `--deadline` still unspent after `spent` seconds, never
    negative: what a bot pull request's CI wait gets once [`bot_head`] has
    waited for Dependabot's rebase out of the same clock (#825)."""
    return max(0.0, deadline_minutes * 60 - spent)


def bot_head(number: int, head: str, root: str, opts: argparse.Namespace) -> tuple[str | None, list[str]]:
    """Dependabot's own rebase, waited for: a head on `main`'s tip, or why not.

    Never pushed to from here — Dependabot stops maintaining a branch
    somebody else pushed to. Asked once with [`BOT_REBASE`], then polled.
    """
    if update.on_tip(head, root) or update.main_moved_by_docs(head, root):
        return head, []
    if opts.dry_run:
        return head, [f"would comment `{BOT_REBASE}` and wait for Dependabot's rebase."]
    # Not the fatal `gh`: if the comment fails, the wait below still answers.
    subprocess.run(
        ["gh", "api", f"repos/{ghrest.slug()}/issues/{number}/comments", "-f", f"body={BOT_REBASE}"],
        capture_output=True, check=False,
    )
    began = time.monotonic()
    while time.monotonic() - began < opts.deadline * 60:
        time.sleep(opts.poll)
        seen = pulls.look(number).get("headRefOid", "")
        if seen != head and update.on_tip(seen, root):
            return seen, []
        outcomes.say(f"#{number}: waiting for Dependabot to rebase {head[:7]}.")
    return None, [f"Dependabot did not rebase {head[:7]} onto `main` within {opts.deadline:.0f} minutes."]
