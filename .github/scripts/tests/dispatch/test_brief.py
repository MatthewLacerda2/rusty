"""#1569: the newest `Brief` is found, its `Model:` read, and a bad launch refused."""

from __future__ import annotations

import unittest

from dispatch import brief


def comment(body: str, at: str = "2026-10-29T10:00:00Z", who: str = "OWNER") -> dict:
    return {"body": body, "created_at": at, "author_association": who, "user": {"login": "someone"},
            "html_url": f"https://example/{at}"}


def issue(state: str = "open", labels: tuple[str, ...] = ("infrastructure",)) -> dict:
    return {"number": 7, "state": state, "labels": [{"name": n} for n in labels]}


BRIEF = "Brief\n\nBranch: `7-x`.\nDecisions: none.\nModel: Opus.\n"


class Finding(unittest.TestCase):
    def test_a_brief_starts_with_the_word(self):
        self.assertTrue(brief.is_brief(BRIEF))
        self.assertTrue(brief.is_brief("\n**Brief** (re-brief)\nModel: Sonnet"))

    def test_other_comments_are_not_briefs(self):
        self.assertFalse(brief.is_brief("Briefly: this is not one.\nModel: Opus"))
        self.assertFalse(brief.is_brief("A note.\nBrief\nModel: Opus"))
        self.assertFalse(brief.is_brief(""))

    def test_the_newest_brief_wins(self):
        old = comment(BRIEF, "2026-10-28T10:00:00Z")
        new = comment(BRIEF.replace("Opus", "Sonnet"), "2026-10-29T10:00:00Z")
        chat = comment("Not a brief", "2026-10-30T10:00:00Z")
        self.assertIs(brief.newest([new, chat, old]), new)
        self.assertIsNone(brief.newest([chat]))


class Model(unittest.TestCase):
    def test_reads_opus_and_sonnet(self):
        self.assertEqual(brief.model_of(BRIEF), brief.MODELS["opus"])
        self.assertEqual(brief.model_of("Brief\n**Model:** Sonnet, by *Which model*"), brief.MODELS["sonnet"])

    def test_refuses_no_model_and_an_unknown_one(self):
        with self.assertRaisesRegex(brief.Refused, "no `Model:` line"):
            brief.model_of("Brief\nBranch: x")
        with self.assertRaisesRegex(brief.Refused, "Haiku"):
            brief.model_of("Brief\nModel: Haiku")


class Read(unittest.TestCase):
    def test_an_open_issue_with_a_trusted_brief_launches(self):
        found, model = brief.read(issue(), [comment(BRIEF)])
        self.assertEqual(found["body"], BRIEF)
        self.assertEqual(model, brief.MODELS["opus"])

    def test_refusals(self):
        cases = {
            "closed": (issue(state="closed"), [comment(BRIEF)]),
            "planning": (issue(labels=("planning", "agent")), [comment(BRIEF)]),
            "human": (issue(labels=("human",)), [comment(BRIEF)]),
            "no comment starting with": (issue(), [comment("hello")]),
            "cannot push": (issue(), [comment(BRIEF, who="NONE")]),
            "pull request": ({**issue(), "pull_request": {}}, [comment(BRIEF)]),
        }
        for why, (subject, comments) in cases.items():
            with self.subTest(why), self.assertRaisesRegex(brief.Refused, why):
                brief.read(subject, comments)


if __name__ == "__main__":
    unittest.main()
