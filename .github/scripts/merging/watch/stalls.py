"""A head handed back for outwaiting its runs, and what brings it back (#1831).

During GitHub's Actions incidents of 2026-11-04, PR #1820's macOS run waited
past the queue's deadline, so the take ended "Nothing is red; nothing is green
either" ([`outcomes.STALLED`]). The run then went green, and nothing brought
the pull request back: a hand-back waits for its head to move or run again
(#869), and a run *ending* is neither. #1827 followed it an hour later. Each
sat until someone re-ran CI by hand, which spends a macOS runner during the
very outage that stalled it.

So a stall is its own kind of hand-back, remembered in
[`handbacks.STALLS_MEMORY`], and the watch asks about it each listing
([`returned`]) by the runs' **state**, read the way the CI wait reads it
([`verdict.decide.assess`], lag included: a run listed `completed` whose jobs
don't yet agree is still out, #1095, #1182):

- **every gating run concluded success**: back in line, once;
- **a run still out**: still passed over, nothing said;
- **anything else** (a cancelled run, a red one, a run gone from the
  listing): still passed over, said once with its reason, and from then on an
  ordinary hand-back: an orchestrator's re-run moves its attempt and the
  #869 mark brings it back. No re-run is made here: a stall ending cancelled
  during an outage would otherwise hold the queue's one slot for a deadline
  per cycle, without limit.

It is a check on state, not on a change of mark, so an ending seconds after
the hand-back is not lost. GitHub not answering (`None`) is no news. A red,
conflict or review hand-back is never asked: those wait for their head.
"""

from __future__ import annotations

from merging.verdict import decide, runlist


# What [`reading`] makes of the runs on a stalled head.
GREEN, OUT, ENDED = "green", "out", "ended"


def reading(pull: dict, runs: list[dict], jobs: dict[int, list[dict]]) -> tuple[str, str]:
    """Whether the runs on `pull`'s head are all green, still out, or ended
    otherwise, with the words for the last ([`decide.assess`] per workflow)."""
    sha = pull.get("headRefOid", "")
    states = [decide.assess(pull, w, runlist.runs_for(runs, sha, w), jobs) for w in runlist.WORKFLOWS]
    if all(state == decide.PASSED for state, _ in states):
        return GREEN, ""
    failed = [lines for state, lines in states if state == decide.FAILED]
    if not failed and any(state == decide.RUNNING for state, _ in states):
        return OUT, ""
    lines = failed[0] if failed else next(lines for state, lines in states if state != decide.PASSED)
    return ENDED, " ".join(lines[:2])


def returned(pulls: list[dict], memory: dict[int, str], stalls: dict[int, str], ask) -> tuple[list[int], list[tuple[int, str]]]:
    """The remembered stalls whose runs went green, and those that ended
    otherwise with why. `ask(pull)` is [`reading`] of its head's runs now, or
    `None` when GitHub failed (no news). Asked only for ready pull requests
    whose head is the remembered one."""
    back, ended = [], []
    for pull in pulls:
        number = pull.get("number")
        if number not in stalls or pull.get("isDraft") or memory.get(number) != pull.get("headRefOid"):
            continue
        now = ask(pull)
        if now is None or now[0] == OUT:
            continue
        if now[0] == GREEN:
            back.append(number)
        else:
            ended.append((number, now[1]))
    return back, ended
