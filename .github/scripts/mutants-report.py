#!/usr/bin/env python3
"""Render an on-request mutation run as the report an agent reads (#750).

cargo-mutants' own output is a log of every mutant it tried; the lines that
matter — which mutations survived — sit somewhere in the middle of it. This
writes only what someone acts on: the scope line, the counts, the survivors as
a worklist, and every survivor's diff in a second file, so `make
mutants-remote` prints the answer in the terminal.

    mutants-report.py plan.json scope.txt --out report/
    mutants-report.py --unanswered "the scope could not be read: ..." --out report/

A run has one part per cargo workspace its scope reached (`mutants-scope.py`,
#2201), each measured into `mutants-SLUG/mutants.out` with its exit status in
`status-SLUG` beside the plan; one report covers them all, a heading per part
when there is more than one or the one is not the engine's, and a standalone
part's survivors named by their path in the repository.

A run that has no answer still says so where its asker looks (#2201):
`--unanswered` writes the one-line report the workflow posts on the pull
request when the scope was unreadable, planned nothing, or broke, so an ask
that went unanswered is visibly unanswered rather than silent.

Two things it never does. It never reports a mutant it did not measure as if
it had: a run stopped at its time budget says how many planned mutants it never
reached and is never called clean, whatever it measured, because a smaller-looking run reads as a cleaner one, and a mutant that
timed out is unmeasured too, never "no survivors". And it never fails over a
survivor — mutation is a signal, and survivors are its result.
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


#: The report's heading, shared by an answered run and an unanswered one.
HEAD = "## Mutation on request"
#: The engine's workspace, as `mutants-scope.py` names its root.
ROOT = "."


def label(outcome: dict, root: str = ROOT) -> str:
    """How cargo-mutants names a mutant, `file:line:col: replace ...`, with the
    file's path from the repository's root when the run was inside `root`."""
    m = outcome["scenario"].get("Mutant", {})
    if m.get("name"):
        name = m["name"]
    else:
        start = m.get("span", {}).get("start", {})
        name = f"{m.get('file', '?')}:{start.get('line', '?')}: {m.get('replacement', '?')}"
    return name if root == ROOT else f"{root}/{name}"


def render(scope: str, run: dict, in_scope: int, status: int) -> str:
    """The Markdown report of a run with one part, the engine's."""
    return "\n".join([HEAD, "", scope, "", *section(run, in_scope, status)]) + "\n"


def unanswered(reason: str) -> str:
    """The report of a run that has none: what kept it from answering."""
    return f"{HEAD}\n\n❌ No answer: {reason.strip()}\n\nThe ask is unanswered; the run's log has the rest.\n"


def section(run: dict, in_scope: int, status: int, root: str = ROOT) -> list[str]:
    """One part's counts, its gaps and its survivors."""
    done = mutants(run)
    counts = {k: sum(o.get("summary") == s for o in done) for k, s in KINDS}
    out = ["| caught | missed | timeout | unviable |", "|---|---|---|---|"]
    out += ["| " + " | ".join(str(counts[k]) for k, _ in KINDS) + " |", ""]
    if status in STOPPED:
        out += [f"> ⚠️ Stopped at its time budget: {len(done)} of {in_scope} planned mutants"
                f" measured, {max(in_scope - len(done), 0)} never reached. Those are"
                " unmeasured, not clean; ask for a narrower scope.", ""]
    out += timeouts(counts)
    survivors = sorted(label(o, root) for o in done if o.get("summary") == "MissedMutant")
    if survivors:
        out += ["These were mutated and no test failed. Strengthen the tests covering them,"
                " or say why a survivor is equivalent:", "", "```", *survivors, "```"]
    elif not done and status in STOPPED:
        out += ["Nothing was measured before the budget ran out: not a result. ❌"]
    elif not done:
        out += ["Nothing in scope was mutated. ➖"]
    elif counts["timeout"] or not counts["caught"] or status in STOPPED:
        out += [f"No survivors among the {counts['caught']} measured; that is not a clean run."]
    else:
        out += ["No survivors. ✅"]
    return out


def timeouts(counts: dict[str, int]) -> list[str]:
    """What the timed-out mutants mean: a timeout is unmeasured, like a budget stop,
    and when most viable mutants time out the run measured the tool, not the code."""
    timed_out = counts["timeout"]
    viable = timed_out + counts["caught"] + counts["missed"]
    if not timed_out:
        return []
    if timed_out * 2 > viable:
        return [f"> ❌ {timed_out} of {viable} viable mutants timed out. That is the run"
                " breaking, not a result: a mutant that breaks nothing should finish inside"
                " the timeout the baseline set. Read the run's log before trusting any count above.", ""]
    return [f"> ⚠️ {timed_out} of {viable} viable mutants timed out: unmeasured, not clean"
            " (a mutant that loops forever times out, and so does one whose tests outgrow"
            " the timeout).", ""]


def diffs(out_dir: Path, run: dict, root: str = ROOT) -> str:
    """Each survivor's diff under a `# name` line; a missing one is named."""
    out = []
    for o in sorted(mutants(run), key=lambda o: label(o, root)):
        if o.get("summary") != "MissedMutant":
            continue
        out.append(f"# {label(o, root)}")
        path = out_dir / o["diff_path"] if o.get("diff_path") else None
        if path is not None and path.is_file():
            out.append(path.read_text().rstrip("\n"))
        else:
            out.append("# (cargo-mutants kept no diff for this one)")
        out.append("")
    return "\n".join(out)


def status(here: Path, slug: str) -> int:
    """A part's cargo-mutants exit status; a broken one leaves no honest report."""
    path = here / f"status-{slug}"
    text = path.read_text().strip() if path.is_file() else ""
    code = int(text) if text.lstrip("-").isdigit() else None
    if code not in FINISHED + STOPPED:
        sys.exit(f"mutants-report: cargo-mutants in the `{slug}` part exited {text or 'without a status'};"
                 " that is the tool or the baseline breaking, not a result, so there is no report to write")
    return code


def report(plan: dict, scope: str, here: Path) -> tuple[str, str]:
    """The report and the survivors' diffs over every part that planned a mutant."""
    runs = [p for p in plan.get("parts", []) if p.get("planned")]
    out, found = [HEAD, "", scope, ""], []
    if not runs:
        out += ["Nothing in scope was mutated. ➖"]
    for part in runs:
        code, mutants_out = status(here, part["slug"]), here / f"mutants-{part['slug']}" / "mutants.out"
        outcomes = mutants_out / "outcomes.json"
        run = json.loads(outcomes.read_text()) if outcomes.is_file() else {}
        if len(runs) > 1 or part["root"] != ROOT:
            name = "the engine's workspace" if part["root"] == ROOT else f"`{part['root']}/`"
            out += [f"### {name}", ""]
        out += section(run, part["planned"], code, part["root"]) + [""]
        found.append(diffs(mutants_out, run, part["root"]))
    return "\n".join(out).rstrip("\n") + "\n", "".join(found)


def main() -> None:
    parser = argparse.ArgumentParser(prog="mutants-report.py", description=__doc__.split("\n")[0])
    parser.add_argument("plan", type=Path, nargs="?", help="the plan `mutants-scope.py check` counted")
    parser.add_argument("scope", type=Path, nargs="?", help="the scope lines mutants-scope.py wrote")
    parser.add_argument("--unanswered", metavar="REASON", help="write the report of a run that has no answer")
    parser.add_argument("--out", type=Path, required=True, help="where report.md goes")
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    if args.unanswered is not None:
        (args.out / "report.md").write_text(unanswered(args.unanswered))
        return
    if args.plan is None or args.scope is None:
        parser.error("name the plan and the scope lines, or --unanswered")
    text, found = report(json.loads(args.plan.read_text()), args.scope.read_text().strip(), args.plan.parent)
    (args.out / "report.md").write_text(text)
    (args.out / "survivors.diff").write_text(found)


if __name__ == "__main__":
    main()
