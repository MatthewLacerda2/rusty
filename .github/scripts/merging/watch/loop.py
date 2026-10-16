"""The watch itself: list, pick, take, remember, repeat ([`run`])."""

from __future__ import annotations

from merging.watch import board, ending, handbacks, order


def run(fx, opts) -> tuple[list[tuple[int, str, str]], str, int]:
    """Watch until nothing is left to do. Returns the results, why it ended,
    and the exit status ([`ending.status`]).

    `fx` carries every effect: `pulls()` (the open listing, or `None` when GitHub
    failed), `issue_labels()`, `clashes(pulls)` (the conflicting pairs among
    those ready, [`order.clashing`]), `turn(number)` (one pull request through the
    queue, as [`queue.taking.take`]), `head(number)`, `attempts(number, sha)`
    (the runs on that head, [`handbacks.attempts`], or `None`), `clock()`, `sleep(seconds)`,
    `say(...)` (also each board change, [`board.transitions`]), `memory` (a path), `bots`, and the end states: `merged`, `ends`
    (ended without a hand-back: green under `--no-merge`, previewed by a dry run)
    `stops` (the machine's failure, [`queue.stopping.Stopped`]) and `skips` (it
    turned draft or closed after the listing). Anything else is a hand-back: it is
    remembered, announced ([`ending.handed_back`]) and passed over.
    `opts.for_minutes` (`--for`) is checked at the top of each pass, so only
    between takes.
    """
    results, done, failed, handed, told, landed = [], set(), 0, [], set(), set()
    memory, marks = handbacks.recall(fx.memory), handbacks.recall(fx.memory.with_name(handbacks.RUNS_MEMORY))

    def save() -> None:
        handbacks.remember(fx.memory, memory)
        handbacks.remember(fx.memory.with_name(handbacks.RUNS_MEMORY), {n: m for n, m in marks.items() if n in memory})

    last, seen, moved = None, None, fx.clock()
    started = moved
    while True:
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
            memory.pop(number, None)
            marks.pop(number, None)
            fx.say(f"watch: #{number} has run again on its handed-back head; back in line.")
        now = board.heads(pulls)
        for line in board.transitions(seen, now, landed):
            fx.say(line)
        seen = now
        if now != last:
            last, moved = now, fx.clock()
        for pull in pulls:
            workflows = board.held(pull, fx.bots)
            if workflows and (pull.get("number"), pull.get("headRefOid")) not in told:
                told.add((pull.get("number"), pull.get("headRefOid")))
                fx.say(board.held_line(pull, workflows))
        waiting = order.ready(pulls, memory, done, fx.bots)
        labels = fx.issue_labels() if len(waiting) > 1 else {}
        clashes = fx.clashes(waiting) if len(waiting) > 1 else set()
        chosen = order.pick(pulls, labels, memory, done, fx.bots, clashes)
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
        counts = order.conflict_counts([p.get("number") for p in waiting], clashes)
        line = order.reordered(chosen, order.pick(pulls, labels, memory, done, fx.bots), counts)
        if line:
            fx.say(line)
        fx.say(f"watch: taking #{number}.")
        result = fx.turn(number)
        results.append(result)
        state = result[1]
        if state == fx.merged:
            memory.pop(number, None)
            landed.add(number)
        elif state in fx.ends:
            done.add(number)
        elif state in fx.skips:
            fx.sleep(opts.poll)
        elif state in fx.stops:
            save()
            return results, (
                f"the machine failed on #{number}, not the branch; nothing is remembered against it."
            ), ending.status(handed, True)
        else:
            memory[number] = fx.head(number) or chosen.get("headRefOid", "")
            marks.pop(number, None)
            mark = fx.attempts(number, memory[number])
            if mark is not None:
                marks[number] = mark
            handed.append(number)
            fx.say(ending.handed_back(number, result[2]))
        save()
        last = None  # the merge moved `main` and the listing; look afresh
