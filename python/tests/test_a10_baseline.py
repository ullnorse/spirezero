"""The A10 baseline harness on a handful of seeds: `python -m unittest discover -s python/tests`
(after `cargo build --profile ffi -p sts2-ffi`). It plays three random-agent runs and two
greedy-resolver runs, checks the aggregates against the env's own counters, and writes the
artefacts to a temporary directory.
"""

import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import a10_baseline as baseline  # noqa: E402

from sts2.env import RUN_ENDS, Sts2RunEnvs  # noqa: E402


class A10BaselineTest(unittest.TestCase):
    def test_the_seeds_are_ascension_10_and_use_the_run_space(self):
        envs = Sts2RunEnvs(3, seeds=[9_000_000, 9_000_001, 9_000_002], character="Silent",
                           ascension=10)
        try:
            self.assertTrue((envs.obs[:, 1] == 10).all())
            self.assertGreater(int(envs.layout.actions), int(envs.layout.run_base))
        finally:
            envs.close()

    def test_random_agent_aggregates(self):
        seeds = [9_000_000, 9_000_001, 9_000_002]
        result = baseline.evaluate("Necrobinder", "random", seeds, envs_n=3)
        self.assertEqual(result.episodes, len(seeds))
        self.assertEqual(len(result.floors), len(seeds))
        self.assertEqual(sum(result.ends), len(seeds))
        self.assertEqual(result.ends[RUN_ENDS.index("died")] + result.ends[RUN_ENDS.index("stuck")]
                         + result.ends[RUN_ENDS.index("capped")]
                         + result.ends[RUN_ENDS.index("unsupported")],
                         len(seeds) - result.wins)
        self.assertTrue(all(floor >= 1 for floor in result.floors))
        self.assertEqual(result.mask_rejections, 0)
        self.assertGreater(result.decisions, 0)
        self.assertGreater(result.steps_per_s, 0)
        self.assertGreater(result.resets_per_s, 0)
        self.assertAlmostEqual(result.decisions_per_episode,
                               result.decisions / result.episodes)

    def test_greedy_resolver_plays_the_fights(self):
        result = baseline.evaluate("Defect", "greedy", [9_000_010, 9_000_011], envs_n=2)
        self.assertEqual(result.episodes, 2)
        self.assertGreater(result.resolved, 0)
        self.assertEqual(result.mask_rejections, 0)

    def test_totals_and_artefacts(self):
        rows = [
            baseline.evaluate("Ironclad", "random", [9_000_020, 9_000_021], envs_n=2),
            baseline.evaluate("Silent", "random", [9_000_020, 9_000_021], envs_n=2),
        ]
        total = baseline.total(rows, "random")
        self.assertEqual(total.episodes, 4)
        self.assertEqual(total.wins, sum(r.wins for r in rows))
        self.assertEqual(len(total.floors), 4)
        self.assertAlmostEqual(total.seconds, sum(r.seconds for r in rows))
        with tempfile.TemporaryDirectory() as tmp:
            json_path, csv_path = baseline.write_results(
                rows + [total], tmp, {"ascension": 10, "seeds_per_character": 2}
            )
            payload = json.loads(pathlib.Path(json_path).read_text())
            self.assertEqual(payload["ascension"], 10)
            self.assertEqual(len(payload["rows"]), 3)
            self.assertEqual(payload["rows"][-1]["character"], "TOTAL")
            self.assertTrue(pathlib.Path(csv_path).read_text().startswith("character,mode,"))


if __name__ == "__main__":
    unittest.main()
