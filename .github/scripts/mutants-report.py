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
part's survivors and their diffs named by their path in the repository. A part
whose cargo-mutants broke gets a section saying so, beside the parts that
finished, and the command exits red after writing the report.

A large plan runs on several shards (`mutants-request.py`, #2283), which
`mutants-request.py merge` turns back into that one-run layout first; a shard
that left a part nothing, or a shard or package whose run broke, is a line in
`lost-SLUG.txt`, said in the part's section beside what the rest measured, its
mutants counted as never reached, and the run red.

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

#: What the survivors are: questions, not defects (`ci-merge`, *Triaging a survivor*).
SURVIVORS = ("These were mutated and no test failed. Each is a question: test the ones that"
             " show behaviour nothing asserts, leave the rest (`ci-merge`, *Triaging a"
             " survivor*):")
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


def unanswered(reason: str) -> str:
    """The report of a run that has none: what kept it from answering."""
    return f"{HEAD}\n\n❌ No answer: {reason.strip()}\n\nThe ask is unanswered; the run's log has the rest.\n"


def section(run: dict, in_scope: int, status: int, root: str = ROOT, lost: list[str] = ()) -> list[str]:
    """One part's counts, its gaps and its survivors. `lost` is each run of
    this part that measured nothing it kept (#2283): a shard that left no
    report, or a shard or package whose run broke; its mutants count as never
    reached and the run is not clean, but the rest's measurements stand."""
    done = mutants(run)
    counts = {k: sum(o.get("summary") == s for o in done) for k, s in KINDS}
    out = ["| caught | missed | timeout | unviable |", "|---|---|---|---|"]
    out += ["| " + " | ".join(str(counts[k]) for k, _ in KINDS) + " |", ""]
    stopped = status in STOPPED
    short = stopped or bool(lost)
    if short:
        cause = "Stopped at its time budget" if stopped else "Not every shard or package reported"
        advice = "; ask for a narrower scope" if stopped else ""
        out += [f"> ⚠️ {cause}: {len(done)} of {in_scope} planned mutants"
                f" measured, {max(in_scope - len(done), 0)} never reached. Those are"
                f" unmeasured, not clean{advice}.", ""]
    out += [f"> ❌ Unmeasured: {why}" for why in lost] + ([""] if lost else [])
    out += timeouts(counts)
    survivors = sorted(label(o, root) for o in done if o.get("summary") == "MissedMutant")
    if survivors:
        out += [SURVIVORS, "", "```", *survivors, "```"]
    elif not done and stopped and not lost:
        out += ["Nothing was measured before the budget ran out: not a result. ❌"]
    elif not done and short:
        out += ["Nothing was measured: not a result. ❌"]
    elif not done:
        out += ["Nothing in scope was mutated. ➖"]
    elif counts["timeout"] or not counts["caught"] or short:
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


def rooted(diff: str, root: str) -> str:
    """A diff cargo-mutants wrote inside `root`, its file paths from the
    repository's root, so it applies there and never to a same-named engine
    file. cargo-mutants' header is `--- <file>` and `+++ <the change>`; a git
    header's `a/`/`b/` paths are prefixed after the letter."""
    if root == ROOT:
        return diff
    out = []
    for line in diff.split("\n"):
        mark, _, rest = line.partition(" ")
        if mark in ("---", "+++") and rest[:2] in ("a/", "b/"):
            line = f"{mark} {rest[:2]}{root}/{rest[2:]}"
        elif mark == "---" and rest and rest != "/dev/null":
            line = f"--- {root}/{rest}"
        elif line.startswith("diff --git a/") and " b/" in line:
            old, new = line[len("diff --git a/"):].split(" b/", 1)
            line = f"diff --git a/{root}/{old} b/{root}/{new}"
        out.append(line)
    return "\n".join(out)


def diffs(out_dir: Path, run: dict, root: str = ROOT) -> str:
    """Each survivor's diff under a `# name` line, its paths from the
    repository's root; a missing one is named."""
    out = []
    for o in sorted(mutants(run), key=lambda o: label(o, root)):
        if o.get("summary") != "MissedMutant":
            continue
        out.append(f"# {label(o, root)}")
        path = out_dir / o["diff_path"] if o.get("diff_path") else None
        if path is not None and path.is_file():
            out.append(rooted(path.read_text().rstrip("\n"), root))
        else:
            out.append("# (cargo-mutants kept no diff for this one)")
        out.append("")
    return "\n".join(out)


def status(here: Path, slug: str) -> tuple[int | None, str]:
    """A part's cargo-mutants exit status, and the status as written. `None`
    is a broken part: the tool or its baseline failed, which is not a result."""
    path = here / f"status-{slug}"
    text = path.read_text().strip() if path.is_file() else ""
    code = int(text) if text.lstrip("-").isdigit() else None
    return (code if code in FINISHED + STOPPED else None), text or "without a status"


def report(plan: dict, scope: str, here: Path) -> tuple[str, str, list[str]]:
    """The report, the survivors' diffs and the broken parts' complaints,
    over every part that planned a mutant. One broken part never discards
    another's measurements (#2201): the parts that finished are rendered, the
    broken one gets a section saying so, and the caller keeps the run red."""
    runs = [p for p in plan.get("parts", []) if p.get("planned")]
    out, found, broken = [HEAD, "", scope, ""], [], []
    if not runs:
        out += ["Nothing in scope was mutated. ➖"]
    for part in runs:
        (code, text), mutants_out = status(here, part["slug"]), here / f"mutants-{part['slug']}" / "mutants.out"
        if len(runs) > 1 or part["root"] != ROOT:
            name = "the engine's workspace" if part["root"] == ROOT else f"`{part['root']}/`"
            out += [f"### {name}", ""]
        if code is None:
            why = f"cargo-mutants in the `{part['slug']}` part exited {text}"
            broken.append(why)
            out += [f"❌ No answer for this part: {why}, the tool or its baseline breaking, not a"
                    f" result. Its {part['planned']} planned mutants are unmeasured; the run's log has why.", ""]
            continue
        outcomes, gone = mutants_out / "outcomes.json", here / f"lost-{part['slug']}.txt"
        run = json.loads(outcomes.read_text()) if outcomes.is_file() else {}
        lost = [line for line in gone.read_text().splitlines() if line.strip()] if gone.is_file() else []
        broken += lost
        out += section(run, part["planned"], code, part["root"], lost) + [""]
        found.append(diffs(mutants_out, run, part["root"]))
    return "\n".join(out).rstrip("\n") + "\n", "".join(found), broken


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
    text, found, broken = report(json.loads(args.plan.read_text()), args.scope.read_text().strip(), args.plan.parent)
    (args.out / "report.md").write_text(text)
    (args.out / "survivors.diff").write_text(found)
    if broken:
        sys.exit("mutants-report: " + "; ".join(broken) + "; that is not a result, so the run is red"
                 " (the report covers the parts that finished)")


if __name__ == "__main__":
    main()
