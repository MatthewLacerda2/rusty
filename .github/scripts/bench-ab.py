#!/usr/bin/env python3
"""`bench-ab.py --pr N [--project DIR] [--stress] [--scenes a,b] [--runs n] [--load-runs n] [--size WxH] [--set k=v]... [--no-editor] [--force]` (#2381).

`make bench-ab` is this: a pull request's measured rows in three legs, `main`,
the PR's head, `main` again, the second `main` leg each row's noise floor
(`bench/__init__.py` says why). In order:

1. the refusals that need no build (`bench/refuse.py`): another timed run (an
   `flock` under `out/bench/`), a build or a bench on the machine;
2. room on the disk for both release builds (`bench/space.py`, #2436): a tree's
   warm `target/release` counts as built, and the refusal names `make reclaim`
   and the largest trees; not a timing refusal, so `--force` does not run past it;
   then `main` and the PR's head read from origin by sha, fetched and checked out detached under
   `.claude/worktrees/` (`bench/trees.py`), each tree locked before it is
   re-pointed (a live run's lock refuses first), `main` merged into the PR's
   tree so the legs differ by the PR's diff alone, `main` built;
3. `main`'s binary asked what it can do (`--help`: a `main` from before
   `--pending-imports` or `--set` merged has neither), and asked its
   un-imported textures (`bench-scenes --pending-imports`);
4. the PR built and asked the same, then the texture refusals from both answers
   together (`refuse.textures`: two builds that disagree are told apart from a
   project never imported), then the three legs (`bench/legs.py`), each in a
   memory-capped scope where the machine has one, the machine checked again
   before each, under `--force` too;
   `--stress` (#2450) then runs the fixture's stress yard in its own three legs
   (`bench/stress.py`), `play` built beside `bench-scenes` in the same cargo
   invocation, **while the PR's tree still holds `main` merged in**; without
   `--project` only the stress legs run, no `bench-scenes` is built and the
   texture refusals do not apply;
5. the table (`bench/table.py`), printed and kept as
   `out/bench/ab/pr-<n>/table.md`, beside each leg's reports; the PR's tree
   put back at its head and the trees unlocked, each only if this run locked it.

`--force` runs past a refusal and the table lists each one it ran past. Exits 0 when the table
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

from bench import legs, refuse, space, stress, table, trees


def args(argv):
    p = argparse.ArgumentParser(prog="bench-ab.py")
    p.add_argument("--pr", type=int, required=True)
    p.add_argument("--project", type=Path)
    p.add_argument("--stress", action="store_true")
    p.add_argument("--scenes")
    p.add_argument("--runs", type=int)
    p.add_argument("--load-runs", type=int, dest="load_runs")
    p.add_argument("--size")
    p.add_argument("--set", action="append", default=[], dest="sets")
    p.add_argument("--no-editor", action="store_true", dest="no_editor")
    p.add_argument("--force", action="store_true")
    a = p.parse_args(argv)
    if a.project is None and not a.stress:
        p.error("give --project DIR (the project's scenes), --stress (the fixture's stress yard), or both")
    alone = [f for f, v in (("--scenes", a.scenes), ("--load-runs", a.load_runs), ("--size", a.size),
                            ("--set", a.sets), ("--no-editor", a.no_editor)) if v]
    if a.project is None and alone:
        p.error(f"{', '.join(alone)} ask(s) the scenes' bench: give --project DIR, or leave them out for "
                "the stress yard alone")
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

    def leg(self, name: str, said: str | None) -> str | None:
        """The busy check before leg `name`: the refusal to stop on, or under `--force` noted, `None`."""
        if said and self.force:
            self.check(f"before the {name} leg: {said}")
            return None
        return said


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


def ask_binary(binary: Path, project: Path, sets: tuple[str, ...], build: str) -> tuple[str, str | None]:
    """Refuse a binary that cannot take what the run asks; else its answer on the project's
    un-imported textures (`refuse.pending_answer`, `("cannot", why)` when it has no check)."""
    shown = subprocess.run([str(binary), "--help"], text=True, capture_output=True)
    hard, soft = refuse.flags_refusal(shown.stdout + shown.stderr, binary, sets)
    if hard:
        sys.exit(f"bench-ab: refused: {hard}")
    if soft:
        return "cannot", soft
    # stdout alone: the list; `[Project] Opened …` and engine warnings go to stderr
    pending = subprocess.run([str(binary), "--project", str(project), "--pending-imports"],
                             text=True, capture_output=True)
    return refuse.pending_answer(build, pending.returncode, pending.stdout)


def check_textures(main: tuple[str, str | None], pr: tuple[str, str | None], gate: Gate) -> bool:
    """Every texture refusal through `gate`; whether `gpu_mb` and the load rows compare."""
    refusals, comparable = refuse.textures(main, pr)
    for said in refusals:
        gate.check(said)
    return comparable


def main(argv) -> int:
    a = args(argv)
    project = a.project.expanduser().resolve() if a.project else None
    if project is not None and not project.is_dir():
        sys.exit(f"bench-ab: no project at {project}")
    bins = ((trees.BENCH,) if project else ()) + ((trees.PLAY,) if a.stress else ())
    me, gate = os.getpid(), Gate(a.force)
    root = trees.main_checkout()
    held, said = refuse.hold(root / refuse.LOCK)
    gate.check(said)
    locked: list[Path] = []
    pr_tree, pr_sha = trees.path(root, trees.pr_tree(a.pr)), None
    try:
        gate.check(refuse.busy_refusal(refuse.busy(ps(), me)))
        extra = bins[1:]  # what the invocation links beside the first binary (`space.EXTRA`)
        builds = [(trees.path(root, trees.MAIN_TREE), "release", extra), (pr_tree, "release", extra)]
        roomless = space.refusal(root, builds, f"`make bench-ab PR={a.pr}`'s two release builds", subprocess.run)
        if roomless:
            raise RuntimeError(f"refused: {roomless}")
        main_sha, pr_sha = trees.fetch(a.pr, again=f"`make bench-ab PR={a.pr}`")
        # holding the timed-run flock, every bench-ab lock found is a dead run's (`trees.lock`)
        exclusive = held is not None
        main_tree = trees.take(root, trees.MAIN_TREE, main_sha, me, locked, exclusive=exclusive)
        trees.take(root, trees.pr_tree(a.pr), pr_sha, me, locked, exclusive=exclusive)
        merged = trees.merge_main(pr_tree, a.pr, main_sha)
        print(f"bench-ab: building main {main_sha[:9]} in {main_tree}", file=sys.stderr)
        main_bin = trees.build(main_tree, bins=bins)
        main_said = ask_binary(main_bin, project, tuple(a.sets), "main") if project else None
        print(f"bench-ab: building PR #{a.pr} {pr_sha[:9]} (main merged in: {merged[:9]}) in {pr_tree}",
              file=sys.stderr)
        pr_bin = trees.build(pr_tree, bins=bins)
        comparable = project is None or check_textures(main_said, ask_binary(pr_bin, project, tuple(a.sets),
                                                                             "the PR"), gate)
        out = root / "out" / "bench" / "ab" / f"pr-{a.pr}"
        plan = legs.plan(main_bin, pr_bin, out) if project else []
        yard = stress.plan((main_tree, trees.binary(main_tree, trees.PLAY)),
                           (pr_tree, trees.binary(pr_tree, trees.PLAY)), out, a.runs) if a.stress else []
        prefix, said = capped_scope()
        print(f"bench-ab: {said}", file=sys.stderr)
        ask = legs.Ask(project, a.scenes, a.runs, a.size, tuple(a.sets), a.load_runs, a.no_editor)

        def before(label):
            return lambda leg: gate.leg(label + leg.name, refuse.busy_refusal(refuse.busy(ps(), me)))

        def runner(argv):
            return subprocess.run(argv).returncode

        done = legs.run(plan, ask, prefix, runner, before(""))
        if len(done) == len(plan) and all(code == 0 for _, code in done):
            # the PR's tree still holds main merged in: `finally` puts it back only after this
            done += stress.run(yard, prefix, runner, before("stress "))
        failed = [("stress " if isinstance(leg, stress.Leg) else "") + leg.name for leg, code in done if code != 0]
        if failed or len(done) != len(plan) + len(yard):
            print(f"bench-ab: the {', '.join(failed) or 'last'} leg failed: no table", file=sys.stderr)
            return 1
        what = f"project `{project.name}`" if project else "the stress yard alone"
        header = [f"`make bench-ab` PR #{a.pr}, {what}; floor = main again against main",
                  f"- main: {main_sha[:9]}; PR head: {pr_sha[:9]}, measured with main merged in ({merged[:9]})",
                  f"- {said}"]
        header += [] if comparable else ["- **gpu_mb and the load rows are not comparable**: the two builds "
                                         "disagree on the texture cache, so one leg loaded textures uncompressed"]
        header += [f"- quality preset overridden: {', '.join(a.sets)}"
                   + (" (the scenes only: the stress yard runs at its own)" if yard else "")] if a.sets else []
        header += [f"- FORCE=1 ran past: {o}" for o in gate.overridden]
        legs_dirs = tuple(leg.out for leg in plan)
        scenes = table.scenes_of_all(legs_dirs[:2]) if plan else []
        header += table.count_changes(scenes, legs_dirs)
        text = table.render(scenes, legs_dirs, header, (stress.section(yard),) if yard else ())
        out.mkdir(parents=True, exist_ok=True)
        (out / "table.md").write_text(text)
        print(text)
        return 0
    except RuntimeError as e:
        said = str(e)
        print(f"bench-ab: {space.full(said, root, subprocess.run) if space.said_full(said) else said}",
              file=sys.stderr)
        return 1
    finally:
        if pr_tree in locked and pr_sha is not None:
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
