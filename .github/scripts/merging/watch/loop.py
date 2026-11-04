"""The watch itself: list, pick, take, remember, repeat ([`run`])."""

from __future__ import annotations

from merging.queue import outcomes
from merging.watch import aging, board, ending, handbacks, mainline, order, stalls


def run(fx, opts) -> tuple[list[tuple[int, str, str]], str, int]:
    """Watch until nothing is left to do. Returns the results, why it ended,
    and the exit status ([`ending.status`]).

    `fx` carries every effect: `pulls()` (the open listing, or `None` when GitHub
    failed), `issue_labels()`, `clashes(pulls)` (the conflicting pairs among
    those ready, [`order.clashing`]), `main_conflicts(pulls)` (those that
    conflict with `main`, and where, [`mainline.conflicts`]), `turn(number,
    tick)` (one pull request through the queue, as [`queue.taking.take`]; it
    calls `tick()` on each poll of its CI wait, so the board is reported
    during it), `head(number)`, `attempts(number, sha)`
    (the runs on that head, [`handbacks.attempts`], or `None`), `blocks(issues)`
    (which of those issues still refuse a merge, or `None`), `decided(number)` (whether a
    comment now records its review decision, or `None`), `clock()`, `sleep(seconds)`,
    `say(...)` (also each board change, [`board.transitions`]), `memory` (a path), `bots`, and the end states: `merged`, `ends`
    (ended without a hand-back: green under `--no-merge`, previewed by a dry run)
    `stops` (the machine's failure, [`queue.stopping.Stopped`]) and `skips` (it
    turned draft or closed after the listing, or its branch moved mid-take).
    [`outcomes.REQUEUED`] (a train's member, or a head that moved during its CI
    wait, #1799) remembers nothing and is said. Anything else is a hand-back: it is
    remembered, announced ([`ending.handed_back`]) and passed over until its head
    moves or runs again; one in [`outcomes.AWAITING`] also until its body or labels
    change ([`handbacks.reworded`]), and an `AWAITS_ISSUE` one until its issues no
    longer refuse it ([`handbacks.unblocked`], #1279), and an `AWAITS_REVIEW` one until a
    comment records its review decision ([`handbacks.reviewed`], #1764). An
    [`outcomes.STALLED`] one (it outwaited its runs, #1831) also comes back when
    `stalled(pull)` ([`stalls.reading`] of its head's runs, or `None`) says every
    run on it went green ([`stalls.returned`]); one that ended otherwise is said
    once and waits like a red one.
    `stale_code()` names the queue's files that changed on `main` since launch
    ([`freshness.changed`]); asked before each take, a non-empty answer ends the
    watch ([`ending.relaunch`]) with the pull request untaken.
    `stop_requested()` says whether [`ending.STOP_FILE`] asked it to stop (and
    removes the file); it is asked at the top of each pass, so only between takes.
    `opts.for_minutes` (`--for`) is checked at the top of each pass, so only
    between takes; `opts.since` (`--since`) limits the board lines ([`board.recent`]).
    """
    results, done, failed, handed, told, landed = [], set(), 0, [], set(), set()
    memory, marks = handbacks.recall(fx.memory), handbacks.recall(fx.memory.with_name(handbacks.RUNS_MEMORY))
    claims = handbacks.recall(fx.memory.with_name(handbacks.CLAIMS_MEMORY))
    issues = handbacks.recall(fx.memory.with_name(handbacks.ISSUES_MEMORY))
    reviews = handbacks.recall(fx.memory.with_name(handbacks.REVIEWS_MEMORY))
    stalled = handbacks.recall(fx.memory.with_name(handbacks.STALLS_MEMORY))
    line = aging.Line()
    shown = [None]  # the board as last reported; `None` until the first listing

    def save() -> None:
        handbacks.remember(fx.memory, memory)
        handbacks.remember(fx.memory.with_name(handbacks.RUNS_MEMORY), {n: m for n, m in marks.items() if n in memory})
        handbacks.remember(fx.memory.with_name(handbacks.CLAIMS_MEMORY), {n: c for n, c in claims.items() if n in memory})
        handbacks.remember(fx.memory.with_name(handbacks.ISSUES_MEMORY), {n: i for n, i in issues.items() if n in memory})
        handbacks.remember(fx.memory.with_name(handbacks.REVIEWS_MEMORY), {n: r for n, r in reviews.items() if n in memory})
        handbacks.remember(fx.memory.with_name(handbacks.STALLS_MEMORY), {n: s for n, s in stalled.items() if n in memory})

    def forget(number: int) -> None:
        """Drop everything remembered against `number`: it is back in line."""
        for kept in (memory, marks, claims, issues, reviews, stalled):
            kept.pop(number, None)

    def report(pulls: list[dict]) -> None:
        now = board.heads(board.recent(pulls, getattr(opts, "since", None)))
        for said in board.transitions(shown[0], now, landed):
            fx.say(said)
        shown[0] = now

    def tick() -> None:
        pulls = fx.pulls()
        if pulls is not None:
            report(pulls)

    def settle(number: int, result: tuple[int, str, str], head: str, ask: bool = True) -> str | None:
        """Record how a pull request ended; why to exit, if the machine failed.
        A hand-back remembers the head GitHub reports now when `ask`, else `head`;
        an [`outcomes.Awaiting`] one also the claim and issues it was judged on."""
        results.append(tuple(result))
        state = result[1]
        if state == fx.merged:
            forget(number)
            landed.add(number)
        elif state in fx.ends:
            done.add(number)
        elif state == outcomes.REQUEUED:
            # A train's member back in line as it was (#1785), or a head that moved
            # during its CI wait (#1799): nothing to remember, an old hand-back dropped.
            forget(number)
        elif state in fx.skips:
            fx.sleep(opts.poll)
        elif state in fx.stops:
            return f"the machine failed on #{number}, not the branch; nothing is remembered against it."
        else:
            forget(number)
            memory[number] = (fx.head(number) if ask else None) or head
            waits = state if isinstance(result, outcomes.Awaiting) or state == outcomes.STALLED else None
            if waits == outcomes.STALLED:
                stalled[number] = waits
            elif waits:
                claims[number] = result.claim
                if result.issues:
                    issues[number] = handbacks.spell(result.issues)
                if waits == outcomes.AWAITS_REVIEW:
                    reviews[number] = waits
            mark = fx.attempts(number, memory[number])
            if mark is not None:
                marks[number] = mark
            handed.append(number)
            fx.say(ending.handed_back(number, result[2], waits))
        return None

    last, moved = None, fx.clock()
    started = moved
    while True:
        if fx.stop_requested():
            save()
            count = sum(1 for _, state, _ in results if state == fx.merged)
            return results, f"{ending.STOP}; {count} merged, nothing in hand.", ending.status(handed, False, stopped=True)
        if ending.overdue(started, fx.clock(), opts.for_minutes):
            save()
            return results, ending.deadline_reached(results, opts.for_minutes, fx.merged), ending.status(handed, False)
        pulls = fx.pulls()
        if pulls is None:
            failed += 1
            if failed >= ending.FAILS:
                return results, f"GitHub failed {ending.FAILS} listings in a row; stopped.", ending.status(handed, True)
            fx.sleep(opts.poll)
            continue
        failed = 0
        memory = handbacks.forget_closed(memory, pulls)
        for number in handbacks.rerun_heads(pulls, memory, marks, fx.attempts):
            forget(number)
            fx.say(f"watch: #{number} has run again on its handed-back head; back in line.")
        back, ended = stalls.returned(pulls, memory, stalled, fx.stalled)
        for number in back:
            forget(number)
            fx.say(f"watch: #{number}'s runs, which its hand-back outwaited, went green; back in line.")
        for number, why in ended:
            stalled.pop(number)  # an ordinary hand-back from here: a re-run or a push brings it back
            fx.say(f"watch: #{number}'s runs, which its hand-back outwaited, ended without going green: {why}"
                   " Passed over until its head moves or runs again.")
        for number in handbacks.reworded(pulls, memory, claims):
            forget(number)
            fx.say(f"watch: #{number}'s description or labels changed since its hand-back; back in line.")
        for number in handbacks.unblocked(pulls, memory, issues, fx.blocks):
            forget(number)
            fx.say(f"watch: #{number}'s issues no longer refuse it; back in line.")
        for number in handbacks.reviewed(pulls, memory, reviews, fx.decided):
            forget(number)
            fx.say(f"watch: #{number} has its review decision recorded now; back in line.")
        report(pulls)
        now = board.heads(pulls)
        if now != last:
            last, moved = now, fx.clock()
        for pull in pulls:
            workflows = board.held(pull, fx.bots)
            if workflows and (pull.get("number"), pull.get("headRefOid")) not in told:
                told.add((pull.get("number"), pull.get("headRefOid")))
                fx.say(board.held_line(pull, workflows))
        for waiter, owner in board.lock_waits(pulls, fx.bots).items():
            if (waiter, owner) not in told:
                told.add((waiter, owner))
                fx.say(board.lock_line(waiter, owner))
        waiting = order.ready(pulls, memory, done, fx.bots)
        stale = fx.main_conflicts(waiting) if waiting else {}
        if stale:
            for pull in waiting:
                if pull.get("number") in stale:
                    settle(pull["number"], (pull["number"], outcomes.HANDED_BACK, mainline.why(stale[pull["number"]])), pull.get("headRefOid", ""), ask=False)
            save()
            continue
        line.see([p.get("number") for p in waiting], fx.clock())
        aged = line.aged(fx.clock())
        labels = fx.issue_labels() if len(waiting) > 1 else {}
        clashes = fx.clashes(waiting) if len(waiting) > 1 else set()
        chosen = order.pick(pulls, labels, memory, done, fx.bots, clashes, aged)
        if chosen is None:
            why = ending.finished(pulls, fx.clock() - moved, opts.idle * 60)
            if opts.dry_run and not why:
                why = "dry run: every ready pull request has been previewed."
            if why:
                save()
                return results, why, ending.status(handed, False)
            fx.sleep(opts.poll)
            continue
        number = chosen["number"]
        moved_code = fx.stale_code()
        if moved_code:
            save()
            return results, ending.relaunch(moved_code, results, fx.merged), ending.status(handed, False, True)
        unaged = order.pick(pulls, labels, memory, done, fx.bots, clashes)
        if unaged is not chosen:
            fx.say(line.said(number, fx.clock()))
        else:
            counts = order.conflict_counts([p.get("number") for p in waiting], clashes)
            said = order.reordered(chosen, order.pick(pulls, labels, memory, done, fx.bots), counts)
            if said:
                fx.say(said)
        yard = getattr(fx, "trains", None)  # a merge train, when the watch runs them (#1785)
        members, train, notes = yard.form(order.line(pulls, labels, memory, done, fx.bots, clashes, aged)) if yard else ([chosen], None, [])
        for note in notes:
            fx.say(note)
        if train is None:
            number = members[0]["number"]
            fx.say(f"watch: taking #{number}.")
            line.took(number)
            rode, halt = [fx.turn(number, tick)], None
            if rode[0][1] == outcomes.REQUEUED:
                fx.say(f"watch: #{number} goes back in line: {rode[0][2]}")
        else:
            fx.say(f"watch: taking {', '.join(f'#{n}' for n in train.numbers)} together.")
            line.took(*train.numbers)  # one take: one pass for those behind it, none for its members
            rode, halt = yard.ride(train, tick)
        heads = {p["number"]: p.get("headRefOid", "") for p in members}
        stops = [settle(r[0], r, heads.get(r[0], "")) for r in rode]  # every member, then the first stop
        stop = next((s for s in stops if s), None)
        save()
        if halt:
            return results, *halt
        if stop:
            return results, stop, ending.status(handed, True)
        last = None  # the merge moved `main` and the listing; look afresh
