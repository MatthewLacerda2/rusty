"""#2667: the queue's state travels in the lock's commit, so a cloud fire's memory outlives it."""

from __future__ import annotations

import unittest

from orchestrating import carry
from tests.orchestrating.fakes import CLOUD, LOCAL, OTHER_CLOUD, Remote


def write(side, name: str, text: str) -> None:
    side.directory.mkdir(parents=True, exist_ok=True)
    (side.directory / name).write_text(text)


class Carrying(unittest.TestCase):
    def setUp(self):
        self.remote = Remote(self)
        self.cloud, self.local = self.remote.side("a", CLOUD), self.remote.side("b", LOCAL)

    def test_a_release_saves_the_state_and_the_next_side_restores_it(self):
        self.cloud.take()
        write(self.cloud, "handed-back.json", '{"7": "red"}\n')
        write(self.cloud, "conflicts.jsonl", '{"pr": 7}\n')
        self.cloud.release()
        refused, restored = self.local.take()
        self.assertIsNone(refused)
        self.assertEqual(restored, ["conflicts.jsonl", "handed-back.json"])
        self.assertEqual((self.local.directory / "handed-back.json").read_text(), '{"7": "red"}\n')

    def test_a_renewal_saves_it_too_so_a_holder_that_died_leaves_its_last_state(self):
        self.cloud.take()
        write(self.cloud, "board.json", "[1]\n")
        self.cloud.renew()
        self.remote.clock.advance(60)
        self.assertEqual(self.local.take()[1], ["board.json"])

    def test_restoring_never_deletes_a_file_the_lock_does_not_carry(self):
        write(self.local, "coders-cap.json", '{"cap": 4}\n')
        self.cloud.take()
        write(self.cloud, "board.json", "[]\n")
        self.cloud.release()
        self.local.take()
        self.assertTrue((self.local.directory / "coders-cap.json").exists())

    def test_the_log_locks_and_temporaries_stay_on_their_machine(self):
        for name in ("queue.log", "stop", ".board.json.part", "board.json"):
            write(self.cloud, name, "x")
        self.assertEqual([p.name for p in carry.carried(self.cloud.directory)], ["board.json"])

    def test_a_local_side_taking_back_its_own_release_keeps_its_newer_files(self):
        self.local.take()
        write(self.local, "coders-waiting.json", "[1]\n")
        self.local.release()
        write(self.local, "coders-waiting.json", "[1, 2]\n")  # a `make coder` after the release
        self.assertEqual(self.local.take()[1], [])
        self.assertEqual((self.local.directory / "coders-waiting.json").read_text(), "[1, 2]\n")

    def test_every_fresh_cloud_fire_restores_the_previous_fires_state(self):
        self.cloud.take()
        write(self.cloud, "quiet.json", "{}\n")
        self.cloud.release()
        self.assertEqual(self.remote.side("c", OTHER_CLOUD).take()[1], ["quiet.json"])

    def test_restores_is_by_place(self):
        self.assertTrue(carry.restores("cloud", "cloud"))
        self.assertTrue(carry.restores("cloud", "local@desk"))
        self.assertTrue(carry.restores("local@desk", "cloud"))
        self.assertFalse(carry.restores("local@desk", "local@desk"))
        self.assertFalse(carry.restores("", "cloud"))  # never taken


if __name__ == "__main__":
    unittest.main()
