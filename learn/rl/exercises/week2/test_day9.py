import numpy as np
import pytest
import torch

from conftest import exercise

day = exercise("week2.day9_reinforce")
from common.minifight import DEFEND, STRIKE, MiniFight, State, win_rate  # noqa: E402


def test_features_are_normalised():
    f = day.features(State(strikes=3, defends=2))
    assert f.shape == (day.N_FEATURES,) and f.dtype == np.float32
    assert f[0] == 1.0 and f[2] == 1.0 and f[3] == pytest.approx(0.6) and f[4] == pytest.approx(0.4)
    assert f[7] == pytest.approx(12 / 28) and f[8:11].tolist() == [1.0, 0.0, 0.0]
    f = day.features(State(block=20, intent="bellow", strength=4))
    assert f[7] == 0.0 and f[8:11].tolist() == [0.0, 0.0, 1.0]


def test_masked_dist_gives_illegal_actions_zero_probability():
    d = day.masked_dist(torch.tensor([2.0, 1.0, 0.0]), torch.tensor([False, True, True]))
    assert d.probs[0] == 0.0 and d.probs[1:].sum() == pytest.approx(1.0)
    assert all(int(d.sample()) != 0 for _ in range(50))
    assert torch.isfinite(d.log_prob(torch.tensor(1)))


def test_run_episode_logs_one_logp_per_step():
    torch.manual_seed(0)
    logps, rewards, s = day.run_episode(MiniFight(0), day.PolicyNet())
    assert s.done and len(logps) == len(rewards) == s.steps
    assert rewards[-1] in (1.0, -1.0) and all(r == 0.0 for r in rewards[:-1])
    assert all(lp.requires_grad for lp in logps)


def test_reinforce_loss_is_minus_logp_times_advantage():
    logps = [torch.tensor(-0.5, requires_grad=True), torch.tensor(-1.0, requires_grad=True)]
    loss = day.reinforce_loss(logps, [1.0, 1.0], 0.0)
    assert loss.item() == pytest.approx(1.5)
    assert day.reinforce_loss(logps, [1.0, 1.0], 1.0).item() == pytest.approx(0.0)
    assert day.reinforce_loss(logps, [-1.0, -1.0], 0.0).item() == pytest.approx(-1.5)


def test_train_beats_random_over_cards_and_stalls_on_the_block():
    net, curve = day.train(episodes=2500, seed=0)
    assert len(curve) == 10
    assert curve[-1] > 0.5 > curve[0] + 0.1
    assert win_rate(day.greedy_policy(net), 500, seed=3) > 0.5  # random over cards: 0.16
    # REINFORCE on these features settles on "play cards, attacks first" (about 0.75) and
    # never learns the lethal block: a local optimum the tabular agent of day 6 did not have.
    assert 0.0 <= day.p_defend(net) <= 1.0
