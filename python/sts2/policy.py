"""A uniform random legal policy, vectorised over the batch.

Env `i`'s draws come from SplitMix64 seeded with `seed + i`: each step it takes the next value
`z` and plays the `(z mod count)`-th set bit of its mask, lowest index first. The Rust driver
`crates/sts2-ffi/examples/ffi_bench.rs` does the same, so both play the same trajectories.
"""

import numpy as np

_GAMMA = np.uint64(0x9E3779B97F4A7C15)
_M1 = np.uint64(0xBF58476D1CE4E5B9)
_M2 = np.uint64(0x94D049BB133111EB)


class RandomLegal:
    def __init__(self, n, seed=0):
        self.state = np.arange(n, dtype=np.uint64) + np.uint64(seed)

    def _next(self):
        self.state += _GAMMA
        z = self.state.copy()
        z ^= z >> np.uint64(30)
        z *= _M1
        z ^= z >> np.uint64(27)
        z *= _M2
        z ^= z >> np.uint64(31)
        return z

    def __call__(self, masks):
        """One action index per env from `masks` (n × mask_words uint64); an env with an empty
        mask gets 0."""
        bits = np.unpackbits(masks.view(np.uint8), axis=1, bitorder="little")
        count = bits.sum(axis=1, dtype=np.uint64)
        nth = self._next() % np.maximum(count, np.uint64(1))
        ranks = bits.cumsum(axis=1, dtype=np.uint16)
        return np.argmax(ranks > nth.astype(np.uint16)[:, None], axis=1)
