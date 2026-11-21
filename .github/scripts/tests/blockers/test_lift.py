"""#2489: `--lift <issue> <blocker>` removes one recorded relationship, whatever
the body says, and says what the body still holds."""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, FakeGh, blockers, issue, run


class Lift(unittest.TestCase):
    def test_removes_the_one_relationship_and_nothing_else(self):
        fake = FakeGh({}, [issue(5, body="Lifted, see the comment.")], {5: [issue(6), issue(7)]})
        code, out = run(fake, "--lift", "5", "#7")
        self.assertEqual((code, fake.deletes, fake.posts),
                         (0, [f"repos/{REPO}/issues/5/dependencies/blocked_by/9007"], []))
        self.assertIn("#5 blocked by #7 (t7): lifted", out)
        self.assertNotIn("still", out)

    def test_says_what_the_body_still_holds(self):
        for body, says in (("Blocked by #7", "still says it is blocked by #7"),
                           ("See #7 (the cull)", "still names #7, not as a blocker")):
            with self.subTest(body=body):
                fake = FakeGh({}, [issue(5, body=body)], {5: [issue(7)]})
                self.assertIn(says, run(fake, "--lift", "5", "7")[1])

    def test_nothing_recorded_is_nothing_lifted(self):
        foreign = issue(7, repository_url="https://api.github.com/repos/x/y")
        fake = FakeGh({}, [], {5: [foreign]})
        code, out = run(fake, "--lift", "5", "7")
        self.assertEqual((code, fake.deletes), (0, []))
        self.assertIn("#5 is not recorded as blocked by #7: nothing to lift", out)

    def test_refuses_a_malformed_lift(self):
        for argv in (["--lift", "5"], ["--lift", "5", "x"], ["--lift", "5", "7", "8"], ["--fix", "x"]):
            with self.subTest(argv=argv), self.assertRaises(SystemExit):
                blockers.main(argv, blockers.Gh(FakeGh({}, [])))


if __name__ == "__main__":
    unittest.main()
