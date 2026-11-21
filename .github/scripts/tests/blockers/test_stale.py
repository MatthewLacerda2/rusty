"""#2489: a recorded open blocker no body states is reported (stale, or mentioned
but not as a blocker) with the `--lift` that removes it; `--fix` never deletes."""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, FakeGh, issue, run

FOREIGN = "https://api.github.com/repos/x/y"


class Stale(unittest.TestCase):
    def test_a_blocker_named_nowhere_is_reported_and_fails(self):
        fake = FakeGh({5: "Was waiting on the cull; lifted, see the comment."}, [], {5: [issue(7)]})
        code, out = run(fake)
        self.assertEqual(code, 1)
        self.assertIn("#5 blocked by #7 (t7): recorded on GitHub, named in neither body: "
                      "lift it with `make blockers ARGS='--lift 5 7'`", out)
        self.assertIn("1 stale", out)
        self.assertEqual((fake.posts, fake.deletes), ([], []), "an audit writes nothing")

    def test_fix_reports_it_and_sends_no_delete(self):
        for body in ("Was waiting on the cull.", "~~Blocked by #7~~ (the cull landed)",
                     "~~Blocked by\r\n#7~~", "Waits on #70 and x/y#7"):
            with self.subTest(body=body):
                fake = FakeGh({5: body}, [], {5: [issue(7)]})
                code, out = run(fake, "--fix")
                self.assertEqual((code, fake.deletes), (1, []))
                self.assertIn("#5 blocked by #7 (t7): recorded on GitHub, named in neither body", out)
                self.assertIn("ARGS='--lift 5 7'", out)

    def test_fix_records_the_gaps_beside_a_stale_one_and_still_fails(self):
        fake = FakeGh({5: "Blocked by #6"}, [issue(6)], {5: [issue(7)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, len(fake.posts), fake.deletes), (1, 1, []))
        self.assertIn("#5 blocked by #6 (t6): recorded", out)

    def test_a_blocker_named_anywhere_is_never_removed(self):
        cases = {
            "Blocked by #6 (after #7)": "a reason",
            "Blocked by the bake and\nthe cull, #7": "a wrapped line",
            "Blocked by:\r\n- #6, then\r\n  maybe #7": "a CRLF bulleted list",
            f"Waits on {REPO}#7": "this repository's own form",
            "Not any more:\nblocked by #7": "a \"not\" on the line above",
            "Blocked by e.g. #7": "an abbreviation",
            "No longer blocked by #7": "a negation that still names it",
            "See #7": "a mention",
        }
        for body, why in cases.items():
            with self.subTest(why):
                fake = FakeGh({5: body}, [issue(6)], {5: [issue(7)]})
                code, out = run(fake, "--fix")
                self.assertEqual(fake.deletes, [], why)
                if "#5 blocked by #7 (t7): recorded, and mentioned in #5's body" in out:
                    self.assertEqual(code, 1, why)
                    self.assertIn("make blockers ARGS='--lift 5 7'", out)
                else:
                    self.assertEqual(code, 0, f"{why}: read as a blocker, so it agrees")

    def test_agreeing_or_closed_blockers_pass(self):
        fake = FakeGh({1: "Blocked by #2", 3: "nothing", 4: "Blocked by:\r\n- #8\r\n- #9"}, [],
                      {1: [issue(2)], 3: [issue(6, "closed")], 4: [issue(8), issue(9)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts, fake.deletes), (0, [], []))
        self.assertIn("every recorded one is in a body", out)

    def test_the_blockers_own_body_keeps_it(self):
        fake = FakeGh({5: "silent"}, [issue(7, body="Blocks #5 (the queue needs it)")], {5: [issue(7)]})
        code, _ = run(fake, "--fix")
        self.assertEqual((code, fake.deletes, fake.fetched), (0, [], [7]))
        fake = FakeGh({5: "silent"}, [issue(7, body="Blocks: the cull (#5)")], {5: [issue(7)]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.deletes), (1, []))
        self.assertIn("mentioned in #7's body but not as a blocker", out)

    def test_an_unread_clause_is_reported_whatever_the_record_holds(self):
        for recorded in ([], [issue(7), issue(8)]):
            with self.subTest(recorded=recorded):
                fake = FakeGh({5: "Blocked by #7 or #8"}, [issue(7), issue(8)], {5: recorded},
                              summary={5: len(recorded)})
                code, out = run(fake, "--fix")
                self.assertEqual((code, fake.deletes), (1, []))
                self.assertIn("#5: a \"Blocked by\" holds #8 in words it could not read", out)
                self.assertNotIn("every recorded one", out)

    def test_the_third_reviews_wordings_cost_no_deletion(self):
        cases = {  # body -> exit code; each fails but "a ~~ b ~~", which is not struck
            "Blocked by issue 7": 1, "Blocked by the shader rewrite.": 1,
            "Blocked by github.com/o/r/issues/7": 1, "Blocked by [the cull](../issues/7)": 1,
            "Blocked by #6\n- #7 the follow-up": 1, "a ~~ b ~~ Blocked by #7": 0,
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                fake = FakeGh({5: body}, [issue(6)], {5: [issue(7)]})
                code, out = run(fake, "--fix")
                self.assertEqual((code, fake.deletes), (want, []))
                if want:
                    self.assertIn("ARGS='--lift 5 7'", out)

    def test_a_blocker_in_another_repository_is_left_alone(self):
        foreign = issue(7, repository_url=FOREIGN)
        fake = FakeGh({5: "Blocked by #7"}, [issue(7, "closed")], {5: [foreign]})
        code, out = run(fake, "--fix")
        self.assertEqual((code, fake.posts, fake.deletes), (0, [], []))
        self.assertIn("#5 blocked by x/y#7: in another repository, left alone", out)
        fake = FakeGh({6: "silent"}, [], {6: [foreign]})
        self.assertEqual((run(fake, "--fix")[0], fake.deletes), (0, []))

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
