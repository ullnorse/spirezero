"""RLN46: Osty absorbs the expected damage; Doom kills count.

    python -m unittest python/tests/test_e2e_rln46.py
"""
import unittest
from pathlib import Path

import torch

from sts2.e2e import encode as E

VOCAB = Path(__file__).resolve().parents[2] / "crates/sts2-rl/vocab/powers.txt"


class T_test_power_ids_match_vocab(unittest.TestCase):
    def test(self):
        names = VOCAB.read_text().split()
        assert names[E.POWER_DOOM] == "DOOM_POWER"
        assert names[E.POWER_DIE_FOR_YOU] == "DIE_FOR_YOU_POWER"


def _creatures():
    # Slot 0 player, slot 1 Osty (DieForYou), slot 2 an enemy with Doom 10.
    pkind = torch.zeros(1, 8, 12, dtype=torch.int32)
    pamt = torch.zeros(1, 8, 12, dtype=torch.int32)
    pkind[0, 1, 0] = E.POWER_DIE_FOR_YOU
    pkind[0, 2, 3], pamt[0, 2, 3] = E.POWER_DOOM, 10
    alive = torch.zeros(1, 8, dtype=torch.bool)
    alive[0, :3] = True
    side = torch.tensor([[0, 0, 1, 0, 0, 0, 0, 0]])
    hp = torch.tensor([[50, 6, 18, 0, 0, 0, 0, 0]])
    block = torch.tensor([[5, 2, 3, 0, 0, 0, 0, 0]])
    return pkind, pamt, alive, side, hp, block


class T_test_osty_absorb_is_his_hp_plus_block_while_alive(unittest.TestCase):
    def test(self):
        pkind, _, alive, side, hp, block = _creatures()
        assert E._osty_absorb(pkind, alive, side, hp, block).tolist() == [8]
        alive[0, 1] = False
        assert E._osty_absorb(pkind, alive, side, hp, block).tolist() == [0]


class T_test_doomed_after_best_damage(unittest.TestCase):
    def test(self):
        pkind, pamt, alive, side, hp, block = _creatures()
        most = torch.zeros(1, 8, dtype=torch.int32)
        most[0, 2] = 10  # 10 - 3 block = 7 damage: 18 -> 11 > Doom 10
        assert not E._doomed(pkind, pamt, alive, side, hp, block, most)[0, 2]
        most[0, 2] = 11  # 18 -> 10 <= Doom 10
        assert E._doomed(pkind, pamt, alive, side, hp, block, most)[0, 2]
        assert not E._doomed(pkind, pamt, alive, side, hp, block, most)[0, :2].any()
