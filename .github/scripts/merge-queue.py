#!/usr/bin/env python3
"""`merge-queue.py N M ...` — do the waiting that serialized merging costs.

Invoked, never a service: it runs when somebody types `make queue`, on the pull
requests they name, in the order they name them. `--dry-run` does every read
and the local rebase, and writes nothing to GitHub. `--watch` (#664) names
none: it takes each ready pull request labelled `queue`, the go-ahead the
orchestrator adds after reading the diff (#925), the one conflicting with the
fewest others in line first, then by label priority (#753),
and is still invoked, never a service — a hand-back skips that pull request
and it carries on (#751); it exits on the machine failing, when nothing is
left, or between pull requests once its `--for` deadline passes
(`merging/watch/`).

The queue lives in `merging/queue/`, the watch in `merging/watch/`; this is the command line.

Run it:

    make queue PRS="524 526"
    make queue ARGS="--watch --no-check --for 110"
    python3 .github/scripts/merge-queue.py 524 526 --dry-run

The decisions are pure functions over plain dictionaries, tested without a
network (`make scripts`).
"""

from merging.queue import cli

if __name__ == "__main__":
    raise SystemExit(cli.main())
