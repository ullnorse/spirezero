"""RLN45: the compile_net crash under the combat gym.

* `_guarded`: a call whose compiled graph fails to build (inductor's `CantSplit`) runs eager,
  the same result, and counts it; a call that compiles stays compiled;
* `compile` without `compile_net` compiles the blocks again (a local `import torch._dynamo`
  made `torch` unbound in `_try_compile`, so every `compile` run went eager);
* `compile_net` turns duck shaping off (equal sizes no longer share a symbol) and trains to
  float noise of eager with no eager fallbacks.
"""

import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import torch  # noqa: E402
import torch._dynamo as dynamo  # noqa: E402
import torch.fx.experimental._config as fx_config  # noqa: E402

import train_e2e as T  # noqa: E402
from test_e2e_learner import tiny_config  # noqa: E402
from test_rln44 import FULL, weights  # noqa: E402


class Guarded(unittest.TestCase):
    def test_a_failed_graph_runs_eager(self):
        calls = []

        def compiled(x):
            calls.append("compiled")
            if x.shape[0] == 3:
                raise dynamo.exc.BackendCompilerFailed(compiled, RuntimeError("CantSplit"), None)
            return x * 2

        f = T._guarded(compiled, lambda x: x * 2, "forward")
        self.assertTrue(torch.equal(f(torch.ones(2)), torch.full((2,), 2.0)))
        self.assertTrue(torch.equal(f(torch.ones(3)), torch.full((3,), 2.0)))
        self.assertTrue(torch.equal(f(torch.ones(3)), torch.full((3,), 2.0)))
        self.assertEqual(f.eager_calls[0], 2)
        self.assertEqual(calls, ["compiled"] * 3)


class Compiled(unittest.TestCase):
    def setUp(self):
        self.dir = pathlib.Path(tempfile.mkdtemp())
        self.duck = fx_config.use_duck_shape
        dynamo.reset()

    def tearDown(self):
        shutil.rmtree(self.dir)
        fx_config.use_duck_shape = self.duck
        dynamo.reset()

    def train(self, name, iters=2, **kw):
        a = T.Learner(tiny_config(**FULL, kl_source="batch", epochs=1, static_head=True, **kw),
                      self.dir / name)
        recs = [a.iterate() for _ in range(iters)]
        return a, weights(a), recs

    def test_compile_without_compile_net_compiles(self):
        a, _, _ = self.train("a", compile=True)
        self.assertTrue(a.compiled)
        a.close()

    def test_compile_net_trains_to_float_noise(self):
        a, wa, _ = self.train("a")
        a.close()
        b, wb, _ = self.train("b", compile=True, compile_net=True)
        self.assertTrue(b.compiled)
        self.assertFalse(fx_config.use_duck_shape)
        self.assertEqual(b.net.forward.eager_calls[0], 0)
        b.close()
        for p, q in zip(wa, wb):
            torch.testing.assert_close(p, q, rtol=1e-3, atol=1e-4)


if __name__ == "__main__":
    unittest.main()
