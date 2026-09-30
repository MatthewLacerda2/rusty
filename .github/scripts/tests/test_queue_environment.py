#!/usr/bin/env python3
"""#580: a check that fails for the machine's reasons stops the queue.

The first real run of the pre-push check filled a tmpfs and handed back three
healthy pull requests as broken. These pin both halves: the failure is named
as the machine's and stops everything, and a tmpfs is refused before it starts.
"""

from __future__ import annotations

import importlib.util
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parent.parent / "merge-queue.py"
_spec = importlib.util.spec_from_file_location("merge_queue_env", SCRIPT)
queue = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(queue)

# Verbatim from the run that motivated #580.
QUOTA = (
    "error: failed to write to `.../wt-q/target/merge-queue/debug/deps/rmeta16Wp8f/full.rmeta`:"
    " Disk quota exceeded (os error 122)\n"
    "error: could not compile `jiff` (lib) due to 1 previous error"
)
MACHINE = (
    QUOTA,
    "error: could not write output to foo.o: No space left on device (os error 28)",
    "error: could not compile `rusty` (lib)\nCaused by:\n  process didn't exit successfully:"
    " `rustc ...` (signal: 9, SIGKILL: kill)",
    "collect2: fatal error: ld terminated with signal 9 [Killed]",
    "memory allocation of 1048576 bytes failed",
)
CODE = (
    "error[E0061]: this method takes 2 arguments but 3 were supplied",
    "error[E0425]: cannot find value `signal` in this scope",
)


def failing(output: str):
    return mock.Mock(return_value=subprocess.CompletedProcess([], 101, "", output))


class Classifying(unittest.TestCase):
    def test_out_of_disk_or_memory_stops_and_blames_the_machine(self):
        for output in MACHINE:
            with self.subTest(output=output[:40]), self.assertRaises(queue.Stopped) as caught:
                queue.verify("/tmp/wt", "/q/target", runner=failing(output))
            said = " ".join(caught.exception.lines)
            self.assertIn("ran out of disk or memory", said)
            self.assertIn("nothing is known to be wrong", said)
            self.assertIn(".claude/worktrees/", said)
            self.assertNotIn("a merge ahead", said)

    def test_the_real_quota_failure_is_quoted_with_the_room_left(self):
        with self.assertRaises(queue.Stopped) as caught:
            queue.verify("/tmp/wt", "/q/target", runner=failing(QUOTA))
        said = " ".join(caught.exception.lines)
        self.assertIn("os error 122", said)
        self.assertIn("GB free under /q/target", said)

    def test_a_compile_error_is_still_the_branchs(self):
        for output in CODE:
            with self.subTest(output=output):
                notes = queue.verify("/tmp/wt", "/q/target", runner=failing(output))
                self.assertIn("a merge ahead", " ".join(notes))


class Draining(unittest.TestCase):
    def test_a_stop_takes_no_more_and_names_the_rest(self):
        def turn(number):
            if number == 2:
                raise queue.Stopped(["the machine ran out of disk or memory during cargo check (dev)."])
            return number, queue.MERGED, "CI passed"

        with mock.patch("builtins.print"):
            results = queue.drain([1, 2, 3, 4], turn)
        self.assertEqual([state for _, state, _ in results], [queue.MERGED, queue.STOPPED, queue.NOT_TAKEN, queue.NOT_TAKEN])
        self.assertIn("stopped at #2", results[3][2])

    def test_a_hand_back_still_carries_on(self):
        turn = mock.Mock(side_effect=[(1, queue.HANDED_BACK, "conflicts"), (2, queue.MERGED, "ok")])
        self.assertEqual(len(queue.drain([1, 2], turn)), 2)


class Preflight(unittest.TestCase):
    MOUNTS = (
        "22 1 254:0 / / rw - ext4 /dev/vda rw\n"
        "30 22 0:26 / /tmp rw,nosuid shared:5 - tmpfs tmpfs rw\n"
        "31 22 0:27 / /tmpx rw - ext4 /dev/vdb rw\n"
    )

    def kind(self, path):
        with tempfile.NamedTemporaryFile("w", suffix=".mountinfo") as info:
            info.write(self.MOUNTS)
            info.flush()
            with mock.patch.object(queue.os.path, "realpath", side_effect=lambda p: path):
                return queue.mount_type(path, info.name)

    def test_the_longest_mount_point_containing_the_path_wins(self):
        self.assertEqual(self.kind("/tmp/scratch/wt/target/merge-queue"), "tmpfs")
        self.assertEqual(self.kind("/tmpx/target"), "ext4")
        self.assertEqual(self.kind("/home/user/rusty/target"), "ext4")

    def test_no_mountinfo_is_no_answer(self):
        self.assertIsNone(queue.mount_type("/x", "/nonexistent/mountinfo"))

    def test_a_tmpfs_target_refuses_before_asking_github(self):
        with mock.patch.object(queue, "mount_type", return_value="tmpfs"), \
            mock.patch.object(queue.mergeable, "gh") as gh, mock.patch("builtins.print") as printed:
            self.assertEqual(queue.main(["524"]), 1)
        gh.assert_not_called()
        self.assertIn(".claude/worktrees/", " ".join(str(c) for c in printed.call_args_list))

    def test_a_run_that_builds_nothing_is_not_refused(self):
        with mock.patch.object(queue, "mount_type", return_value="tmpfs"):
            self.assertTrue(queue.preflight("/q"))
        for flag in ("--dry-run", "--no-check"):
            with mock.patch.object(queue, "preflight") as pre, \
                mock.patch.object(queue.mergeable, "gh", side_effect=SystemExit(9)):
                with self.assertRaises(SystemExit):
                    queue.main(["524", flag])
            pre.assert_not_called()


if __name__ == "__main__":
    unittest.main()
