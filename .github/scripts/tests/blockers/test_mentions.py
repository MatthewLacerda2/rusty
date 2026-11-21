"""What `deps.prose.stated` reads as a stated blocker, and what it reports it could
not read (#2489). Recording rests on it, so it reads at least every wording the
old one-regex parser did, and every input of both reviews of PR #2516."""

from __future__ import annotations

import unittest

from tests.blockers.fakes import REPO, prose


def read(body):
    return tuple(prose.stated(body, "blocked by", REPO))


class Mentions(unittest.TestCase):
    def test_reads_every_spelling_of_a_list(self):
        cases = {
            "Blocked by #396": [396],
            "blocked BY #396, #397 and #398 & #399 + #400": [396, 397, 398, 399, 400],
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
                self.assertEqual(read(body), (want, []))

    def test_reads_what_the_second_review_found_lost(self):
        cases = {
            "Blocked by:\r\n- #7\r\n- #8\r\n": [7, 8],  # the web editor's CRLF
            "Blocked by #7 and\n#8": [7, 8],  # a wrapped line, which `main` read
            "Blocked by\n#7": [7],
            "Blocked by:\n- #7 (the bake must\n  land first)\n- #8": [7, 8],  # a wrapped bullet
            "Blocked by:\n- #7 and\n  #8": [7, 8],
            "Blocked by:\n\n\n- #7\n\n- #8": [7, 8],  # a loose list after blank lines
            "Blocked by: #7\n- #8": [7, 8],  # mixed
            f"Blocked by {REPO}#7 and {REPO.upper()}#8": [7, 8],  # this repository's own form
            "Not yet.\nBlocked by #7": [7],  # a negation on the line above
            "## Never\nBlocked by #7": [7],
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                self.assertEqual(read(body), (want, []))

    def test_a_number_in_a_reason_is_not_a_blocker(self):
        self.assertEqual(read("Blocked by #7 (see #3)"), ([7], []))
        self.assertEqual(read("Blocked by #7 (PR #9, the\ncull)."), ([7], []))

    def test_a_bullet_beside_the_verbs_own_is_another_issues_line(self):
        body = "- #401 surface — blocked by #398, #399\n- #402 postfx — blocked by #397\n"
        self.assertEqual(read(body), ([398, 399, 397], []))
        self.assertEqual(read("- Blocked by:\n  - #7\n  - #8"), ([7, 8], []))

    def test_another_repositorys_issue_is_taken_but_not_numbered(self):
        for body in ("Blocked by x/y#7 and #8", "Blocked by https://github.com/x/y/issues/7, #8"):
            with self.subTest(body=body):
                self.assertEqual(read(body), ([8], []))

    def test_ignores_lifted_or_absent_blockers(self):
        for body in ("No longer blocked by #5", "not **blocked by** #5", "unblocked by #5",
                     "~~Blocked by #5~~", "- ~~**Blocked by #5**~~ lifted", "~~Blocked by\r\n#5~~",
                     "Blocks #5", "", None):
            with self.subTest(body=body):
                self.assertEqual(read(body), ([], []))

    def test_a_reference_it_did_not_take_is_reported(self):
        cases = {
            "Blocked by #7 or #8": ["#8"], "Blocked by: the bake (#7)": ["#7"],
            "Blocked by the bake, #7": ["#7"], "Blocked by:\n- the bake, see #7": ["#7"],
            "Blocked by e.g. #7": ["#7"], "Blocked by the bake and\nthe cull #8.": ["#8"],
            "Blocked by #7 and therefore by #6, which": ["#6"],
        }
        for body, want in cases.items():
            with self.subTest(body=body):
                self.assertEqual(read(body)[1], want)

    def test_a_number_read_elsewhere_is_not_reported(self):
        self.assertEqual(read("**Blocked by #412**, until #412 lands"), ([412], []))

    def test_blocks_reads_the_same_lists(self):
        self.assertEqual(tuple(prose.stated("Blocks #5 and #6", "blocks", REPO)), ([5, 6], []))
        self.assertEqual(tuple(prose.stated("~~Blocks #5~~", "blocks", REPO)), ([], []))


class Named(unittest.TestCase):
    """The token search a deletion rests on: any sentence, any form, not struck."""

    def test_names_every_form_and_drops_struck_text(self):
        body = (f"see #7, {REPO}#8, https://github.com/{REPO}/issues/9 and x/y#10;\r\n"
                "~~#11~~ #120")
        self.assertEqual(prose.named(body, REPO), {7, 8, 9, 120})
        self.assertEqual(prose.named(None, REPO), set())


if __name__ == "__main__":
    unittest.main()
