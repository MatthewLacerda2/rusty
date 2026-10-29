"""The merge path: is a pull request checked, and the queue that merges them.

Three parts, one per old script, each one responsibility per module:

- [`verdict`] — `make mergeable PR=N`: did CI really run on the head commit and pass?
  Pure over plain dictionaries; [`verdict.decide.judge`] is the whole decision.
- [`queue`] — `make queue PRS=...`: merge `main` in, compile-check, push, wait for CI, ask the
  verdict, squash-merge, one pull request at a time.
- [`watch`] — `make queue ARGS=--watch`: which labelled pull request goes next, and
  when the watch is done; and `make queue-daemon` ([`watch.supervisor`], #908), which
  starts it again across its planned exits.

The entry points stay `.github/scripts/mergeable.py` and `merge-queue.py`; each is a
few lines that call into here. Inside the package a module names another's function
as `module.name`, never `from module import name`, so a test patches a name once,
where it is defined, and every caller sees the patch.
"""
