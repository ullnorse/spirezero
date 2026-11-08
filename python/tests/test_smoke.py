"""Smoke test of the binding: `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`). The PufferEnv steps 1,000 random legal actions per
env; the observations are the engine's buffer, not a copy; errors and stale builds raise."""

import ctypes
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402

from sts2 import ffi  # noqa: E402
from sts2.env import Sts2Envs, Sts2PufferEnv  # noqa: E402
from sts2.policy import RandomLegal  # noqa: E402

ENVS = 16


class Smoke(unittest.TestCase):
    def test_puffer_env_steps_1000_random_legal_actions(self):
        env = Sts2PufferEnv(num_envs=ENVS, seed=3)
        lib = env.batch.lib
        obs, infos = env.reset()
        self.assertEqual(obs.shape, (ENVS, env.single_observation_space.shape[0]))
        self.assertEqual(obs.dtype, np.uint8)
        self.assertEqual(infos, [])
        # Zero copy: the observations are the engine's buffer.
        self.assertEqual(obs.ctypes.data, lib.sts2_envs_obs_ptr(env.batch._ptr))
        self.assertIs(env.observations, obs)
        rng = np.random.default_rng(0)
        episodes = wins = 0
        for step in range(1_000):
            legal = env.action_masks
            self.assertTrue(legal.any(axis=1).all(), f"step {step}: every env has a move")
            # A uniform legal action per env.
            scores = np.where(legal, rng.random(legal.shape), -1.0)
            actions = scores.argmax(axis=1).astype(np.int32)
            self.assertTrue(legal[np.arange(ENVS), actions].all())
            obs, rewards, terminals, truncations, infos = env.step(actions)
            self.assertIs(obs, env.observations)
            self.assertEqual(obs.ctypes.data, lib.sts2_envs_obs_ptr(env.batch._ptr))
            ended = int(terminals.sum())
            episodes += ended
            wins += int((rewards == 1.0).sum())
            self.assertEqual(int((rewards != 0).sum()), ended)
            self.assertFalse(truncations.any())
            if ended:
                self.assertEqual(infos[0]["episodes"], ended)
            # Header: HP, max HP, block as i16; HP within [0, max HP].
            header = obs[:, :6].copy().view("<i2")
            self.assertTrue(((header[:, 0] >= 0) & (header[:, 0] <= header[:, 1])).all())
            self.assertTrue((header[:, 2] >= 0).all())
        self.assertGreater(episodes, 20)
        self.assertGreater(wins, 0)
        env.close()

    def test_illegal_action_raises_and_applies_nothing(self):
        envs = Sts2Envs(2, seed=1)
        legal = envs.legal()
        illegal = int(np.flatnonzero(~legal[0])[0])
        before = envs.obs.copy()
        actions = np.array([illegal, int(np.flatnonzero(legal[1])[0])])
        with self.assertRaises(ffi.Sts2Error) as raised:
            envs.step(actions)
        self.assertEqual(raised.exception.code, ffi.STS2_ERR_ILLEGAL)
        np.testing.assert_array_equal(before, envs.obs)
        with self.assertRaises(ffi.Sts2Error) as raised:
            Sts2Envs(1, starts="/nonexistent/starts.jsonl")
        self.assertIn("/nonexistent/starts.jsonl", str(raised.exception))
        envs.close()

    def test_the_random_legal_policy_plays_legal_actions(self):
        envs = Sts2Envs(8, seed=0)
        policy = RandomLegal(8, 0)
        for _ in range(200):
            actions = policy(envs.masks)
            self.assertTrue(envs.legal()[np.arange(8), actions].all())
            envs.step(actions)
        envs.close()

    def test_a_stale_build_is_refused(self):
        lib = ffi.load()
        ffi.check_build(lib)
        self.assertEqual(lib.sts2_build_id().decode(), ffi.source_hash())
        with self.assertRaises(ffi.StaleBuild):
            ffi.check_build(lib, expected_build_id="0" * 16)
        with self.assertRaises(ffi.StaleBuild):
            Sts2Envs(1, lib=lib, vocab_hash=lib.sts2_vocab_hash() ^ 1)

    def test_layout_matches_the_buffers(self):
        lib = ffi.load()
        layout = ffi.layout(lib)
        envs = Sts2Envs(3)
        self.assertEqual(envs.obs.shape, (3, layout.obs_bytes))
        self.assertEqual(envs.masks.shape, (3, layout.mask_words))
        self.assertEqual(envs.static_obs.shape, (3, layout.static_bytes))
        self.assertFalse(envs.obs.flags.writeable)
        self.assertEqual(ctypes.sizeof(ffi.StepStats), 32)
        envs.close()


if __name__ == "__main__":
    unittest.main()
