"""`slowest_tests.gpu_rule` reads the cap of the GPU group this platform's
override picks, as nextest does: the first matching override (#2363)."""

from __future__ import annotations

import unittest

import slowest_tests

FILTER = "(kind(test) & test(/^gpu::/)) | test(/(^|::)gpu_[^:]*$/)"
MACOS = """[test-groups]
gpu = {{ max-threads = 2 }}
gpu-macos = {{ max-threads = 3 }}
[[profile.default.overrides]]
platform = '{platform}'
filter = '{macos}'
test-group = "gpu-macos"
[[profile.default.overrides]]
filter = '{filter}'
test-group = "gpu"
"""


def config(platform: str = 'cfg(target_os = "macos")', macos: str = FILTER) -> str:
    return MACOS.format(platform=platform, macos=macos, filter=FILTER)


try:
    import tomllib  # 3.11+, which `gpu_rule` needs
except ModuleNotFoundError:
    tomllib = None


@unittest.skipUnless(tomllib, "tomllib needs Python 3.11+")
class Platform(unittest.TestCase):
    def test_macos_reads_its_own_group_and_linux_the_other(self):
        self.assertEqual(slowest_tests.gpu_rule(config(), "darwin")[1], 3)
        self.assertEqual(slowest_tests.gpu_rule(config(), "linux")[1], 2)

    def test_both_platforms_take_the_same_tests(self):
        case = slowest_tests.Case("rusty::main", "gpu::frame::draws", 1)
        for platform in ("darwin", "linux"):
            self.assertTrue(slowest_tests.gpu_rule(config(), platform)[0](case), platform)

    def test_two_filters_are_named_not_split_two_ways(self):
        with self.assertRaisesRegex(ValueError, "2 filters"):
            slowest_tests.gpu_rule(config(macos="test(/(^|::)gpu_[^:]*$/)"), "darwin")

    def test_a_platform_it_cannot_read_is_named(self):
        with self.assertRaisesRegex(ValueError, "not one of"):
            slowest_tests.gpu_rule(config(platform='cfg(target_os = "linux")'), "linux")

    def test_a_platform_table_is_named_not_a_traceback(self):
        table = config().replace("platform = 'cfg(target_os = \"macos\")'",
                                 "platform = { host = 'cfg(target_os = \"macos\")' }")
        with self.assertRaisesRegex(ValueError, "not a `cfg"):
            slowest_tests.gpu_rule(table, "darwin")


if __name__ == "__main__":
    unittest.main()
