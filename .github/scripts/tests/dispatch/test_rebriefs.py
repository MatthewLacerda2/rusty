"""#1724: `make rebriefs` counts briefs per issue and re-briefs by their `Round:` line."""

from __future__ import annotations

import contextlib
import io
import unittest
from unittest import mock

from dispatch import rebriefs
from tests import load

cli = load("rebriefs.py")
REPO = "https://api.github.com/repos/o/r/issues/"


def comment(issue: int, when: str, body: str, who: str = "OWNER") -> dict:
    return {"issue_url": f"{REPO}{issue}", "created_at": when, "body": body, "author_association": who}


FIRST = comment(7, "2026-10-29T10:00:00Z", "Brief\n\nBranch: `7-x`.")
SUPERSEDED = comment(7, "2026-10-30T10:00:00Z", "Brief\n\nRound: review\nBranch: `7-x`.")
NEWEST = comment(7, "2026-10-31T10:00:00Z", "Brief\n\nBranch: `7-x`.")  # no Round: line
ONCE = comment(8, "2026-10-30T12:00:00Z", "Brief\n\nBranch: `8-y`.")
CHAT = comment(8, "2026-10-31T12:00:00Z", "Thanks, merged.")
STRANGER = comment(8, "2026-10-31T13:00:00Z", "Brief\n\nRound: ci", who="NONE")


class Round(unittest.TestCase):
    def test_reads_the_cause_through_markdown(self):
        self.assertEqual(rebriefs.round_of("Brief\n\nRound: CI\n"), "ci")
        self.assertEqual(rebriefs.round_of("Brief\n- **Round:** conflict"), "conflict")

    def test_a_missing_or_unknown_round_is_unstated(self):
        self.assertEqual(rebriefs.round_of("Brief\n\nBranch: `x`."), "unstated")
        self.assertEqual(rebriefs.round_of("Brief\nRound: vibes"), "unstated")
        self.assertEqual(rebriefs.round_of("Brief\nAround: review"), "unstated")


class Tally(unittest.TestCase):
    def test_a_superseded_brief_still_counts_and_only_later_ones_are_rebriefs(self):
        counted = rebriefs.tally([NEWEST, ONCE, FIRST, SUPERSEDED, CHAT])
        self.assertEqual(counted.briefs, {7: 3, 8: 1})
        self.assertEqual((counted.total, counted.rebriefs, counted.in_one), (4, 2, 1))
        self.assertEqual(counted.rounds["review"], 1)
        self.assertEqual(counted.rounds["unstated"], 1)

    def test_an_untrusted_brief_is_not_counted(self):
        counted = rebriefs.tally([ONCE, STRANGER])
        self.assertEqual((counted.briefs, counted.rebriefs), ({8: 1}, 0))

    def test_since_never_turns_a_rebrief_into_a_first_brief(self):
        counted = rebriefs.tally([FIRST, SUPERSEDED, NEWEST, ONCE], since="2026-10-30T11:00:00Z")
        self.assertEqual(counted.briefs, {7: 1, 8: 1})
        self.assertEqual((counted.rebriefs, counted.rounds["unstated"]), (1, 1))

    def test_nothing_briefed_says_so(self):
        self.assertEqual(rebriefs.report(rebriefs.tally([CHAT])), "briefs all time: none")

    def test_report_quotes_the_numbers(self):
        text = rebriefs.report(rebriefs.tally([FIRST, SUPERSEDED, NEWEST, ONCE]), "2026-10-29")
        self.assertIn("briefs since 2026-10-29: 4 on 2 issues, 2.00 per issue", text)
        self.assertIn("one brief: 1 of 2 issues", text)
        self.assertIn("re-briefs: 2", text)
        self.assertIn("most briefed: #7 (3)", text)


class CommandLine(unittest.TestCase):
    def run_cli(self, argv: list[str], comments: list[dict] | None) -> tuple[int, str, list[str]]:
        asked: list[str] = []

        def gh(path: str):
            asked.append(path)
            return None if comments is None else (comments if "page=1" in path else [])

        out = io.StringIO()
        with mock.patch.object(cli.ghrest, "slug", return_value="o/r"), \
                contextlib.redirect_stdout(out), contextlib.redirect_stderr(io.StringIO()):
            code = cli.main(argv, gh=gh)
        return code, out.getvalue(), asked

    def test_reads_every_repository_comment_and_prints_the_report(self):
        code, out, asked = self.run_cli(["--since", "2026-10-30"], [FIRST, SUPERSEDED, ONCE])
        self.assertEqual(code, 0)
        self.assertTrue(asked[0].startswith("repos/o/r/issues/comments?"))
        self.assertIn("briefs since 2026-10-30: 2 on 2 issues", out)

    def test_github_failing_or_a_bad_date_exits_2(self):
        self.assertEqual(self.run_cli([], None)[0], 2)
        self.assertEqual(self.run_cli(["--since", "yesterday"], [])[0], 2)

    def test_since_is_normalised_to_utc(self):
        self.assertEqual(cli.since_of(["--since", "2026-10-29"]), "2026-10-29T00:00:00Z")
        self.assertEqual(cli.since_of(["--since", "2026-10-29T18:00-03:00"]), "2026-10-29T21:00:00Z")
