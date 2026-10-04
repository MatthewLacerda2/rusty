#!/usr/bin/env python3
"""Render an on-request mutation run as the report an agent reads (#750).

cargo-mutants' own output is a log of every mutant it tried; the lines that
matter — which mutations survived — sit somewhere in the middle of it. This
writes only what someone acts on: the scope line, the counts, the survivors as
a worklist, and every survivor's diff in a second file, so `make
mutants-remote` prints the answer in the terminal.

    mutants-report.py mutants.out scope.txt --in-scope 42 --exit 2 --out report/

Two things it never does. It never reports a run that did not finish as if it
had: a run stopped at its time budget says how many planned mutants it never
reached, because a smaller-looking run reads as a cleaner one. And it never
fails over a survivor — mutation is a signal, and survivors are its result.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

#: Exit statuses that mean cargo-mutants finished: clean, survivors, timeouts.
FINISHED = (0, 2, 3)
#: `timeout`'s statuses: the run hit its budget and stopped itself.
STOPPED = (124, 137)
#: What the counts table lists, in cargo-mutants' outcome names.
KINDS = (("caught", "CaughtMutant"), ("missed", "MissedMutant"),
         ("timeout", "Timeout"), ("unviable", "Unviable"))


def mutants(run: dict) -> list[dict]:
    """Every mutant outcome; the unmutated baseline is not one."""
    return [o for o in run.get("outcomes", []) if isinstance(o.get("scenario"), dict)]


def label(outcome: dict) -> str:
    """How cargo-mutants names a mutant: `file:line:col: replace ...`."""
    m = outcome["scenario"].get("Mutant", {})
    if m.get("name"):
        return m["name"]
    start = m.get("span", {}).get("start", {})
    return f"{m.get('file', '?')}:{start.get('line', '?')}: {m.get('replacement', '?')}"


def render(scope: str, run: dict, in_scope: int, status: int) -> str:
    """The Markdown report."""
    done = mutants(run)
    counts = {k: sum(o.get("summary") == s for o in done) for k, s in KINDS}
    out = ["## Mutation on request", "", scope, ""]
    out += ["| caught | missed | timeout | unviable |", "|---|---|---|---|"]
    out += ["| " + " | ".join(str(counts[k]) for k, _ in KINDS) + " |", ""]
    if status in STOPPED:
        out += [f"> ⚠️ Stopped at its time budget: {len(done)} of {in_scope} planned mutants"
                f" measured, {max(in_scope - len(done), 0)} never reached. Those are"
                " unmeasured, not clean; ask for a narrower scope.", ""]
    survivors = sorted(label(o) for o in done if o.get("summary") == "MissedMutant")
    if survivors:
        out += ["These were mutated and no test failed. Strengthen the tests covering them,"
                " or say why a survivor is equivalent:", "", "```", *survivors, "```"]
    elif done:
        out += ["No survivors. ✅"]
    else:
        out += ["Nothing in scope was mutated. ➖"]
    return "\n".join(out) + "\n"


def diffs(root: Path, run: dict) -> str:
    """Each survivor's diff under a `# name` line; a missing one is named."""
    out = []
    for o in sorted(mutants(run), key=label):
        if o.get("summary") != "MissedMutant":
            continue
        out.append(f"# {label(o)}")
        path = root / o["diff_path"] if o.get("diff_path") else None
        if path is not None and path.is_file():
            out.append(path.read_text().rstrip("\n"))
        else:
            out.append("# (cargo-mutants kept no diff for this one)")
        out.append("")
    return "\n".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-report.py", description=__doc__.split("\n")[0])
    parser.add_argument("mutants_out", type=Path, help="cargo-mutants' output directory")
    parser.add_argument("scope", type=Path, help="the scope line mutants-scope.py wrote")
    parser.add_argument("--in-scope", type=int, required=True, help="mutants --list planned")
    parser.add_argument("--exit", type=int, required=True, help="cargo-mutants' exit status")
    parser.add_argument("--out", type=Path, required=True, help="where report.md goes")
    args = parser.parse_args()

    if args.exit not in FINISHED + STOPPED:
        sys.exit(f"mutants-report: cargo-mutants exited {args.exit}; that is the tool or the"
                 " baseline breaking, not a result, so there is no report to write")
    outcomes = args.mutants_out / "outcomes.json"
    run = json.loads(outcomes.read_text()) if outcomes.is_file() else {}
    args.out.mkdir(parents=True, exist_ok=True)
    scope = args.scope.read_text().strip()
    (args.out / "report.md").write_text(render(scope, run, args.in_scope, args.exit))
    (args.out / "survivors.diff").write_text(diffs(args.mutants_out, run))


if __name__ == "__main__":
    main()
