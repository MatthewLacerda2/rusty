"""What `blockers.stated` reads as a stated blocker, and when it says it could not
read one: a doubt keeps a recorded blocker, so every wording a person writes for a
list must read, and anything else must say so (#2489)."""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, blockers


def read(body):
    return tuple(blockers.stated(body, "blocked by", REPO))


class Mentions(unittest.TestCase):
    def test_reads_every_spelling_of_a_list(self):
        cases = {
            "Blocked by #396": [396],
            "blocked BY #396, #397 and #398 & #399": [396, 397, 398, 399],
            "Blocked by #7, #8, and #9": [7, 8, 9],
            "Blocked by #7 and by #9": [7, 9],
            "**Blocked by:** #12\n- blocked by #12": [12],
            "## Depends\nBlocked by #1. See also #2.": [1],
            "Blocked by:\n- #7 (the bake)\n- **#8**\n\nSee #4 too.": [7, 8],
            "**Blocked by:**\n\n* [ ] #7 — the bake\n1. #8": [7, 8],
            f"Blocked by https://github.com/{REPO}/issues/7 and #8": [7, 8],
            # #399's own body, the incident behind #624.
            "**Blocked by #396** (materials must reference shaders first) and **#393** (naming).":
                [396, 393],
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                self.assertEqual(read(body), (want, False))

    def test_a_number_in_a_reason_is_not_a_blocker(self):
        self.assertEqual(read("Blocked by #7 (see #3)"), ([7], False))

    def test_another_repositorys_issue_is_taken_but_not_numbered(self):
        for body in ("Blocked by x/y#7 and #8", "Blocked by https://github.com/x/y/issues/7, #8"):
            with self.subTest(body=body):
                self.assertEqual(read(body), ([8], False))

    def test_ignores_lifted_or_absent_blockers(self):
        for body in ("No longer blocked by #5", "not **blocked by** #5", "unblocked by #5",
                     "~~Blocked by #5~~", "- ~~**Blocked by #5**~~ lifted", "Blocks #5", "", None):
            with self.subTest(body=body):
                self.assertEqual(read(body), ([], False))

    def test_a_reference_it_did_not_take_is_a_doubt(self):
        for body in ("Blocked by #7 or #8", "Blocked by: the bake (#7)", "Blocked by the bake, #7",
                     "Blocked by:\n- the bake, see #7", "Blocked by:\n#7 #8 are in the way"):
            with self.subTest(body=body):
                self.assertTrue(read(body)[1])

    def test_blocks_reads_the_same_lists(self):
        self.assertEqual(tuple(blockers.stated("Blocks #5 and #6", "blocks", REPO)), ([5, 6], False))
        self.assertEqual(tuple(blockers.stated("~~Blocks #5~~", "blocks", REPO)), ([], False))


if __name__ == "__main__":
    unittest.main()
