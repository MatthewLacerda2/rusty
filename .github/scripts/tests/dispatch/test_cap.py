"""#2630: a cap on cloud coders below the limit, set by `make coders --cap N` and read by every launch."""

from __future__ import annotations

import unittest

from dispatch import cap, model, running, waiting
from tests.dispatch.coder_fakes import ROUND, Board, brief_on, busy, drafts


def board(*running_on: int) -> Board:
    """Coders with drafts on `running_on`, issues 9 and 10 open and briefed."""
    fake = Board({}, routines=busy(*running_on), pulls=drafts(*running_on))
    for number in (9, 10):
        fake.open(number)
    return fake


def models(fake: Board) -> list[str]:
    return [b["job_config"]["ccr"]["session_context"]["model"] for b in fake.created()]


class Setting(unittest.TestCase):
    def test_a_cap_above_the_limit_or_under_one_is_refused_and_nothing_written(self):
        fake = board()
        for asked in (str(running.LIMIT + 1), "0", "four"):
            with self.subTest(asked):
                code, out = fake.coders("--cap", asked)
                self.assertEqual(code, 1)
                self.assertIn(f"from 1 to {running.LIMIT}", out)
        self.assertFalse(cap.path(fake.line).exists())

    def test_the_cap_is_kept_said_and_lifted_at_the_limit(self):
        fake = board(1, 2)
        code, out = fake.coders("--cap", "4")
        self.assertEqual((code, cap.read(fake.line)), (0, (4, None)))
        self.assertIn(f"running 2 of 4 (the cap; the limit is {running.LIMIT})", out)
        fake.coders("--cap", str(running.LIMIT))
        self.assertFalse(cap.path(fake.line).exists())
        self.assertIn(f"running 2 of {running.LIMIT}:\n", fake.coders()[1])

    def test_the_cap_goes_to_the_lock_too_and_a_refused_push_is_said(self):
        fake = board()
        fake.coders("--cap", "3")
        self.assertEqual(fake.shared, [3])
        fake.unshared = OSError("HTTP 403")
        code, out = fake.coders("--cap", "2")
        self.assertEqual((code, cap.read(fake.line)), (0, (2, None)))
        self.assertIn("the cap holds on this machine only", out)

    def test_a_dry_run_says_the_cap_and_writes_nothing(self):
        fake = board(1)
        _, out = fake.coders("--dry-run", "--cap", "3")
        self.assertIn("would set the cap 3", out)
        self.assertIn("running 1 of 3 (the cap", out)
        self.assertFalse(cap.path(fake.line).exists())
        self.assertFalse(hasattr(fake, "shared"))

    def test_a_file_that_holds_no_cap_is_ignored_and_said(self):
        fake = board()
        for held in ('{"cap": 9}', "{", '{"cap": true}'):
            with self.subTest(held):
                cap.path(fake.line).parent.mkdir(parents=True, exist_ok=True)
                cap.path(fake.line).write_text(held)
                ceiling, why = cap.read(fake.line)
                self.assertEqual(ceiling, running.LIMIT)
                self.assertIn("ignored", why)


class Honoured(unittest.TestCase):
    def test_under_the_cap_a_launch_goes(self):
        fake = board(1, 2)
        fake.coders("--cap", "3")
        code, _, err = fake.run("9", "--dry-run")
        self.assertIn("running 2 of 3 (the cap", err)
        self.assertEqual(fake.run("9")[0], 0)
        self.assertEqual(fake.launched(), [9])

    def test_at_the_cap_make_coder_waits_and_the_pass_launches_nothing(self):
        fake = board(1, 2, 3)
        fake.coders("--cap", "3")
        code, _, err = fake.run("9")
        self.assertEqual((code, fake.created()), (0, []))
        self.assertIn("the cap is 3", err)
        self.assertEqual([e["issue"] for e in waiting.load(fake.line)], [9])
        _, out = fake.coders()
        self.assertEqual(fake.created(), [])
        self.assertIn("no free slot", out)
        fake.coders("--cap", "4")
        self.assertEqual(fake.launched(), [9], "raising the cap launches from the line")

    def test_a_round_relaunches_its_coder_in_review_at_the_cap(self):
        fake = board(1, 2, 3)
        fake.pulls = [*drafts(1, 2), {"number": 103, "draft": False, "head": {"ref": "3-x"}}]
        fake.open(3, comments=[brief_on(3, body=ROUND)])
        fake.coders("--cap", "3")
        self.assertIn("running 3 of 3", fake.coders()[1])  # the ready PR holds its slot
        self.assertEqual(fake.run("9")[0], 0)
        self.assertEqual(fake.launched(), [], "a new issue waits")
        self.assertEqual(fake.run("3")[0], 0)
        self.assertEqual(fake.launched(), [3], "the round takes the slot it held")


class Dispatched(unittest.TestCase):
    def test_a_minor_issue_launches_on_sonnet_and_says_why(self):
        fake = board()
        fake.issues[9][0]["labels"] = [{"name": "minor"}]
        code, out, _ = fake.run("9")
        self.assertEqual((code, models(fake)), (0, [model.SONNET]))
        self.assertIn("(labelled `minor`)", out)

    def test_documentation_only_is_refused_unless_forced_and_force_survives_the_line(self):
        fake = board(1, 2, 3, 4, 5)
        fake.issues[9][0]["labels"] = [{"name": "documentation"}]
        code, _, err = fake.run("9")
        self.assertEqual((code, waiting.load(fake.line)), (1, []))
        self.assertIn("the orchestrator writes it", err)
        self.assertEqual(fake.run("9", "--force")[0], 0)
        fake.pulls, fake.routines = [], busy()
        fake.coders()
        self.assertEqual((fake.launched(), models(fake)), ([9], [model.MODEL]))


if __name__ == "__main__":
    unittest.main()
