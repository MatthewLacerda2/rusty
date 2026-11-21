"""What decides a skip is held in both places that list it (#2333).

`proved_tree.DECIDER` must name every file under `.github/scripts` that running
`proved_tree.py` imports (followed through, each package's `__init__.py` on the
way included), and each gating workflow's `decider:` filter must match every
`DECIDER` path and both workflow files. Both are derived, never a copied list,
so a new import or a new script file fails here until both lists carry it.
"""

from __future__ import annotations

import ast
import re
import unittest
from pathlib import Path

import proved_tree
from tests import SCRIPTS
from tests.proved.test_wiring import body, job

REPO = SCRIPTS.parent.parent


def module_files(name: str, root: Path = SCRIPTS) -> list[Path]:
    """The files importing `name` runs under `root`: each package's `__init__.py`
    on the way, then the module; empty when it is not there (the stdlib)."""
    parts, found = name.split("."), []
    for depth in range(1, len(parts) + 1):
        here = root.joinpath(*parts[:depth])
        if (here / "__init__.py").is_file():
            found.append(here / "__init__.py")
        elif here.with_suffix(".py").is_file() and depth == len(parts):
            found.append(here.with_suffix(".py"))
        else:
            return found if depth > 1 else []
    return found


def imported(source: str, root: Path = SCRIPTS) -> set[Path]:
    """Every file under `root` that running `source` imports, followed through."""
    seen: set[Path] = set()
    todo = [source]
    while todo:
        for node in ast.walk(ast.parse(todo.pop())):
            names = ([a.name for a in node.names] if isinstance(node, ast.Import)
                     else [node.module] if isinstance(node, ast.ImportFrom) and node.module and not node.level
                     else [])
            for name in names:
                for path in module_files(name, root):
                    if path not in seen:
                        seen.add(path)
                        todo.append(path.read_text(encoding="utf-8"))
    return seen


def decider_globs(workflow: str) -> list[str]:
    text = job(body(workflow), "changes")
    block = re.search(r"\n            decider:\n((?:(?: {14}- | {12}#).*\n)+)", text).group(1)
    return re.findall(r"^ {14}- '([^']+)'$", block, re.M)


def matches(path: str, glob: str) -> bool:
    """paths-filter's picomatch, for the shapes the filter uses: `**` any depth, `*` one name."""
    pattern = re.escape(glob).replace(r"\*\*", "\0").replace(r"\*", "[^/]*").replace("\0", ".*")
    return re.fullmatch(pattern, path) is not None


def missing(paths: set[Path]) -> set[str]:
    return {p.relative_to(REPO).as_posix() for p in paths} - set(proved_tree.DECIDER)


def uncovered(paths: tuple[str, ...], globs: list[str]) -> list[str]:
    return [p for p in paths if not any(matches(p, g) for g in globs)]


class Decider(unittest.TestCase):
    def test_decider_names_every_file_the_script_imports(self):
        source = (SCRIPTS / "proved_tree.py").read_text(encoding="utf-8")
        self.assertEqual(missing(imported(source) | {SCRIPTS / "proved_tree.py"}), set())

    def test_a_new_sibling_import_is_flagged(self):
        source = "import ghrest\nimport blockers\nfrom merging.verdict.runlist import base\nimport json\n"
        # blockers.py brings what it imports too (deps/), so only its own row is asserted.
        self.assertIn(".github/scripts/blockers.py", missing(imported(source)))

    def test_a_package_import_brings_its_init_files(self):
        got = {p.relative_to(SCRIPTS).as_posix() for p in imported("from merging.verdict.runlist import base")}
        self.assertLessEqual({"merging/__init__.py", "merging/verdict/__init__.py", "merging/verdict/runlist.py"}, got)

    def test_each_workflow_filter_matches_the_decider_and_both_workflows(self):
        paths = (*proved_tree.DECIDER, *(f".github/workflows/{w}" for w in proved_tree.BUILT))
        for workflow in proved_tree.BUILT:
            with self.subTest(workflow=workflow):
                self.assertEqual(uncovered(paths, decider_globs(workflow)), [])

    def test_a_path_the_filter_misses_is_flagged(self):
        globs = [".github/scripts/merging/verdict/**", ".github/scripts/ghrest.py"]
        self.assertEqual(uncovered((".github/scripts/merging/__init__.py", ".github/scripts/ghrest.py",
                                    ".github/scripts/merging/verdict/runlist.py"), globs),
                         [".github/scripts/merging/__init__.py"])
        self.assertFalse(matches(".github/scripts/ghrest.pyc", ".github/scripts/ghrest.py"))


if __name__ == "__main__":
    unittest.main()
