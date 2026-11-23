#!/usr/bin/env python3
"""The root `Cargo.toml`'s per-crate profile tables stay true to the lockfile (#2320).

`[profile.dev.package."*"]` builds every dependency at opt-level 2; a crate no test or
frame runs hot gets its own `[profile.<p>.package.<name>]` table back at 0. Two rules
keep those tables honest, and cargo enforces neither:

- **Every named crate is in `Cargo.lock`.** When one leaves the lock, cargo only warns
  (`profile package spec ... did not match any packages`), and the line lingers.
- **A table tagged `# host-only` names a crate only dependencies use.** Such a proc
  macro or build helper runs while a dependency compiles, once per clean build. A
  workspace member that depends on it directly, or on the crate it is reached through
  (the tag's `via <crate>, ...`: `thiserror` for `thiserror-impl`), would run it
  unoptimised on every rebuild of the edit loop. The lock is read for members'
  direct dependencies: it has every kind (normal, dev, build) and resolves renames.
  A crate `[patch.crates-io]` points at a path (`vendor/lua-src`, #2636) has no `source`
  in the lock either, yet is a dependency, not a member: `patched` drops it.
"""

from __future__ import annotations

import re
import unittest
from pathlib import Path

try:
    import tomllib  # 3.11+; CI's runner has it, an older local python3 skips
except ModuleNotFoundError:
    tomllib = None

REPO = Path(__file__).resolve().parents[3]

#: `[profile.dev.package.cc]  # host-only` or `... # host-only, via thiserror, x`.
TABLE = re.compile(
    r'^\[profile\.[\w-]+\.package\.([\w-]+)\][ \t]*(?:#[ \t]*(host-only)(?:,[ \t]*via[ \t]+([^\n#]+))?)?',
    re.M,
)


def tables(manifest: str) -> dict[str, set[str] | None]:
    """Each named package table: None if untagged, else the `host-only` crate's facades."""
    out: dict[str, set[str] | None] = {}
    for name, tag, via in TABLE.findall(manifest):
        out[name] = {v.strip() for v in via.split(",") if v.strip()} if tag else None
    return out


def lock_packages(lock: str) -> tuple[set[str], dict[str, set[str]]]:
    """Every package name in the lock, and each workspace member's direct dependencies."""
    packages = tomllib.loads(lock)["package"]
    members = {
        p["name"]: {d.split()[0] for d in p.get("dependencies", [])}
        for p in packages
        if "source" not in p
    }
    return {p["name"] for p in packages}, members


def stale(manifest: str, lock: str) -> list[str]:
    names, _ = lock_packages(lock)
    return [
        f"Cargo.toml names `{n}` in a profile table, but `{n}` is no longer in Cargo.lock; "
        f"drop its `[profile.*.package.{n}]` table."
        for n in tables(manifest)
        if n not in names
    ]


def patched(manifest: str) -> set[str]:
    """What `[patch.*]` points at a path: a dependency, though the lock gives it no source."""
    return {name for table in tomllib.loads(manifest).get("patch", {}).values() for name in table}


def host_only_used(manifest: str, lock: str) -> list[str]:
    _, members = lock_packages(lock)
    for name in patched(manifest):
        members.pop(name, None)
    found = []
    for crate, via in tables(manifest).items():
        if via is None:
            continue
        for member, deps in sorted(members.items()):
            for used in sorted(deps & ({crate} | via)):
                found.append(
                    f"workspace member `{member}` depends on `{used}`, so it runs `{crate}`, "
                    f"which Cargo.toml builds at opt-level 0 as host-only (`# host-only`): "
                    f"move `{crate}` back to opt-level 2 (delete its table) or don't depend on `{used}`."
                )
    return found


LOCK = """
[[package]]
name = "game"
version = "0.1.0"
dependencies = ["thiserror 2.0.1", "serde"]

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["serde_derive"]

[[package]]
name = "serde_derive"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "thiserror"
version = "2.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
dependencies = ["thiserror-impl"]

[[package]]
name = "thiserror-impl"
version = "2.0.1"
source = "registry+https://github.com/rust-lang/crates.io-index"
"""


@unittest.skipIf(tomllib is None, "needs python 3.11's tomllib")
class DevProfile(unittest.TestCase):
    def test_the_repository_holds_both_rules(self):
        manifest = (REPO / "Cargo.toml").read_text()
        lock = (REPO / "Cargo.lock").read_text()
        self.assertTrue(any(v is not None for v in tables(manifest).values()), "no `# host-only` tag")
        self.assertEqual(stale(manifest, lock), [])
        self.assertEqual(host_only_used(manifest, lock), [])

    def test_a_table_whose_crate_left_the_lock_is_flagged(self):
        manifest = "[profile.dev.package.serde]\nopt-level = 0\n[profile.dev.package.gone]\nopt-level = 0\n"
        (message,) = stale(manifest, LOCK)
        self.assertIn("`gone`", message)
        self.assertIn("drop its", message)

    def test_tables_of_locked_crates_pass(self):
        self.assertEqual(stale('[profile.dev.package."*"]\n[profile.dev.package.serde_derive]\n', LOCK), [])

    def test_a_member_using_a_host_only_crate_through_its_facade_is_flagged(self):
        manifest = "[profile.dev.package.thiserror-impl]  # host-only, via thiserror\nopt-level = 0\n"
        (message,) = host_only_used(manifest, LOCK)
        self.assertIn("`game` depends on `thiserror`", message)
        self.assertIn("back to opt-level 2", message)

    def test_a_member_using_a_host_only_crate_directly_is_flagged(self):
        lock = LOCK.replace('["thiserror 2.0.1", "serde"]', '["serde_derive"]')
        manifest = "[profile.dev.package.serde_derive] # host-only\nopt-level = 0\n"
        self.assertEqual(len(host_only_used(manifest, lock)), 1)

    def test_host_only_crates_only_dependencies_use_pass(self):
        manifest = "[profile.dev.package.serde_derive]  # host-only, via serde_json\nopt-level = 0\n"
        self.assertEqual(host_only_used(manifest, LOCK), [])

    def test_a_patched_dependency_is_no_member(self):
        lock = LOCK.replace('name = "game"', 'name = "vendored"').replace('"thiserror 2.0.1", ', "")
        manifest = '[profile.dev.package.serde]  # host-only\nopt-level = 0\n'
        self.assertEqual(len(host_only_used(manifest, lock)), 1)
        patch = '[patch.crates-io]\nvendored = { path = "vendor/vendored" }\n'
        self.assertEqual(host_only_used(manifest + patch, lock), [])

    def test_an_untagged_table_is_not_held_to_the_host_only_rule(self):
        self.assertEqual(host_only_used("[profile.dev.package.thiserror-impl]\nopt-level = 0\n", LOCK), [])


if __name__ == "__main__":
    unittest.main()
