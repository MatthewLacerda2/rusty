"""#2489: a recorded open blocker no body states is stale, and `--fix` removes it.

A deletion needs proof the relationship is stated nowhere: a body the parser could
not read, a "Blocks #N" in the blocker's own body, or a blocker in another
repository keeps it.
"""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, FakeGh, issue, run

FOREIGN = "https://api.github.com/repos/x/y"


class Stale(unittest.TestCase):
    def test_a_lifted_dependency_is_reported_and_fails(self):
        fake = FakeGh({5: "Was waiting on the cull; lifted, see the comment."}, [],
                      {5: [issue(7)]})
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#5 blocked by #7 (t7): recorded on GitHub, not in either body", out)
        self.assertIn("1 stale", out)
        self.assertEqual((fake.posts, fake.deletes), ([], []), "an audit writes nothing")

    def test_fix_removes_it_by_database_id(self):
        for body in ("No longer blocked by #7", "~~Blocked by #7~~ (the cull landed)"):
            with self.subTest(body=body):
                fake = FakeGh({5: body}, [], {5: [issue(7)]})
                code, out = run(fake, "--fix")
                self.assertEqual(code, 0)
                self.assertEqual(fake.deletes, [f"repos/{REPO}/issues/5/dependencies/blocked_by/9007"])
                self.assertIn("#5 blocked by #7 (t7): removed", out)

    def test_agreeing_closed_or_reworded_blockers_pass(self):
        fake = FakeGh(
            bodies={
                1: "Blocked by #2",  # body and relationship agree
                3: "nothing",  # the recorded blocker is closed: history
                4: "**Blocked by:** **#8** (needs the bake) and #9",  # other accepted wording
                10: "Blocked by #11, #12, and #13",
                14: "Blocked by #15 and by #16",
                17: "Blocked by:\n- #18 (the bake)\n- #19",
                20: f"Blocked by https://github.com/{REPO}/issues/21",
                22: "Blocked by #23 (see #24)",
            },
            issues=[],
            recorded={1: [issue(2)], 3: [issue(6, "closed")], 4: [issue(8), issue(9)],
                      10: [issue(11), issue(12), issue(13)], 14: [issue(15), issue(16)],
                      17: [issue(18), issue(19)], 20: [issue(21)], 22: [issue(23)]},
        )
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts, fake.deletes), (0, [], []))
        self.assertIn("every recorded one is in a body", out)

    def test_the_blockers_own_blocks_line_keeps_it(self):
        fake = FakeGh({5: "silent"}, [issue(7, body="Blocks #5 (the queue needs it)")], {5: [issue(7)]})
        code, _ = run(fake, "--fix")
        self.assertEqual((code, fake.deletes, fake.fetched), (0, [], [7]))

    def test_a_body_it_could_not_read_keeps_every_recorded_blocker(self):
        fake = FakeGh({5: "Blocked by #7 or #8"}, [], {5: [issue(7), issue(8)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.deletes), (1, []))
        self.assertIn("#5 blocked by #8 (t8): recorded on GitHub; a \"Blocked by\"", out)
        self.assertNotIn("#5 blocked by #7", out, "#7 was read, so it is not in doubt")

    def test_an_unreadable_blocks_line_keeps_it_too(self):
        fake = FakeGh({5: "silent"}, [issue(7, body="Blocks: the cull (#5)")], {5: [issue(7)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.deletes), (1, []))
        self.assertIn("could not be read", out)

    def test_a_blocker_in_another_repository_is_left_alone(self):
        foreign = issue(7, repository_url=FOREIGN)
        fake = FakeGh({5: "Blocked by #7"}, [issue(7, "closed")], {5: [foreign]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts, fake.deletes), (0, [], []))
        self.assertIn("#5 blocked by x/y#7: in another repository, left alone", out)
        fake = FakeGh({6: "silent"}, [], {6: [foreign]})
        self.assertEqual(run(fake, "--fix")[0], 0)
        self.assertEqual(fake.deletes, [])

    def test_a_bare_number_is_not_matched_against_another_repository(self):
        own = issue(7, repository_url=f"https://api.github.com/repos/{REPO}")
        fake = FakeGh({5: "Blocked by #7"}, [own], {5: [issue(7, repository_url=FOREIGN)]})
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#5 blocked by #7 (t7): in the body, not recorded on GitHub", out)

    def test_an_issue_listed_without_open_blockers_is_not_asked(self):
        # (open, total): #1 none, #2 one open, #3 only a closed one, #4 states one
        fake = FakeGh({1: "plain", 2: "plain", 3: "plain", 4: "Blocked by #9"}, [issue(9)],
                      {2: [issue(8)], 4: [issue(9)]}, summary={1: 0, 2: 1, 3: (0, 1), 4: (0, 0)})
        code, _ = run(fake)
        self.assertEqual((code, fake.asked), (1, [2, 4]))


if __name__ == "__main__":
    unittest.main()
