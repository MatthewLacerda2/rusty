#!/usr/bin/env python3
"""#2430: the heads are read from origin and fetched by sha, so no local ref another git
process can move sits between the fetch and the read (`refs/bench/pr-<n>` was gone twice)."""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

from bench import trees

LISTING = "aaa\trefs/heads/main\nbbb\trefs/pull/42/head\n"


def fake(listing=LISTING, fail_fetch=None):
    calls = []

    def git(argv, **_):
        calls.append(argv)
        if "ls-remote" in argv:
            return listing
        if "fetch" in argv and fail_fetch:
            raise RuntimeError(fail_fetch)
        return ""
    return calls, git


class Fetch(unittest.TestCase):
    def test_both_heads_come_from_the_remote_and_are_fetched_by_sha(self):
        calls, git = fake()
        self.assertEqual(trees.fetch(42, git), ("aaa", "bbb"))
        self.assertIn(["git", "ls-remote", "origin", "refs/heads/main", "refs/pull/42/head"], calls)
        self.assertEqual(calls[-1][-2:], ["aaa", "bbb"])
        self.assertFalse([a for argv in calls for a in argv if "refs/bench" in a or a == "FETCH_HEAD"],
                         "no local ref is written or read back")
        self.assertFalse(any("rev-parse" in argv for argv in calls), "nothing is read after the fetch")

    def test_without_a_pr_only_main(self):
        calls, git = fake("aaa\trefs/heads/main\n")
        self.assertEqual(trees.fetch(None, git), ("aaa", None))
        self.assertEqual(calls[0][-1], "refs/heads/main")

    def test_a_pr_origin_lacks_or_a_failed_fetch_is_one_line_saying_what_to_run(self):
        for listing, fail in (("aaa\trefs/heads/main\n", None), (LISTING, "fatal: remote error:\n  not our ref")):
            _, git = fake(listing, fail)
            with self.assertRaises(RuntimeError) as said:
                trees.fetch(42, git, again="`make bench-ab PR=42`")
            text = str(said.exception)
            self.assertNotIn("\n", text)
            self.assertIn("PR #42", text)
            self.assertIn("run `make bench-ab PR=42` again", text)


def sh(cwd, *argv) -> str:
    return subprocess.run(["git", "-C", str(cwd), *argv], check=True, text=True, capture_output=True).stdout


class RealGit(unittest.TestCase):
    """A local origin with a `refs/pull/7/head`; another process deletes every
    `refs/bench/*` ref between each git call this one makes."""

    def test_a_ref_deleted_between_the_calls_cannot_lose_the_head(self):
        base = Path(tempfile.mkdtemp())
        origin, clone = base / "origin", base / "clone"
        subprocess.run(["git", "init", "-q", "-b", "main", str(origin)], check=True)
        for msg in ("one", "two"):
            sh(origin, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "--allow-empty", "-m", msg)
        sh(origin, "update-ref", "refs/pull/7/head", "HEAD~1")
        subprocess.run(["git", "clone", "-q", str(origin), str(clone)], check=True)

        def git(argv, **_):
            out = trees.run(argv[:1] + ["-C", str(clone)] + argv[1:])
            for ref in sh(clone, "for-each-ref", "--format=%(refname)", "refs/bench").split():
                sh(clone, "update-ref", "-d", ref)  # the other process
            return out
        main, head = trees.fetch(7, git)
        self.assertEqual(main, sh(origin, "rev-parse", "main").strip())
        self.assertEqual(head, sh(origin, "rev-parse", "refs/pull/7/head").strip())
        self.assertEqual(sh(clone, "cat-file", "-t", head).strip(), "commit", "fetched, by sha")
        self.assertEqual(sh(clone, "for-each-ref", "refs/bench"), "", "no ref was written")


if __name__ == "__main__":
    unittest.main()
