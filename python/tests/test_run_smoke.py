"""Smoke test of the run env binding: `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`). The run PufferEnv steps 1,000 random legal actions per
env over at least 10 whole runs; the observations are the engine's buffer; the hybrid mode
leaves the fights to the env; errors raise."""

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

from sts2 import ffi  # noqa: E402
from sts2.env import RUN_ENDS, Sts2RunEnvs, Sts2RunPufferEnv  # noqa: E402

ENVS = 8


class RunSmoke(unittest.TestCase):
    def test_run_puffer_env_steps_1000_random_legal_actions(self):
        env = Sts2RunPufferEnv(num_envs=ENVS, master=5, character=None)
        lib = env.batch.lib
        layout = env.batch.layout
        obs, infos = env.reset()
        self.assertEqual(obs.shape, (ENVS, int(layout.obs_bytes)))
        self.assertEqual(obs.ctypes.data, lib.sts2_run_envs_obs_ptr(env.batch._ptr))
        self.assertEqual(env.single_action_space.n, int(layout.actions))
        rng = np.random.default_rng(0)
        runs = 0
        floors = []
        run_base = int(layout.run_base)
        run_level = combat = 0
        for step in range(1_000):
            legal = env.action_masks
            self.assertTrue(legal.any(axis=1).all(), f"step {step}: every env has a move")
            # The phase byte says whether the decision is a fight's; a fight's moves are
            # combat indices, any other decision's are run-level ones.
            phase = obs[:, 3]
            in_combat = phase == 3
            self.assertTrue((~legal[in_combat, run_base:]).all())
            self.assertTrue((~legal[~in_combat, :run_base]).all())
            run_level += int((~in_combat).sum())
            combat += int(in_combat.sum())
            scores = np.where(legal, rng.random(legal.shape), -1.0)
            actions = scores.argmax(axis=1).astype(np.int32)
            obs, rewards, terminals, truncations, infos = env.step(actions)
            self.assertIs(obs, env.observations)
            ended = int(terminals.sum())
            runs += ended
            self.assertEqual(int((rewards != 0).sum()), ended)
            if ended:
                self.assertEqual(infos[0]["runs"], ended)
                floors.extend(infos[0]["floors"])
                ends = env.batch.end[terminals]
                self.assertTrue(((ends >= 1) & (ends < len(RUN_ENDS))).all())
            # Header: HP and max HP (i16 at 8 and 10), HP within [0, max HP].
            header = obs[:, 8:12].copy().view("<i2")
            self.assertTrue(((header[:, 0] >= 0) & (header[:, 0] <= header[:, 1])).all())
        self.assertGreaterEqual(runs, 10)
        self.assertGreater(run_level, 0)
        self.assertGreater(combat, 0)
        self.assertTrue(all(f >= 1 for f in floors))
        env.close()

    def test_the_greedy_resolver_settles_the_fights(self):
        envs = Sts2RunEnvs(2, seed_range=(0, 2), combat="greedy")
        run_base = int(envs.layout.run_base)
        resolved = 0
        for _ in range(60):
            legal = envs.legal()
            if not legal.any():
                break
            self.assertFalse(legal[:, :run_base].any(), "the agent never sees a fight")
            actions = legal.argmax(axis=1)
            envs.step(actions)
            resolved += envs.stats()["resolved"]
        self.assertGreater(resolved, 0)
        envs.close()

    def test_errors_raise(self):
        with self.assertRaises(ffi.Sts2Error):
            Sts2RunEnvs(1, seed_range=(3, 3))
        with self.assertRaises(ffi.Sts2Error):
            Sts2RunEnvs(1, ascension=11)
        envs = Sts2RunEnvs(1, seeds=[4, 5])
        illegal = int(np.flatnonzero(~envs.legal()[0])[0])
        before = envs.obs.copy()
        with self.assertRaises(ffi.Sts2Error) as raised:
            envs.step([illegal])
        self.assertEqual(raised.exception.code, ffi.STS2_ERR_ILLEGAL)
        np.testing.assert_array_equal(before, envs.obs)
        envs.close()

    def test_the_final_observation_keeps_the_ended_episode(self):
        envs = Sts2RunEnvs(1, seed_range=(9_000_000, 9_000_001), character="Defect",
                           ascension=10, combat="random")
        try:
            for _ in range(10_000):
                legal = envs.legal()[0]
                if not legal.any():
                    break
                envs.step([int(np.argmax(legal))])
                if envs.done[0]:
                    self.assertTrue(envs.final_obs[0].any(), "the final observation is kept")
                    self.assertFalse(np.array_equal(envs.final_obs[0], envs.obs[0]),
                                     "the reset wrote a new observation")
                    return
            self.fail("the seed ended without a done step")
        finally:
            envs.close()


if __name__ == "__main__":
    unittest.main()
