"""#2490, #2630: the coder's model follows the issue's labels; its `Model:` line overrides them."""

from __future__ import annotations

import unittest

from dispatch import brief, model, rebriefs, routine
from tests.dispatch.test_routine import BRIEF_URL, NOW


def issue(*labels: str, body: str = "") -> dict:
    return {"number": 7, "state": "open", "body": body, "labels": [{"name": n} for n in labels]}


def briefed(body: str, who: str = "OWNER") -> dict:
    return {"body": body, "author_association": who, "created_at": "2026-11-22T10:00:00Z", "user": {"login": "o"}}


class ModelLine(unittest.TestCase):
    def test_absent_means_the_labels_choice(self):
        for body in ("", "**Suggested model: Opus** (the default; no `Model:` line).",
                     "```lua\nmodel: player.glb\n```", "Model: the mesh the importer reads", "~~~\nModel: gpt\n~~~"):
            with self.subTest(body):
                self.assertEqual(model.choose(issue("bug", body=body)), model.Choice(model.MODEL, "the default"))

    def test_each_accepted_value_maps_to_its_id(self):
        for name, wanted in model.MODELS.items():
            for line in (f"Model: {name}", f"model: {name.upper()} \r", f"  Model:{name}"):
                with self.subTest(line):
                    self.assertEqual(model.choose(issue(body=f"intro\n{line}\nrest")).model, wanted)
        made = routine.body(42, "t", BRIEF_URL, "e", "o/r", NOW, model=model.SONNET)
        self.assertEqual(made["job_config"]["ccr"]["session_context"]["model"], "claude-sonnet-5-5")

    def test_an_unknown_value_or_two_lines_is_refused_naming_the_accepted_ones(self):
        for body in ("Model: gpt", "Model: gpt-9", "Model:", "```\n```\nModel: gpt", "Model: opus\nModel: sonnet"):
            with self.subTest(body), self.assertRaises(ValueError) as raised:
                model.choose(issue(body=body))
            self.assertIn("opus, sonnet", str(raised.exception))

    def test_the_brief_is_refused_before_the_line_is_joined(self):
        with self.assertRaises(brief.Refused) as raised:
            brief.read(issue(body="Model: haiku"), [])
        self.assertIn("haiku", str(raised.exception))


class Labels(unittest.TestCase):
    def test_plain_runs_on_opus_and_minor_on_sonnet(self):
        self.assertEqual(model.choose(issue("infrastructure")).model, model.MODEL)
        self.assertEqual(model.choose(issue("bug", "minor")), model.Choice(model.SONNET, "labelled `minor`"))

    def test_the_body_line_wins_over_the_label(self):
        picked = model.choose(issue("bug", "minor", body="Model: opus"))
        self.assertEqual(picked, model.Choice(model.MODEL, "the issue's `Model: opus` line"))

    def test_a_minor_issue_sent_back_twice_goes_back_to_opus(self):
        once = [briefed("Brief\nx"), briefed("Brief\nRound: review\ny")]
        twice = [*once, briefed("Brief\nRound: ci\nz"), briefed("Brief\nRound: review", who="NONE")]
        self.assertEqual(model.choose(issue("minor"), rebriefs.rounds(once)).model, model.SONNET)
        picked = model.choose(issue("minor"), rebriefs.rounds(twice))
        self.assertEqual(picked.model, model.MODEL)
        self.assertIn("sent it back 2 times", picked.why)
        self.assertEqual(model.choose(issue("bug"), 5).model, model.MODEL)

    def test_documentation_only_is_the_orchestrators_unless_forced(self):
        doc = issue("documentation")
        self.assertIn("the orchestrator writes it", model.stop(doc))
        with self.assertRaises(brief.Refused):
            brief.read(doc, [briefed("Brief\nx")])
        self.assertEqual(brief.read(doc, [briefed("Brief\nx")], force=True)["body"], "Brief\nx")
        for dispatched in (issue("documentation", "bug"), issue("documentation", body="Model: sonnet"), issue()):
            with self.subTest(dispatched["labels"]):
                self.assertIsNone(model.stop(dispatched))

    def test_two_conflicting_signals_are_refused_naming_both(self):
        why = model.stop(issue("documentation", "minor"))
        self.assertIn("`documentation` alone and `minor`", why)
        with self.assertRaises(brief.Refused):
            brief.read(issue("documentation", "minor"), [briefed("Brief\nx")])


if __name__ == "__main__":
    unittest.main()
