#!/usr/bin/env python3
"""`ruleset.py [--apply]` — `main`'s ruleset, kept as code (#491).

Until #491 `main`'s only rules were `deletion` and `non_fast_forward`: GitHub
would merge a red pull request, or one CI never ran on, and `make mergeable`
was the whole defence. The ruleset now lives in `.github/rulesets/main.json`
and adds the two required checks the workflows were built to be (#218, #229):
`ci-gate` and `lint-gate`, from GitHub Actions only (`integration_id` 15368, so
a status posted by anything else cannot satisfy them), and
`strict_required_status_checks_policy: false`, because the queue leaves a head
behind `main` when `main` moved only by Markdown (`update.docs_advance`, #587)
and strict mode would refuse it. scorsese keeps its ruleset as code too
(MatthewLacerda2/scorsese#1351, in Terraform); rusty has no Terraform, so a
JSON file and this script play that role.

Read-only by default: it compares the live ruleset with the file and exits 1
on drift, naming each difference. `--apply` writes the file to GitHub (a `PUT`
of the ruleset with the file's name, a `POST` when there is none) and checks
again. Applying changes how the merge queue merges, so the orchestrator does
it between merges, never a coder.

The comparison reads only what the file states: a key GitHub adds on its own
(an id, a timestamp, a default parameter) is not drift, while a rule type the
file does not list is.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import ghrest

FILE = Path(__file__).resolve().parents[1] / "rulesets" / "main.json"
FIELDS = ("name", "target", "enforcement", "conditions", "bypass_actors")
# Keys GitHub leaves out of its answer for a caller without write access to the
# ruleset: absent is "cannot read", never drift.
WITHHELD = ("bypass_actors",)


def covers(want: object, live: object) -> bool:
    """`live` holds everything `want` states: keys GitHub adds are ignored,
    list items match in any order, and a list's length must agree."""
    if isinstance(want, dict):
        return isinstance(live, dict) and all(k in live and covers(v, live[k]) for k, v in want.items())
    if isinstance(want, list):
        if not isinstance(live, list) or len(want) != len(live):
            return False
        return all(any(covers(w, item) for item in live) for w in want)
    return want == live


def unread(want: dict, live: dict | None) -> list[str]:
    """The committed keys this token cannot see on the live ruleset."""
    return [key for key in WITHHELD if live is not None and key in want and key not in live]


def drift(want: dict, live: dict | None) -> list[str]:
    """Each way the live ruleset differs from the committed one, in words."""
    if live is None:
        return [f"no ruleset named `{want['name']}` on GitHub"]
    found = [
        f"`{key}`: committed {json.dumps(want[key])}, live {json.dumps(live.get(key))}"
        for key in FIELDS
        if key in want and key not in unread(want, live) and not covers(want[key], live.get(key))
    ]
    wanted = {rule["type"]: rule for rule in want.get("rules", [])}
    present = {rule["type"]: rule for rule in live.get("rules") or []}
    for kind, rule in wanted.items():
        if kind not in present:
            found.append(f"rule `{kind}` is missing")
        elif not covers(rule.get("parameters", {}), present[kind].get("parameters", {})):
            found.append(f"rule `{kind}`: committed {json.dumps(rule.get('parameters'))}, "
                         f"live {json.dumps(present[kind].get('parameters'))}")
    found += [f"rule `{kind}` is live but not committed" for kind in present if kind not in wanted]
    return found


class Gh:
    """`gh` with JSON in and out; `runner` is injected so tests need no network."""

    def __init__(self, runner=subprocess.run):
        self.runner = runner

    def __call__(self, *args: str, body: dict | None = None) -> object:
        stdin = json.dumps(body) if body is not None else None
        done = self.runner(["gh", "api", *args], input=stdin, capture_output=True, text=True, check=False)
        if done.returncode != 0:
            sys.exit(f"ruleset: gh api {' '.join(args)}: {done.stderr.strip()}")
        return json.loads(done.stdout) if done.stdout.strip() else None


def live(gh: Gh, repo: str, name: str) -> dict | None:
    """The repository's own ruleset called `name`, in full, or `None`."""
    listed = ghrest.pages(lambda path: gh(path), f"repos/{repo}/rulesets?includes_parents=false") or []
    match = next((r for r in listed if r.get("name") == name), None)
    return gh(f"repos/{repo}/rulesets/{match['id']}") if match else None


def apply(gh: Gh, repo: str, want: dict, current: dict | None) -> None:
    """Write `want` over `current`, or create it when there is none."""
    if current is None:
        gh("--method", "POST", f"repos/{repo}/rulesets", "--input", "-", body=want)
    else:
        gh("--method", "PUT", f"repos/{repo}/rulesets/{current['id']}", "--input", "-", body=want)


def main(argv: list[str], gh: Gh | None = None, repo: str | None = None) -> int:
    gh = gh or Gh()
    repo = repo or ghrest.slug()
    want = json.loads(FILE.read_text())
    current = live(gh, repo, want["name"])
    gaps = drift(want, current)
    if gaps and "--apply" in argv:
        apply(gh, repo, want, current)
        print(f"ruleset: applied {FILE.name} to {repo}")
        current = live(gh, repo, want["name"])
        gaps = drift(want, current)
    for key in unread(want, current):
        print(f"ruleset: `{key}` cannot be read with this token (it needs write access); not compared")
    for gap in gaps:
        print(f"ruleset: drift: {gap}")
    if not gaps:
        print(f"ruleset: `{want['name']}` on {repo} matches {FILE.name}")
    return 1 if gaps else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
