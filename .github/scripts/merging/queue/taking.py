"""One pull request, from where it is to merged or handed back ([`take`])."""

from __future__ import annotations

import argparse
import functools
import subprocess
import time

from merging.queue import checks, dependabot, gitio, landing, outcomes, pulls, refusal, stamping, update, waiting
from merging.verdict import carried, closing, decide, github, protocol
from merging.watch import handbacks


def dry(repo: str, pull: dict, fresh: str, notes: list[str]) -> tuple[int, str, str]:
    """The dry run's report: what would happen, and the verdict on today's head."""
    number, head = pull["number"], pull["headRefOid"]
    if notes:
        # Dependabot's branch, behind `main` ([`dependabot.bot_head`]), or a head `main`
        # passed only by Markdown ([`update.advance`]): the note says what happens.
        moved, notes = notes[0].rstrip("."), notes[1:]
    elif update.push_needed(head, fresh):
        moved = f"would push {fresh[:7]} once it passes the local checks, then wait for CI on it"
    else:
        moved = "already on `main`; nothing to push"
    runs, jobs = github.evidence(repo, head)
    ok, lines = decide.judge(pull, runs, jobs, github.paths(pull))
    outcomes.say(f"#{number}: {moved}.", *notes, f"today's head {head[:7]}: {'mergeable' if ok else 'not mergeable'} — {lines[0]}", *lines[1:])
    return number, outcomes.DRY, f"{moved}; head {head[:7]} is {'mergeable' if ok else 'not mergeable'}."


def take(repo: str, number: int, opts: argparse.Namespace, tick=None) -> tuple[int, str, str]:
    """One pull request, from where it is to merged or handed back. `tick()`,
    when given, is called on each poll of the CI wait ([`waiting.wait_for`])."""
    pull = pulls.look(number)
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
        fresh, notes = update.advance(branch, head, opts.root, push=not opts.dry_run, check=check)
    if fresh is None:
        outcomes.say(f"#{number}: {notes[0]}", *notes[1:])
        # Pushed to since the take began: its new head is taken afresh (#1568).
        moved = notes[0] == refusal.MOVED
        return number, outcomes.NOT_READY if moved else outcomes.HANDED_BACK, notes[0]
    if opts.dry_run:
        return dry(repo, pull, fresh, notes)

    if update.push_needed(head, fresh):
        outcomes.say(f"#{number}: now at {fresh[:7]}; waiting for CI.")
        stamping.stamp(repo, pull, fresh, opts.root)
    else:
        outcomes.say(f"#{number}: {notes[0] if notes else 'already on `main`; the run on record is a run on it.'}")

    # A bot's rebase wait came out of the same `--deadline`; a merge from
    # here is local work, and its CI wait gets the whole deadline as before.
    spent = time.monotonic() - began if pulls.is_bot(pull) else 0.0
    state, lines = waiting.wait_for(repo, number, fresh, head, dependabot.left(opts.deadline, spent), opts.poll, tick)
    outcomes.say(f"#{number}: {lines[0]}", *lines[1:])
    if state != outcomes.GO:
        return number, outcomes.HANDED_BACK, lines[0]
    if opts.no_merge:
        return number, outcomes.GREEN, lines[0]

    # REST's merge (#970); `sha` is `--match-head-commit`: a push after the
    # verdict makes GitHub refuse.
    done = subprocess.run(
        ["gh", "api", "--method", "PUT", f"repos/{repo}/pulls/{number}/merge",
         "-f", "merge_method=squash", "-f", f"sha={fresh}", "-f", f"commit_title={pulls.subject(pull)}"],
        capture_output=True, text=True, check=False,
    )
    if done.returncode != 0:
        failure = done.stderr.strip()
        if not landing.transport(failure):
            blocked = f"CI passed but the merge was refused: {failure}"
            outcomes.say(f"#{number}: {blocked}")
            return number, outcomes.HANDED_BACK, blocked
        outcomes.say(f"#{number}: the merge call failed in transit: {failure}")
        state, after = landing.confirm(number, opts.poll)
        outcomes.say(f"#{number}: {after[0]}", *after[1:])
        if state != outcomes.GO:
            return number, outcomes.HANDED_BACK, after[0]
    outcomes.say(f"#{number}: merged.")
    return number, outcomes.MERGED, lines[0]


def unready(result: tuple[int, str, str]) -> tuple[int, str, str]:
    """[`take`]'s refusal of a draft or closed pull request, renamed for the
    watch: it raced the listing, and is not a hand-back to wake anyone for."""
    number, state, why = result
    refused = why == outcomes.A_DRAFT or why in {outcomes.NOT_OPEN.format(s) for s in ("closed", "merged")}
    return (number, outcomes.NOT_READY, why) if state == outcomes.HANDED_BACK and refused else result
