#!/usr/bin/env python3
"""`bench-ab.py --pr N --project DIR [--scenes a,b] [--runs n] [--size WxH] [--set k=v]... [--force]` (#2381).

`make bench-ab` is this: a pull request's measured rows in three legs, `main`,
the PR's head, `main` again, the second `main` leg each row's noise floor
(`bench/__init__.py` says why). In order:

1. the refusals that need no build (`bench/refuse.py`): another timed run (an
   `flock` under `out/bench/`), a build or a bench on the machine;
2. `main` and the PR's head fetched and checked out detached under
   `.claude/worktrees/` (`bench/trees.py`), `main` merged into the PR's tree so
   the legs differ by the PR's diff alone, both trees locked against `make
   reclaim` for the run, `main` built;
3. `main`'s binary asked what it can do (`--help`: a `main` from before
   `--pending-imports` or `--set` merged has neither), then the un-imported
   textures refusal from it (`bench-scenes --pending-imports`);
4. the PR built, asked the same and checked the same (a PR that changes the
   texture cache's stamp sees every cache as stale), then the three legs
   (`bench/legs.py`), each in a memory-capped scope where the machine has one,
   the machine checked again before each;
5. the table (`bench/table.py`), printed and kept as
   `out/bench/ab/pr-<n>/table.md`, beside each leg's reports; the trees
   unlocked and the PR's put back at its head.

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
    a = p.parse_args(argv)
    for pair in a.sets:
        if "{" in pair:  # make splits SET on spaces and the shell eats its quotes: no JSON objects
            p.error(f"--set {pair}: a JSON object value is not taken; set each field by its dotted key "
                    "(ssao.high.radius=1.5)")
    return a


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
    if cap is None:
        return legs.scope(None, True)
    # Probed with the cap set: a user manager without the memory controller
    # delegated takes a plain scope and ignores the limit, and refuses this one.
    probe = subprocess.run(["systemd-run", "--user", "--scope", "--quiet", "-p", f"MemoryMax={cap}", "--", "true"],
                           capture_output=True)
    return legs.scope(cap, probe.returncode == 0)


def ask_binary(binary: Path, project: Path, sets: tuple[str, ...], gate: Gate) -> None:
    """Refuse a binary that cannot take what the run asks, then its project's un-imported textures."""
    shown = subprocess.run([str(binary), "--help"], text=True, capture_output=True)
    hard, soft = refuse.flags_refusal(shown.stdout + shown.stderr, binary, sets)
    if hard:
        sys.exit(f"bench-ab: refused: {hard}")
    if soft:
        gate.check(soft)
        return
    pending = subprocess.run([str(binary), "--project", str(project), "--pending-imports"],
                             text=True, capture_output=True)
    gate.check(refuse.pending_refusal(pending.returncode, pending.stdout + pending.stderr))


def main(argv) -> int:
    a = args(argv)
    project = a.project.expanduser().resolve()
    if not project.is_dir():
        sys.exit(f"bench-ab: no project at {project}")
    me, gate = os.getpid(), Gate(a.force)
    root = trees.main_checkout()
    held, said = refuse.hold(root / refuse.LOCK)
    gate.check(said)
    locked: list[Path] = []
    pr_tree, pr_sha = None, None
    try:
        gate.check(refuse.busy_refusal(refuse.busy(ps(), me)))
        main_sha, pr_sha = trees.fetch(a.pr)
        main_tree = trees.checkout(root, trees.MAIN_TREE, main_sha)
        pr_tree = trees.checkout(root, trees.pr_tree(a.pr), pr_sha)
        for tree in (main_tree, pr_tree):
            trees.lock(root, tree, me)
            locked.append(tree)
        merged = trees.merge_main(pr_tree, a.pr, main_sha)
        print(f"bench-ab: building main {main_sha[:9]} in {main_tree}", file=sys.stderr)
        main_bin = trees.build(main_tree)
        ask_binary(main_bin, project, tuple(a.sets), gate)
        print(f"bench-ab: building PR #{a.pr} {pr_sha[:9]} (main merged in: {merged[:9]}) in {pr_tree}",
              file=sys.stderr)
        pr_bin = trees.build(pr_tree)
        ask_binary(pr_bin, project, tuple(a.sets), gate)
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
        header = [f"`make bench-ab` PR #{a.pr}, project `{project.name}`; floor = main again against main",
                  f"- main: {main_sha[:9]}; PR head: {pr_sha[:9]}, measured with main merged in ({merged[:9]})",
                  f"- {said}"]
        header += [f"- quality preset overridden: {', '.join(a.sets)}"] if a.sets else []
        header += [f"- FORCE=1 ran past: {o}" for o in gate.overridden]
        legs_dirs = tuple(leg.out for leg in plan)
        text = table.render(table.scenes_of_all(legs_dirs[:2]), legs_dirs, header)
        (out / "table.md").write_text(text)
        print(text)
        return 0
    except RuntimeError as e:
        print(f"bench-ab: {e}", file=sys.stderr)
        return 1
    finally:
        if pr_tree is not None and pr_sha is not None:
            try:  # back at the PR's own head, so reclaim judges it by the PR's commit
                trees.run(["git", "-C", str(pr_tree), "checkout", "--quiet", "--detach", pr_sha])
            except RuntimeError as e:
                print(f"bench-ab: {pr_tree} not put back at the PR's head: {e}", file=sys.stderr)
        for tree in locked:
            trees.unlock(root, tree, me)
        if held is not None:
            held.close()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
