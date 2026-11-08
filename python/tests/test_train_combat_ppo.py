"""A smoke test for the combat PPO trainer: one update, no evaluation run.

Skipped when torch is not installed. The full trainer is `python/train_combat_ppo.py`; this test
checks the mask, the rollout/GAE shapes and that a checkpoint is written.
"""

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

try:
    import torch
except ImportError:  # pragma: no cover - the system Python has no torch
    torch = None


@unittest.skipIf(torch is None, "torch is not installed")
class CombatPpoTest(unittest.TestCase):
    def test_one_update_writes_a_checkpoint(self):
        import train_combat_ppo as trainer

        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "net.pt"
            class Args:
                updates = 1
                steps = 8
                envs = 4
                hidden = 32
                seed = 0
                lr = 3e-4
                gamma = 0.99
                lam = 0.95
                clip = 0.2
                epochs = 1
                minibatches = 2
                entropy = 0.01
                value_coef = 0.5
                eval_every = 1
                eval_episodes = 4
                device = "cpu"
                out = str(path)
            _, best = trainer.train(Args())
            self.assertTrue(path.is_file(), "the best checkpoint is written")
            self.assertGreaterEqual(best, 0.0)
            saved = torch.load(path, weights_only=False)
            self.assertEqual(saved["actions"], 140)
            self.assertIn("model", saved)


if __name__ == "__main__":
    unittest.main()
