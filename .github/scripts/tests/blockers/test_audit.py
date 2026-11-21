"""#624: a blocker written only in prose is found, and `--fix` records it."""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, FakeGh, blockers, issue, run


class Audit(unittest.TestCase):
    def fake(self) -> FakeGh:
        return FakeGh(
            bodies={
                399: "Blocked by #396 and #397",  # 396 open, 397 closed
                400: "Blocked by #396",  # already recorded
                401: "Blocked by #401 and #402",  # self, and a pull request
                402: "nothing here",
            },
            issues=[issue(396), issue(397, "closed"), issue(402, pull_request={})],
            recorded={400: [issue(396)]},
        )

    def test_reports_only_open_unrecorded_blockers_and_fails(self):
        fake = self.fake()
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#399 blocked by #396 (t396): in the body, not recorded on GitHub", out)
        self.assertNotIn("#397", out)
        self.assertNotIn("#400 blocked", out)
        self.assertNotIn("#401 blocked", out)
        self.assertEqual(fake.posts, [], "an audit writes nothing")

    def test_fix_records_each_by_database_id(self):
        fake = self.fake()
        code, out = run(fake, "--fix")
        self.assertEqual(code, 0)
        self.assertEqual(fake.posts, [(f"repos/{REPO}/issues/399/dependencies/blocked_by",
                                       "issue_id=9396")])
        self.assertIn("#399 blocked by #396 (t396): recorded", out)

    def test_a_clean_board_passes(self):
        fake = FakeGh({1: "Blocked by #2"}, [issue(2)], {1: [issue(2)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts), (0, []))
        self.assertIn("is recorded", out)

    def test_unknown_flags_are_refused(self):
        with self.assertRaises(SystemExit):
            blockers.main(["--fxi"], blockers.Gh(self.fake()))


if __name__ == "__main__":
    unittest.main()
