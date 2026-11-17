#!/usr/bin/env python3
"""Give every artifact `download-artifact` fetched by `pattern:` its own directory.

`actions/download-artifact` with `pattern:` and `path: <dir>` unpacks each match
into `<dir>/<artifact-name>/`, except when the pattern matches exactly one
artifact: that one is unpacked flat into `<dir>/` itself. A step that reads
`<dir>/<name>-*/` then finds nothing, and the lone artifact reads as absent. It
bit twice: `mutants-on-request.yml`, where a one-shard run (its common case)
reported its only shard as lost (#2284, fixed by downloading each shard by
name), and `mutants-sweep.yml`, where a sweep with a single surviving shard
dropped the one shard that measured from its report (#2288).

So each uploader writes its own artifact's name into a file `artifact-name` at
the root of what it uploads, and the step after the download runs

    python3 .github/scripts/artifact-dirs.py <dir>

which, on a flat layout (`<dir>/artifact-name` present), moves everything in
`<dir>` into `<dir>/<name>/`. A layout already split per artifact, an empty
`<dir>`, or no `<dir>` at all (nothing matched) is left as it is. A flat layout
without `artifact-name` is an uploader that forgot to write it: the script says
so and exits 1, rather than guess a name. `test_workflow_hygiene.py` holds every
`pattern:` download to this step (or to `merge-multiple: true`, with a reason).
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

#: The file at an artifact's root that names it.
NAME_FILE = "artifact-name"


def normalise(root: Path) -> str:
    """Split a flat download in `root` into `root/<name>/`; what happened, in a line.

    Raises `ValueError` for a flat layout with no `artifact-name`.
    """
    if not root.is_dir():
        return f"{root}: absent, no artifact matched"
    entries = list(root.iterdir())
    name_file = root / NAME_FILE
    if not name_file.is_file():
        if any(entry.is_file() for entry in entries):
            raise ValueError(
                f"{root} holds files but no {NAME_FILE}: a lone artifact unpacked flat whose "
                f"uploader did not write its name. Write `echo <artifact name> > <upload dir>/{NAME_FILE}` "
                f"in the step before its `upload-artifact`.")
        return f"{root}: {len(entries)} artifact(s), one directory each"
    name = name_file.read_text().strip()
    if not name or "/" in name or name in (".", ".."):
        raise ValueError(f"{name_file} names no artifact: {name!r}")
    # Through a temporary name: the artifact may hold a directory called `name`.
    staging = root / f".{name}.artifact-dirs"
    staging.mkdir()
    for entry in entries:
        entry.rename(staging / entry.name)
    staging.rename(root / name)
    return f"{root}: one artifact unpacked flat, moved into {root / name}"


def jobs_steps(workflow: str) -> list[list[str]]:
    """Each `steps:` list in a workflow's text, every step as its own text.

    Indentation alone, no YAML parser (the scripts are stdlib only): a step is an
    item `- ` at the list's first item's indent, until the next one or a line at
    or left of `steps:` itself. Comment lines are dropped.
    """
    blocks, lines = [], workflow.splitlines()
    for i, line in enumerate(lines):
        if line.strip() != "steps:":
            continue
        depth, item, block = len(line) - len(line.lstrip()), None, []
        for body in lines[i + 1:]:
            indent, text = len(body) - len(body.lstrip()), body.strip()
            if not text or text.startswith("#"):
                continue
            if indent <= depth and not text.startswith("- "):
                break
            if text.startswith("- ") and item in (None, indent):
                item = indent
                block.append([body])
            elif block:
                block[-1].append(body)
        blocks.append(["\n".join(step) for step in block])
    return blocks


def unsplit_downloads(workflow: str) -> list[str]:
    """Each `pattern:` download whose lone match would stay flat, with what to do.

    A `download-artifact` step with `pattern:` passes when the very next step of
    its job runs `artifact-dirs.py <its path>`, or when it sets
    `merge-multiple: true` with a `#` reason on that line (one directory for all
    matches, on purpose).
    """
    found = []
    for block in jobs_steps(workflow):
        for k, step in enumerate(block):
            if "actions/download-artifact@" not in step or not re.search(r"(?m)^\s+pattern:", step):
                continue
            if re.search(r"(?m)^\s+merge-multiple:\s*true\s+#\s*\S", step):
                continue
            path = re.search(r"(?m)^\s+path:\s*(\S+)\s*$", step)
            after = block[k + 1] if k + 1 < len(block) else ""
            if path and re.search(rf"artifact-dirs\.py\s+{re.escape(path[1])}(\s|$)", after):
                continue
            first = step.strip().splitlines()[0].strip()
            found.append(
                f"`{first}` downloads by `pattern:`, which unpacks a lone match flat: follow it with a step "
                f"running `python3 .github/scripts/artifact-dirs.py <its path>` (a `path:` is needed), or set "
                f"`merge-multiple: true  # <why one directory is right>`.")
    return found


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: artifact-dirs.py <download dir>")
    try:
        print(normalise(Path(sys.argv[1])))
    except ValueError as err:
        print(f"::error::{err}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
