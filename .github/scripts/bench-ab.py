#!/usr/bin/env python3
"""`bench-ab.py --pr N --project DIR [--scenes a,b] [--runs n] [--size WxH] [--set k=v]... [--force]` (#2381).

`make bench-ab` is this: a pull request's measured rows in three legs, `main`,
the PR's head, `main` again, the second `main` leg each row's noise floor
(`bench/__init__.py` says why). In order:

1. the refusals that need no build (`bench/refuse.py`): another timed run (the
   lock under `out/bench/`), a build or a bench on the machine;
2. `main` and the PR's head fetched and checked out detached under
   `.claude/worktrees/` (`bench/trees.py`), `main` built;
3. the un-imported textures refusal, from `main`'s build (`bench-scenes
   --pending-imports`), before the PR's build and before any leg;
4. the PR built, then the three legs (`bench/legs.py`), each in a memory-capped
   scope where the machine has one, the machine checked again before each;
5. the table (`bench/table.py`), printed and kept as
   `out/bench/ab/pr-<n>/table.md`, beside each leg's reports.

`--force` runs past a refusal and the table says which. Exits 0 when the table
was made, 1 on a refusal or a failed leg, 2 on a usage error.
"""

from __future__ import annotations

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

from bench import legs, refuse, table, trees


def args(argv):
    p = argparse.ArgumentParser(prog="bench-ab.py")
    p.add_argument("--pr", type=int, required=True)
    p.add_argument("--project", type=Path, required=True)
    p.add_argument("--scenes")
    p.add_argument("--runs", type=int)
    p.add_argument("--size")
    p.add_argument("--set", action="append", default=[], dest="sets")
    p.add_argument("--force", action="store_true")
    return p.parse_args(argv)


def ps() -> str:
    return subprocess.run(["ps", "-A", "-o", "pid=,comm="], text=True, capture_output=True).stdout


class Gate:
    """Each refusal stops the run, or with `--force` is noted for the table."""

    def __init__(self, force: bool):
        self.force, self.overridden = force, []

    def check(self, said: str | None) -> None:
        if not said:
            return
        if not self.force:
            sys.exit(f"bench-ab: refused: {said}\n(FORCE=1 runs anyway, and the table says so)")
        print(f"bench-ab: FORCE=1 runs past: {said}", file=sys.stderr)
        self.overridden.append(said.splitlines()[0])


def capped_scope() -> tuple[list[str], str]:
    """The scope prefix the legs run under, probed once."""
    if platform.system() != "Linux" or not shutil.which("systemd-run"):
        return legs.scope(None, False)
    try:
        cap = legs.available_bytes(Path("/proc/meminfo").read_text())
    except OSError:
        cap = None
    cap = None if cap is None else int(cap * legs.CAP_FRACTION)
    probe = subprocess.run(["systemd-run", "--user", "--scope", "--quiet", "--", "true"], capture_output=True)
    return legs.scope(cap, probe.returncode == 0)


def main(argv) -> int:
    a = args(argv)
    project = a.project.expanduser().resolve()
    if not project.is_dir():
        sys.exit(f"bench-ab: no project at {project}")
    me, gate = os.getpid(), Gate(a.force)
    root = trees.main_checkout()
    lock = root / refuse.LOCK
    gate.check(refuse.take_lock(lock, me))
    try:
        gate.check(refuse.busy_refusal(refuse.busy(ps(), me)))
        main_sha, pr_sha = trees.fetch(a.pr)
        main_tree = trees.checkout(root, trees.MAIN_TREE, main_sha)
        pr_tree = trees.checkout(root, trees.pr_tree(a.pr), pr_sha)
        print(f"bench-ab: building main {main_sha[:9]} in {main_tree}", file=sys.stderr)
        main_bin = trees.build(main_tree)
        pending = subprocess.run([str(main_bin), "--project", str(project), "--pending-imports"],
                                 text=True, capture_output=True)
        gate.check(refuse.pending_refusal(pending.returncode, pending.stdout + pending.stderr))
        print(f"bench-ab: building PR #{a.pr} {pr_sha[:9]} in {pr_tree}", file=sys.stderr)
        pr_bin = trees.build(pr_tree)
        out = root / "out" / "bench" / "ab" / f"pr-{a.pr}"
        plan = legs.plan(main_bin, pr_bin, out)
        prefix, said = capped_scope()
        print(f"bench-ab: {said}", file=sys.stderr)
        ask = legs.Ask(project, a.scenes, a.runs, a.size, tuple(a.sets))

        def before_each(_leg):
            return None if a.force else refuse.busy_refusal(refuse.busy(ps(), me))

        done = legs.run(plan, ask, prefix, lambda argv: subprocess.run(argv).returncode, before_each)
        failed = [leg.name for leg, code in done if code != 0]
        if failed or len(done) != len(plan):
            print(f"bench-ab: the {', '.join(failed) or 'last'} leg failed: no table", file=sys.stderr)
            return 1
        header = [f"`make bench-ab` PR #{a.pr} ({pr_sha[:9]}) against main ({main_sha[:9]}), "
                  f"project `{project.name}`; floor = main again against main",
                  f"- {said}"]
        header += [f"- quality preset overridden: {', '.join(a.sets)}"] if a.sets else []
        header += [f"- FORCE=1 ran past: {o}" for o in gate.overridden]
        legs_dirs = tuple(leg.out for leg in plan)
        text = table.render(table.scenes_of(plan[0].out), legs_dirs, header)
        (out / "table.md").write_text(text)
        print(text)
        return 0
    except RuntimeError as e:
        print(f"bench-ab: {e}", file=sys.stderr)
        return 1
    finally:
        refuse.release(lock, me)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
