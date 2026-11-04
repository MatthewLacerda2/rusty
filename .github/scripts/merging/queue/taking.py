"""One pull request, from where it is to merged or handed back ([`take`])."""

from __future__ import annotations

import argparse
import functools
import time

from merging.queue.conflict import record
from merging.queue import checks, dependabot, gitio, landing, outcomes, pulls, refusal, stamping, update, waiting
from merging.verdict import carried, closing, decide, github, protocol, reviewed
from merging.watch import handbacks


def dry(repo: str, pull: dict, fresh: str, notes: list[str]) -> tuple[int, str, str]:
    """The dry run's report: what would happen, and the verdict on today's head."""
    number, head = pull["number"], pull["headRefOid"]
    if notes and not update.push_needed(head, fresh):
        # Dependabot's branch, behind `main` ([`dependabot.bot_head`]), or a head `main`
        # passed only by Markdown ([`update.advance`]): the note says what happens.
        # A sorted-list resolution's note ([`update.resolved_merge`]) moves the head,
        # so it is said beside the push below, never in place of it.
        moved, notes = notes[0].rstrip("."), notes[1:]
    elif update.push_needed(head, fresh):
        moved = f"would push {fresh[:7]} once it passes the local checks, then wait for CI on it"
    else:
        moved = "already on `main`; nothing to push"
    runs, jobs = github.evidence(repo, head)
    ok, lines = decide.judge(pull, runs, jobs, github.paths(pull))
    outcomes.say(f"#{number}: {moved}.", *notes, f"today's head {head[:7]}: {'mergeable' if ok else 'not mergeable'} — {lines[0]}", *lines[1:])
    return number, outcomes.DRY, f"{moved}; head {head[:7]} is {'mergeable' if ok else 'not mergeable'}."


def claims(repo: str, number: int, pull: dict | None = None) -> tuple[int, str, str] | None:
    """Why pull request `number` is refused before any push, as a hand-back
    result, or `None`: not open, a draft, what it claims (#927), or no review
    decision recorded (#1764). A merge train asks it of each member too (#1785)."""
    pull = pull if pull is not None else pulls.look(number)
    if pull.get("state") != "OPEN":
        return number, outcomes.HANDED_BACK, outcomes.NOT_OPEN.format(str(pull.get("state")).lower())
    if pull.get("isDraft"):
        # Before any push: a draft is not a claim, so there is nothing to check.
        return number, outcomes.HANDED_BACK, outcomes.A_DRAFT
    # Also before any push: what the pull request claims (#927), with the issues'
    # labels as they stand now, so a greenlight given since it was queued counts.
    closes = closing.closed_by(repo, pull)
    refused = protocol.refusal({**pull, "closes": closes})
    if refused:
        outcomes.say(f"#{number}: {refused[0]}", *refused[1:])
        # An image is deleted by a commit; the rest waits for an edit or an issue (#1279).
        if carried.images(pull):
            return number, outcomes.HANDED_BACK, refused[0]
        blocking = closing.blocking(closes)
        state = outcomes.AWAITS_ISSUE if blocking else outcomes.AWAITS_EDIT
        return outcomes.Awaiting(number, state, refused[0], handbacks.claim(pull), blocking)
    # Then whether a review was decided (#1764): a refusal only a comment clears.
    refused = reviewed.refusal(pull, reviewed.comments(repo, number), pulls.is_bot(pull))
    if refused:
        outcomes.say(f"#{number}: {refused[0]}", *refused[1:])
        return outcomes.Awaiting(number, outcomes.AWAITS_REVIEW, refused[0], handbacks.claim(pull))
    return None


def take(repo: str, number: int, opts: argparse.Namespace, tick=None) -> tuple[int, str, str]:
    """One pull request, from where it is to merged or handed back. `tick()`,
    when given, is called on each poll of the CI wait ([`waiting.wait_for`])."""
    pull = pulls.look(number)
    refused = claims(repo, number, pull)
    if refused is not None:
        return refused
    branch, head = pull["headRefName"], pull["headRefOid"]
    gitio.remote("fetch", "--quiet", "origin", "main", cwd=opts.root)
    gitio.remote("fetch", "--quiet", "origin", head, cwd=opts.root)
    began = time.monotonic()
    if pulls.is_bot(pull):
        outcomes.say(f"#{number} ({branch}): Dependabot's branch; it rebases itself.")
        fresh, notes = dependabot.bot_head(number, head, opts.root, opts)
    else:
        outcomes.say(f"#{number} ({branch}): merging origin/main into {head[:7]}.")
        check = None
        if checks.needs_check(pull, opts):
            check = functools.partial(checks.verify, target=checks.target_dir(opts))
        # A dry run's conflicts are previews, not conflicts the queue met: unrecorded.
        log = None if opts.dry_run else functools.partial(record.note, record.path(opts.root), number, head)
        fresh, notes = update.advance(branch, head, opts.root, push=not opts.dry_run, check=check, log=log)
    if fresh is None:
        # Moved since the take began: the watch takes its new head afresh
        # (#1568); named pull requests are never retaken, so there it is not merged.
        moved = notes[0] == refusal.MOVED
        if moved and not opts.watch:
            notes, moved = [refusal.MOVED_NAMED, *notes[1:]], False
        outcomes.say(f"#{number}: {notes[0]}", *notes[1:])
        return number, outcomes.NOT_READY if moved else outcomes.HANDED_BACK, notes[0]
    if opts.dry_run:
        return dry(repo, pull, fresh, notes)

    if update.push_needed(head, fresh):
        outcomes.say(f"#{number}: now at {fresh[:7]}; waiting for CI.", *notes)
        stamping.stamp(repo, pull, fresh, opts.root)
    else:
        outcomes.say(f"#{number}: {notes[0] if notes else 'already on `main`; the run on record is a run on it.'}")

    # A bot's rebase wait came out of the same `--deadline`; a merge from
    # here is local work, and its CI wait gets the whole deadline as before.
    spent = time.monotonic() - began if pulls.is_bot(pull) else 0.0
    state, lines = waiting.wait_for(repo, number, fresh, head, dependabot.left(opts.deadline, spent), opts.poll, tick)
    outcomes.say(f"#{number}: {lines[0]}", *lines[1:])
    if state == outcomes.HEAD_MOVED and opts.watch:
        # Nothing was judged about the new head (#1799): it goes back in line,
        # remembered against nothing, and its next take waits on its own CI.
        # A named take keeps the hand-back: nobody watches a line for it.
        return number, outcomes.REQUEUED, f"{lines[0]} Taken again on its new head."
    if state != outcomes.GO:
        return number, outcomes.HANDED_BACK, lines[0]
    if opts.no_merge:
        return number, outcomes.GREEN, lines[0]

    state, why = landing.squash(repo, pull, fresh, opts.root, opts.poll)
    if state == landing.BASE_MOVED:  # no fault of the branch's: taken again, not handed back
        return number, outcomes.NOT_READY, why
    if state != outcomes.MERGED:
        return number, outcomes.HANDED_BACK, why
    outcomes.say(f"#{number}: merged.")
    return number, outcomes.MERGED, lines[0]


def unready(result: tuple[int, str, str]) -> tuple[int, str, str]:
    """[`take`]'s refusal of a draft or closed pull request, renamed for the
    watch: it raced the listing, and is not a hand-back to wake anyone for."""
    number, state, why = result
    refused = why == outcomes.A_DRAFT or why in {outcomes.NOT_OPEN.format(s) for s in ("closed", "merged")}
    return (number, outcomes.NOT_READY, why) if state == outcomes.HANDED_BACK and refused else result
