#!/usr/bin/env python3
"""Fit an on-request mutation run to its plan: what each mutant tests, how many
runners the plan gets, and one report merged from them (#2283).

In the batch that ended 2026-11-17, five of seven scoped runs stopped at their
180-minute budget having measured a fifth to a half of their plan: #2282's run
(43991788372) measured 20 of 114 in 144 minutes of mutating, about 7 minutes a
mutant. Its log says where they went. `.cargo/mutants.toml`'s `test_workspace`
gave every mutant the whole workspace's suite, 6520 tests in 809 s, in
nextest's order: the root package's integration binary first (351 s), then
every crate alphabetically. A mutant in `rusty-components` was not tested by
its own crate until 359 s in; one in `rusty-shell`, not until 799 s. A survivor
ran all 809 s (71 s build + 785 s test). So each mutant is now tested by:

- **only the packages that can see it**: nextest's `-E 'rdeps(=P)'`, the
  mutated package and every workspace package that depends on it, through
  dev-dependencies too (`rdeps(=rusty-time)` keeps `rusty-ui`, whose only edge
  to it is a dev-dependency). Nothing else can catch a mutant in P: a package
  that does not depend on P does not compile P.
- **its own package's tests first**: a nextest tool config ([`priority`]) that
  raises `package(=P)`'s priority, so the crate that knows the code best runs
  before the integration binary rather than after it.

Both are per mutated package, so the engine's part runs one cargo-mutants
invocation per package in the plan ([`packages`]), each `--package P`, each
with its own filter and priority. The build is unchanged: `--workspace`, as the
baseline built it, so a mutant rebuilds only what depends on P.

- **A large plan is sharded** ([`shards`]): runners enough that each one's
  share ends inside the budget at `MINUTES_PER_MUTANT`, at most `MAX_SHARDS`.
  The merge queue runs two requests at once (`merging/after/slots.CAP`), so
  mutation holds at most eight runners, clear of the gating CI the batch waits
  on; a `diff` plan past the cap runs at the cap and its report says what it
  never reached, since no narrower honest ask exists for it. A standalone part runs on shard 0
  before the engine's share, so its mutants count toward the sizing too.
  Every shard runs the same per-package invocations with `--shard J/N
  --sharding round-robin`, where `J = (K - offset) mod N` for shard K and
  the package's offset in the plan ([`packages`]), so the whole plan is dealt
  round-robin as one list.
- **A hand ask too large to measure is refused** ([`refusal`], #2371): named
  files mutated whole that plan more than [`HEADROOM`] times what `MAX_SHARDS`
  runners measure start no shard; the `plan` job ends red with the planned
  count, what fits one ask and each file's count, largest first, so the next
  ask can be narrower. Run 44203600903 held four runners 166 minutes each for
  a 737-mutant plan it measured 263 of; that it could not fit was known in
  `plan`, minutes in.
- **A package is checked before it is mutated** ([`tests`]): `cargo nextest
  list` with exactly the filter and tool config its mutants get must succeed
  and select a test. cargo-mutants counts any non-zero nextest exit as a
  caught mutant, so a filter nextest refuses would otherwise report every
  mutant caught.
- **One report from every shard** ([`merge`]): each part's outcomes, from every
  shard and package, become the single-run layout `mutants-report.py` reads,
  `mutants-SLUG/mutants.out/outcomes.json` and `status-SLUG`. What went
  unmeasured is a line in `lost-SLUG.txt`, its mutants counted as never
  reached and the run red, beside what the rest measured (#2201's rule): a
  package or shard whose cargo-mutants (or baseline, or check) broke, written
  by `merge`; and a shard that uploaded nothing (its runner died, its job hit
  `timeout-minutes`), printed as `slug shard` for the workflow to name the
  cause (`mutants-torn-down.py`). Each shard's artifact is downloaded by name
  into its own directory: `download-artifact` unpacks a lone match of a
  `pattern` flat, which made a one-shard run's only shard read as lost.

    mutants-request.py shards --planned 114 --budget-seconds 9900
    mutants-request.py packages planned-root.json
    mutants-request.py priority rusty-shell > priority.toml
    cargo nextest list ... --message-format json | mutants-request.py tests
    mutants-request.py merge plan.json --shards shards/ --total 4
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

#: Minutes a mutant costs with the per-package tests above: run 44014339190
#: (#2269's plan re-asked) spent 282 runner-minutes on 71 mutants, 191 s a
#: caught one and 631 s a survivor, against 7.2 min a mutant before.
#: Checked, and kept, against the two large runs since: 44149115191 (110
#: mutants, four shards) spent 534 runner-minutes, 454 past setup, 4.1 a
#: mutant; 44203600903 (263 measured) 666, 586 past setup, 2.2. 4.0 sits at
#: the slow end, so a plan sized by it ends inside its budget.
MINUTES_PER_MUTANT = 4.0
#: What a shard spends before its first mutant: setup, the workspace's build
#: and its timed baseline (15 to 17 min in run 44014339190, 21 in #2282's),
#: with room for a cold cache.
SETUP_MINUTES = 20
#: The most runners one request takes: the queue runs two at once, and the
#: gating CI of the batch's pull requests needs the rest of the account's.
MAX_SHARDS = 4
#: How far past what fits one ask (`MAX_SHARDS` runners at `MINUTES_PER_MUTANT`)
#: a whole-file ask may plan before it is refused. At 4.0 one ask fits 145;
#: four runners measure about 141 at the slow 4.1 minutes a mutant and 263 at
#: the fast 2.2; 1.5 x 145 = 217 sits between, and a plan past it is
#: refused. A hand ask of 218 to 263
#: is turned away though the fast rate would finish it: kept so, since a
#: refusal costs the asker a narrower re-ask minutes later, while a plan the
#: slow rate cannot finish holds four runners for their whole budget and
#: still leaves mutants unreached.
HEADROOM = 1.5
#: How many files the refusal lists by count; the rest are summed.
LISTED = 15
#: cargo-mutants' exit statuses that finished, and `timeout`'s at the budget.
FINISHED, STOPPED = (0, 2, 3), (124, 137)
#: The engine's part, as `mutants-scope.py` names it; only it is sharded.
ROOT = "root"


def shards(planned: int, budget_seconds: int, minutes: float = MINUTES_PER_MUTANT,
           standalone: int = 0) -> tuple[int, str]:
    """How many runners a plan of `planned` engine mutants gets, and why. A
    standalone part's mutants run on shard 0 first, so they count too."""
    usable = budget_seconds / 60 - SETUP_MINUTES
    if planned <= 0 or usable <= 0:
        return 1, f"{planned} engine mutants: one runner"
    want = math.ceil((planned + standalone) * minutes / usable)
    n = max(1, min(want, MAX_SHARDS))
    extra = f" and {standalone} standalone" if standalone else ""
    why = (f"{planned} engine{extra} mutants at {minutes:g} min each over {usable:.0f} min a runner:"
           f" {n} {'runner' if n == 1 else 'runners'}")
    if want > MAX_SHARDS:
        why += f" (capped from {want}; the report names what it never reached)"
    return n, why


def fits(budget_seconds: int, minutes: float = MINUTES_PER_MUTANT) -> int:
    """How many mutants `MAX_SHARDS` runners measure inside the budget."""
    return max(0, int(MAX_SHARDS * (budget_seconds / 60 - SETUP_MINUTES) / minutes))


def refusal(plan: dict, budget_seconds: int) -> str | None:
    """Why a whole-file ask is refused before any shard starts, or `None`.
    Each part's `by_file` is what `mutants-scope.py check` counted in the
    `--list` the `plan` job already ran. A `diff`, narrowed or not, is never
    refused: it runs at the cap."""
    if plan.get("kind") != "paths":
        return None
    per_file: dict[str, int] = {}
    for part in plan.get("parts", []):
        for f, n in part.get("by_file", {}).items():
            per_file[f] = per_file.get(f, 0) + n
    total, fit = sum(per_file.values()), fits(budget_seconds)
    if total <= HEADROOM * fit:
        return None
    ranked = sorted(per_file.items(), key=lambda kv: (-kv[1], kv[0]))
    rows = [f"- `{f}`: {n}" for f, n in ranked[:LISTED]]
    if len(ranked) > LISTED:
        rest = ranked[LISTED:]
        rows.append(f"- {len(rest)} more files: {sum(n for _, n in rest)}")
    return "\n".join([
        f"the scope plans {total} mutants with its files mutated whole, and one ask measures about {fit}"
        f" ({MAX_SHARDS} runners at {MINUTES_PER_MUTANT:g} min a mutant; refused past {int(HEADROOM * fit)})."
        " No shard started. Name fewer files, or ask `diff` for what a branch changed. Mutants per file:",
        "", *rows])


def packages(listed: list[dict]) -> list[tuple[str, int]]:
    """The packages the engine's plan mutates, in a stable order, each with
    how many planned mutants come before it. cargo-mutants shards each
    invocation alone, so a package's own `--shard` is turned by that offset
    (the workflow's `(K - offset) mod N`): the plan is dealt round-robin as one
    list, and a run of one-mutant packages does not all land on shard 0."""
    counts: dict[str, int] = {}
    for m in listed:
        if m.get("package"):
            counts[m["package"]] = counts.get(m["package"], 0) + 1
    out, before = [], 0
    for name in sorted(counts):
        out.append((name, before))
        before += counts[name]
    return out


def priority(package: str) -> str:
    """A nextest tool config running `package`'s own tests first. A tool
    config's override applies where the repository's `.config/nextest.toml`
    sets nothing, and it sets no priority, so its test groups still hold."""
    return f"[[profile.default.overrides]]\nfilter = 'package(={package})'\npriority = 100\n"


def tests(listed: dict) -> int:
    """How many tests a `cargo nextest list --message-format json` selects."""
    return sum(1 for suite in listed.get("rust-suites", {}).values()
               for case in suite.get("testcases", {}).values()
               if case.get("filter-match", {}).get("status") == "matches" and not case.get("ignored"))


def entries(shard: Path, slug: str) -> list[tuple[Path, str, str]]:
    """Each measured run of the part `slug` a shard holds, as its output
    directory, its status text and what ran: `status-SLUG` beside
    `mutants-SLUG/` (a standalone part, or an engine part that never reached
    its mutants), and `mutants-SLUG/<package>/status` per package of the
    engine's part."""
    out, top = [], shard / f"status-{slug}"
    if top.is_file():
        out.append((shard / f"mutants-{slug}" / "mutants.out", top.read_text().strip(), ""))
    for status in sorted((shard / f"mutants-{slug}").glob("*/status")):
        out.append((status.parent / "mutants.out", status.read_text().strip(), status.parent.name))
    return out


def code(text: str) -> int | None:
    """A status text's exit code when it is one that finished or stopped."""
    value = int(text) if text.lstrip("-").isdigit() else None
    return value if value in FINISHED + STOPPED else None


def combined(codes: list[int]) -> str:
    """One status for a part from its runs that finished or stopped: stopped
    if any stopped, else the highest finished status (survivors, timeouts).
    A broken run is not among them: it is a `lost-SLUG.txt` line instead."""
    stopped = [c for c in codes if c in STOPPED]
    return str(stopped[0] if stopped else max(codes, default=0))


def broke(part: dict, package: str, k: int, total: int, text: str) -> str:
    """The `lost-SLUG.txt` line of a run that broke: what, where, and how."""
    what = f"`{package}`" if package else (
        "the engine's part" if part["slug"] == ROOT else f"`{part['root']}/`")
    where = f" in shard {k}" if total > 1 else ""
    how = f"cargo-mutants exited {text}" if text.lstrip("-").isdigit() else text
    return f"{what}{where}: {how}; its mutants are unmeasured"


def merge(plan: dict, here: Path, shard_dir: Path, total: int) -> list[str]:
    """Write each planned part's merged outcomes, status and broken runs
    beside the plan; return `slug shard` for each shard that left that part
    nothing at all. One broken run never hides what the others measured."""
    lost = []
    for part in (p for p in plan.get("parts", []) if p.get("planned")):
        slug, runs, codes, broken = part["slug"], [], [], []
        expected = range(total) if slug == ROOT else range(1)
        for k in expected:
            shard = shard_dir / f"mutants-shard-{k}"
            found = entries(shard, slug) if shard.is_dir() else []
            if not shard.is_dir():
                lost.append(f"{slug} {k}")
            elif not found:
                broken.append(broke(part, "", k, total, "left no status"))
            for out_dir, text, package in found:
                runs.append(out_dir)
                if code(text) is None:
                    broken.append(broke(part, package, k, total, text))
                else:
                    codes.append(code(text))
        outcomes = []
        for out_dir in runs:
            path = out_dir / "outcomes.json"
            for o in (json.loads(path.read_text()).get("outcomes", []) if path.is_file() else []):
                if o.get("diff_path"):
                    o["diff_path"] = str((out_dir / o["diff_path"]).resolve())
                outcomes.append(o)
        merged = here / f"mutants-{slug}" / "mutants.out"
        merged.mkdir(parents=True, exist_ok=True)
        (merged / "outcomes.json").write_text(json.dumps({"outcomes": outcomes}))
        (here / f"status-{slug}").write_text(combined(codes) + "\n")
        (here / f"lost-{slug}.txt").write_text("".join(line + "\n" for line in broken))
    return lost


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-request.py", description=__doc__.split("\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    size = sub.add_parser("shards", help="print `shards=[...]` for the matrix; why on stderr")
    size.add_argument("--planned", type=int, required=True, help="the engine part's planned mutants")
    size.add_argument("--budget-seconds", type=int, required=True)
    size.add_argument("--standalone", type=int, default=0, help="the standalone parts' planned mutants")
    ref = sub.add_parser("refuse", help="exit 1, printing why, when a whole-file plan is too large to measure")
    ref.add_argument("plan", type=Path, help="the plan, after `mutants-scope.py check` counted it")
    ref.add_argument("--budget-seconds", type=int, required=True)
    sub.add_parser("tests", help="how many tests the `cargo nextest list` JSON on stdin selects")
    pkgs = sub.add_parser("packages", help="`package offset` for each package a `--list --json` file mutates")
    pkgs.add_argument("listed", type=Path)
    pri = sub.add_parser("priority", help="print the nextest tool config that runs PACKAGE's tests first")
    pri.add_argument("package")
    mrg = sub.add_parser("merge", help="merge the shards' outcomes beside the plan; print lost `slug shard`s")
    mrg.add_argument("plan", type=Path)
    mrg.add_argument("--shards", type=Path, required=True, help="one `mutants-shard-K/` per shard")
    mrg.add_argument("--total", type=int, required=True, help="how many shards the matrix had")
    args = parser.parse_args()

    if args.command == "shards":
        n, why = shards(args.planned, args.budget_seconds, standalone=args.standalone)
        print(f"shards={json.dumps(list(range(n)))}")
        print(why, file=sys.stderr)
    elif args.command == "refuse":
        if why := refusal(json.loads(args.plan.read_text()), args.budget_seconds):
            print(why)
            sys.exit(1)
    elif args.command == "packages":
        text = args.listed.read_text().strip() if args.listed.is_file() else ""
        for name, offset in packages(json.loads(text) if text else []):
            print(name, offset)
    elif args.command == "tests":
        text = sys.stdin.read().strip()
        print(tests(json.loads(text)) if text else 0)
    elif args.command == "priority":
        print(priority(args.package), end="")
    else:
        plan = json.loads(args.plan.read_text())
        for line in merge(plan, args.plan.parent, args.shards, args.total):
            print(line)


if __name__ == "__main__":
    main()
