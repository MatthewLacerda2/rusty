"""main-health.sh waits out a jobs API that lags the run's conclusion (#1200).

A stubbed `gh` answers the jobs endpoint from a queue of canned bodies (the last
one repeats), so a job can read `null` first and `success` on the re-read. The
script runs with `DRY_RUN=true`, so an issue it would file is printed, not made.
"""

import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

from tests import SCRIPTS

RUN = {"name": "ci", "event": "push", "head_branch": "main", "head_sha": "abc",
       "html_url": "http://run", "conclusion": "success"}
GH = """#!/usr/bin/env bash
dir=$(dirname "$0"); args="$*"; jq_expr=''
while [[ $# -gt 0 ]]; do [[ $1 == --jq ]] && jq_expr=$2; shift; done
case "$args" in
  *"/jobs?filter=latest"*)
    n=$(cat "$dir/n" 2>/dev/null || echo 0); echo $((n + 1)) >"$dir/n"
    f="$dir/jobs.$n"; [[ -f $f ]] || f=$(ls "$dir"/jobs.* | sort | tail -n1)
    jq -r "$jq_expr" "$f" ;;
  *"/actions/runs/"*) cat "$dir/run.json" ;;
  *) ;;
esac
"""


def jobs(*conclusions):
    return json.dumps({"jobs": [{"name": f"job{i}", "conclusion": c} for i, c in enumerate(conclusions)]})


class JobRace(unittest.TestCase):
    def run_health(self, *reads):
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            (d / "gh").write_text(GH)
            (d / "gh").chmod(0o755)
            (d / "run.json").write_text(json.dumps(RUN))
            for i, body in enumerate(reads):
                (d / f"jobs.{i}").write_text(body)
            env = {**os.environ, "PATH": f"{d}:{os.environ['PATH']}", "REPO": "o/r", "RUN_ID": "1",
                   "DRY_RUN": "true", "JOBS_RETRY_INTERVAL": "0", "JOBS_RETRY_ATTEMPTS": "3"}
            out = subprocess.run(["bash", str(SCRIPTS / "main-health.sh")], env=env, text=True,
                                 capture_output=True, check=True).stdout
            return out, int((d / "n").read_text())

    def test_a_job_that_concludes_on_the_reread_files_nothing(self):
        out, reads = self.run_health(jobs("success", None), jobs("success", "success"))
        self.assertIn("state=green", out)
        self.assertNotIn("DRY-RUN", out)
        self.assertEqual(reads, 2)

    def test_a_job_unconcluded_through_every_read_is_unfinished(self):
        out, reads = self.run_health(jobs("success", None))
        self.assertIn("state=red|jobs=job1 (unfinished);", out)
        self.assertEqual(reads, 3)

    def test_concluded_jobs_are_read_once(self):
        _, reads = self.run_health(jobs("success", "skipped"))
        self.assertEqual(reads, 1)


if __name__ == "__main__":
    unittest.main()
