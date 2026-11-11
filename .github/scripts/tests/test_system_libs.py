#!/usr/bin/env python3
"""Workflows install apt packages only through `.github/actions/system-libs` (#2101).

A bare `sudo apt-get update && sudo apt-get install` reaches Ubuntu's mirror on
every job; on 2026-11-11 the runners could not resolve it for a quarter of an
hour and the queue handed back PRs #2094 and #2096 twice each, their code
green. The action installs the `.deb`s from the Actions cache, so a direct
call is flagged here (an exception goes on its line: `# system-libs: <reason>`).

`install.sh` is then run against a fake `apt-get`, `dpkg` and `dpkg-query` on
`PATH`: a hit installs with no apt-get call at all, and a miss with the mirror
down fails loudly with nothing passed off as installed.
"""

from __future__ import annotations

import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

GITHUB = Path(__file__).resolve().parents[2]
WORKFLOW_DIR = GITHUB / "workflows"
INSTALL = GITHUB / "actions" / "system-libs" / "install.sh"
DIRECT = re.compile(r"^.*\bapt-get\b.*$", re.M)
ALLOW = re.compile(r"#\s*system-libs:\s*\S")

# The fakes. `apt-get` logs every call; with MIRROR=down it fails `update`'s
# download the way apt does (exit 0, a warning) and the download itself (exit
# 100). `--download-only` writes one `.deb` per package into the archive dir;
# `dpkg -i` and `--no-download` mark packages installed in $STATE.
FAKES = {
    "sudo": 'exec "$@"\n',
    "timeout": 'shift; exec "$@"\n',
    "sleep": "true\n",
    "apt-get": r"""echo "apt-get $*" >> "$LOG"
dir=$(printf '%s\n' "$@" | sed -n 's/^Dir::Cache::archives=//p')
case " $* " in
  *" update "*) grep -q azure "$SOURCES" && [ "$MIRROR" = azure-down ] && exit 100
    [ "$MIRROR" = down ] && echo "W: Temporary failure resolving 'azure.archive.ubuntu.com'"; exit 0;;
  *" --download-only "*) [ "$MIRROR" = down ] && exit 100
    for p in "$@"; do case $p in -*|*=*|install) ;; *) touch "$dir/$p.deb";; esac; done;;
  *" --no-download "*) for p in "$@"; do case $p in -*|*=*|install) ;; *) echo "$p" >> "$STATE";; esac; done;;
esac
""",
    "dpkg": r"""echo "dpkg $*" >> "$LOG"
[ "$DPKG" = broken ] && exit 1
shift; for deb in "$@"; do basename "$deb" .deb >> "$STATE"; done
""",
    "dpkg-query": r"""grep -qx "${!#}" "$STATE" 2>/dev/null && printf installed || exit 1
""",
}


def direct_calls(text: str) -> list[str]:
    """Each line naming `apt-get` outside a comment, without an inline reason."""
    return [m.group(0).strip() for m in DIRECT.finditer(text)
            if not m.group(0).lstrip().startswith("#") and not ALLOW.search(m.group(0))]


def install(hit: bool, mirror: str = "up", dpkg: str = "ok", cached: tuple[str, ...] = ()) -> tuple[int, str, str, list[str]]:
    """Run install.sh for `pkg-a pkg-b` with the fakes; (status, output, the call log, installed)."""
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        bin_dir, archive = root / "bin", root / "archive"
        bin_dir.mkdir()
        archive.mkdir()
        for name, body in FAKES.items():
            (bin_dir / name).write_text("#!/usr/bin/env bash\n" + body)
            (bin_dir / name).chmod(0o755)
        (root / "mirrors.txt").write_text("http://azure.archive.ubuntu.com/ubuntu/\n")
        for deb in cached:
            (archive / f"{deb}.deb").touch()
        env = {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}", "LOG": str(root / "log"),
               "STATE": str(root / "state"), "MIRROR": mirror, "DPKG": dpkg,
               "SOURCES": str(root / "mirrors.txt"), "APT_SOURCES": str(root / "mirrors.txt")}
        done = subprocess.run([str(INSTALL), str(archive), "true" if hit else "false", "pkg-a", "pkg-b"],
                              env=env, capture_output=True, text=True, check=False)
        log = (root / "log").read_text() if (root / "log").exists() else ""
        state = (root / "state").read_text().split() if (root / "state").exists() else []
        return done.returncode, done.stdout + done.stderr, log, state


class Workflows(unittest.TestCase):
    def test_no_workflow_calls_apt_get_directly(self):
        for path in sorted(WORKFLOW_DIR.glob("*.yml")):
            with self.subTest(workflow=path.name):
                self.assertEqual(direct_calls(path.read_text()), [])

    def test_a_direct_call_is_flagged_and_a_reasoned_one_passes(self):
        self.assertEqual(len(direct_calls("        run: sudo apt-get update && sudo apt-get install -y x\n")), 1)
        self.assertEqual(direct_calls("        run: sudo apt-get install -y x  # system-libs: no cache here\n"), [])
        self.assertEqual(direct_calls("      # apt-get from the mirror\n"), [])


class Install(unittest.TestCase):
    def test_a_hit_installs_with_no_network(self):
        status, out, log, state = install(hit=True, mirror="down", cached=("pkg-a", "pkg-b"))
        self.assertEqual(status, 0, out)
        self.assertNotIn("apt-get", log)
        self.assertIn("dpkg -i", log)
        self.assertEqual(sorted(state), ["pkg-a", "pkg-b"])

    def test_a_miss_with_the_mirror_down_fails_loudly(self):
        status, out, log, state = install(hit=False, mirror="down")
        self.assertNotEqual(status, 0)
        self.assertIn("::error::system-libs", out)
        self.assertEqual(log.count(" update"), 3, "three attempts")
        self.assertEqual(state, [])

    def test_a_miss_with_the_mirror_up_downloads_then_installs(self):
        status, out, log, state = install(hit=False)
        self.assertEqual(status, 0, out)
        self.assertIn("Acquire::Retries=", log)
        self.assertLess(log.index("--download-only"), log.index("--no-download"))
        self.assertEqual(sorted(state), ["pkg-a", "pkg-b"])

    def test_a_dead_azure_mirror_falls_back_to_ubuntus_archive(self):
        status, out, log, state = install(hit=False, mirror="azure-down")
        self.assertEqual(status, 0, out)
        self.assertEqual(log.count(" update"), 2, "the Azure mirror, then the archive")
        self.assertEqual(sorted(state), ["pkg-a", "pkg-b"])

    def test_a_hit_that_installs_short_is_flagged(self):
        # A cached entry missing a package (or dpkg failing over it) never passes silently.
        status, out, _log, _state = install(hit=True, mirror="down", cached=("pkg-a",))
        self.assertNotEqual(status, 0)
        self.assertIn("pkg-b", out)

    def test_a_broken_cache_falls_back_to_the_mirror(self):
        status, out, log, state = install(hit=True, dpkg="broken", cached=("pkg-a", "pkg-b"))
        self.assertEqual(status, 0, out)
        self.assertIn("--download-only", log)
        self.assertEqual(sorted(state), ["pkg-a", "pkg-b"])


if __name__ == "__main__":
    unittest.main()
