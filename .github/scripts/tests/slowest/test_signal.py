"""The slow-test signal (#2350): a test over the budget in a pull request's
report that `main`'s reports have under it, or lack, is named as new and
annotated, as is one `main` had just under it that grew past the noise; one
over it on both sides is known, listed once; one under it is not named; an unreadable baseline says why, lists the tests over the budget and
exits 0; the top N below it is unchanged."""

from __future__ import annotations

import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import slowest_tests


def report(*cases: tuple[str, float]) -> str:
    rows = "".join(f'<testcase name="{n}" classname="rusty::main" time="{s}"/>' for n, s in cases)
    return f'<testsuites time="99"><testsuite name="rusty::main">{rows}</testsuite></testsuites>'


PR = report(("gpu::new_slow", 31.0), ("gpu::known_slow", 40.0), ("gpu::grew", 25.0),
            ("gpu::quick", 3.0), ("scene::loads", 0.5))
#: `main` in two shards: a `count:` partition moves tests between them.
MAIN_1 = report(("gpu::known_slow", 36.0), ("gpu::quick", 2.0))
MAIN_2 = report(("gpu::grew", 12.0), ("scene::loads", 0.4))


def run(pr: str, *baseline: str, write: bool = True) -> tuple[int, list[str]]:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / "pr.xml").write_text(pr)
        base = root / "main"
        if write:
            for i, xml in enumerate(baseline):
                (base / f"junit-{i}").mkdir(parents=True)
                (base / f"junit-{i}" / "junit-dev.xml").write_text(xml)
        out = io.StringIO()
        with redirect_stdout(out):
            code = slowest_tests.main(["slowest_tests.py", str(root / "pr.xml"), "3", "--baseline", str(base)])
        return code, out.getvalue().splitlines()


class Signal(unittest.TestCase):
    def test_a_test_newly_over_the_budget_is_named_first_with_mains_time(self):
        code, out = run(PR, MAIN_1, MAIN_2)
        self.assertEqual(code, 0)
        self.assertEqual(out[:2], ["new over 20 s: rusty::main gpu::new_slow 31.0 s (main: absent)",
                                   "new over 20 s: rusty::main gpu::grew 25.0 s (main: 12.0 s)"])

    def test_each_new_one_is_annotated_and_nothing_else_is(self):
        _, out = run(PR, MAIN_1, MAIN_2)
        warnings = [l for l in out if l.startswith("::warning")]
        self.assertEqual(len(warnings), 2)
        self.assertTrue(all("New test over 20 s" in w for w in warnings))
        self.assertFalse(any("known_slow" in w for w in warnings))

    def test_one_over_on_both_sides_is_known_once(self):
        _, out = run(PR, MAIN_1, MAIN_2)
        known = [l for l in out if "known_slow" in l and "slowest" not in l and not l.startswith(" ")]
        self.assertEqual(known, ["known over 20 s: rusty::main gpu::known_slow 40.0 s (main: 36.0 s)"])

    def test_one_under_the_budget_is_not_named(self):
        _, out = run(PR, MAIN_1, MAIN_2)
        self.assertFalse(any("quick" in l for l in out if "over 20 s" in l))

    def test_one_main_had_just_under_the_budget_is_near_not_new(self):
        _, out = run(report(("gpu::edge", 20.2)), report(("gpu::edge", 17.7)))
        self.assertEqual(out[:2], ["new over 20 s: none (against 1 tests of main)",
                                   "near over 20 s: rusty::main gpu::edge 20.2 s (main: 17.7 s)"])
        self.assertFalse(any(l.startswith("::warning") for l in out))

    def test_one_in_the_band_that_grew_past_the_noise_is_new(self):
        _, out = run(report(("gpu::regressed", 90.0)), report(("gpu::regressed", 16.0)))
        self.assertEqual(out[0], "new over 20 s: rusty::main gpu::regressed 90.0 s (main: 16.0 s)")
        self.assertEqual(sum(l.startswith("::warning") for l in out), 1)

    def test_one_over_on_main_stays_known_however_it_grew(self):
        _, out = run(report(("gpu::known_slow", 90.0)), MAIN_1)
        self.assertEqual(out[1], "known over 20 s: rusty::main gpu::known_slow 90.0 s (main: 36.0 s)")

    def test_nothing_new_says_so(self):
        _, out = run(report(("gpu::known_slow", 30.0)), MAIN_1)
        self.assertEqual(out[0], "new over 20 s: none (against 2 tests of main)")

    def test_no_baseline_says_why_lists_the_slow_ones_and_passes(self):
        for name, args in (("absent", dict(write=False)), ("malformed", {})):
            with self.subTest(name):
                code, out = run(PR, "<testsuites><testcase", **args)
                self.assertEqual(code, 0)
                self.assertTrue(out[0].startswith("no baseline: "), out[0])
                self.assertIn("over 20 s: rusty::main gpu::known_slow 40.0 s", out)
                self.assertFalse(any(l.startswith("::warning") for l in out))

    def test_the_top_n_below_is_unchanged(self):
        _, out = run(PR, MAIN_1, MAIN_2)
        top = out.index(next(l for l in out if l.startswith("the 3 slowest tests")))
        self.assertEqual([l.split()[-1] for l in out[top + 1:top + 4]],
                         ["gpu::known_slow", "gpu::new_slow", "gpu::grew"])


class Regex(unittest.TestCase):
    def test_a_filter_whose_regex_does_not_compile_is_named_not_raised(self):
        config = ('[test-groups]\ngpu = { max-threads = 2 }\n[[profile.default.overrides]]\n'
                  "filter = '(kind(test) & test(/^gpu(::/)) | test(/x/)'\ntest-group = \"gpu\"\n")
        with self.assertRaisesRegex(ValueError, "does not compile"):
            slowest_tests.gpu_rule(config)


if __name__ == "__main__":
    unittest.main()
