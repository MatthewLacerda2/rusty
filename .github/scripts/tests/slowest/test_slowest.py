"""`slowest_tests.py` reads nextest's JUnit report (#1976, #2326): the longest
first, capped, a case without a readable time skipped, a missing report no
failure; and the shard's accounting, the `gpu` group (its filter and cap read from
`.config/nextest.toml`) against the rest, per binary, with the GPU floor."""

from __future__ import annotations

import io
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import slowest_tests

REPORT = """<?xml version="1.0" encoding="UTF-8"?>
<testsuites><testsuite name="rusty">
<testcase name="a::fast" classname="rusty" time="0.010"/>
<testcase name="a::slow" classname="rusty" time="96.500"/>
<testcase name="a::middle" classname="rusty" time="4.200"><failure/></testcase>
<testcase name="a::untimed" classname="rusty"/>
</testsuite><testsuite name="main"><testcase name="gpu::frame" classname="main" time="12.0"/></testsuite>
</testsuites>"""


class Slowest(unittest.TestCase):
    def test_the_longest_come_first_and_the_list_is_capped(self):
        self.assertEqual(slowest_tests.slowest(slowest_tests.cases(REPORT)[0], 3),
                         [(96.5, "rusty a::slow"), (12.0, "main gpu::frame"), (4.2, "rusty a::middle")])

    def test_a_case_without_a_time_is_skipped(self):
        names = [name for _, name in slowest_tests.slowest(slowest_tests.cases(REPORT)[0], 10)]
        self.assertEqual(len(names), 4)
        self.assertNotIn("rusty a::untimed", names)

    def test_a_missing_report_informs_and_passes(self):
        out = io.StringIO()
        with redirect_stdout(out):
            code = slowest_tests.main(["slowest_tests.py", "/nonexistent/junit.xml"])
        self.assertEqual(code, 0)
        self.assertIn("no timings", out.getvalue())


#: Two binaries, as nextest names them: the integration binary `rusty::main` and
#: the library `rusty-render`, whose own `gpu::` module is not GPU by path alone.
SHARD = """<testsuites name="nextest-run" time="40.0">
<testsuite name="rusty::main">
<testcase name="gpu::frame::draws" classname="rusty::main" time="10.0"/>
<testcase name="gpu::frame::lights" classname="rusty::main" time="6.0"/>
<testcase name="scene::loads" classname="rusty::main" time="1.0"/>
</testsuite><testsuite name="rusty-render">
<testcase name="taa::tests::gpu_shimmer" classname="rusty-render" time="4.0"/>
<testcase name="gpu::tests::a_cpu_reference" classname="rusty-render" time="2.0"/>
<testcase name="setup::tests::budget" classname="rusty-render" time="1.0"/>
</testsuite></testsuites>"""

NO_GPU = '<testsuites time="3.0"><testsuite><testcase name="a" classname="rusty-ai" time="1.5"/></testsuite></testsuites>'


def run(xml: str, config: Path = slowest_tests.NEXTEST, cores: int = 4) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        report = Path(tmp) / "junit.xml"
        report.write_text(xml)
        out = io.StringIO()
        with redirect_stdout(out):
            assert slowest_tests.main(["slowest_tests.py", str(report)], config, cores) == 0
        return out.getvalue()


try:
    import tomllib  # 3.11+; `gpu_rule` needs it, so the accounting tests skip without it
except ModuleNotFoundError:
    tomllib = None

NEEDS_TOML = unittest.skipUnless(tomllib, "tomllib needs Python 3.11+")
#: This platform's cap, as the script reads it: 2 on Linux, 3 on macOS (#2363).
CAP = slowest_tests.gpu_rule(slowest_tests.NEXTEST.read_text())[1] if tomllib else 0


@NEEDS_TOML
class Accounting(unittest.TestCase):
    def test_the_group_against_the_rest_with_the_repos_own_filter_and_cap(self):
        out = run(SHARD)
        self.assertIn(f"gpu: 3 tests, 20 s, S/cap {CAP} = {20 / CAP:.0f} s; rest: 3 tests, 4 s, R/cores 4 = 1 s; (S+R)/cores = 6 s; "
                      "shard wall 40 s", out)
        self.assertIn("gpu tests: median 6.000 s, p10 4.000 s", out)

    def test_seconds_per_binary_heaviest_first(self):
        out = run(SHARD).split("test-seconds per binary:\n", 1)[1].splitlines()
        self.assertEqual([l.split() for l in out],
                         [["17.0s", "3", "tests", "rusty::main"], ["7.0s", "3", "tests", "rusty-render"]])

    def test_a_shard_without_gpu_tests_prints_zero_not_an_error(self):
        out = run(NO_GPU)
        self.assertIn(f"gpu: 0 tests, 0 s, S/cap {CAP} = 0 s; rest: 1 tests, 2 s", out)
        self.assertNotIn("median", out)

    def test_kind_test_is_the_integration_binary_only(self):
        self.assertTrue(slowest_tests.is_integration("rusty::main"))
        for binary in ("rusty-render", "rusty::bin/player", "rusty::example/demo"):
            self.assertFalse(slowest_tests.is_integration(binary), binary)

    def test_the_repos_filter_is_the_shape_the_script_reads(self):
        # A change of the `gpu` filter's shape in nextest.toml fails here first,
        # so the split never silently drifts from what nextest groups.
        is_gpu, cap = slowest_tests.gpu_rule(slowest_tests.NEXTEST.read_text())
        self.assertGreater(cap, 0)
        self.assertTrue(is_gpu(slowest_tests.Case("rusty::main", "gpu::x::y", 1)))

    def test_a_filter_of_another_shape_is_named_and_the_top_n_still_prints(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = Path(tmp) / "nextest.toml"
            config.write_text('[test-groups]\ngpu = { max-threads = 2 }\n'
                              '[[profile.default.overrides]]\nfilter = "test(/^gpu/)"\ntest-group = "gpu"\n')
            out = run(SHARD, config)
        self.assertIn("the 6 slowest tests", out)
        self.assertIn("no group accounting", out)
        self.assertIn("FILTER_SHAPE", out)

    def test_a_malformed_report_informs_and_passes(self):
        out = run("<testsuites><testcase")
        self.assertIn("no timings", out)


if __name__ == "__main__":
    unittest.main()
