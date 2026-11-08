import numpy as np
import pytest

from conftest import exercise

day = exercise("week1.day2_observe")


@pytest.fixture(scope="module")
def envs():
    e = day.make_envs(16, seed=1)
    yield e
    e.close()


def test_header_reads_the_first_turn(envs):
    for i in range(envs.n):
        h = day.header(envs.obs[i])
        assert h["hp"] == int(envs.obs[i, 0:2].copy().view("<i2")[0])
        assert 0 < h["hp"] <= h["max_hp"]
        assert h["block"] >= 0
        assert h["turn"] == 1
        assert h["status"] == 0
        assert h["hand"] == int(envs.obs[i, 16])
        assert h["energy"] == int(envs.obs[i, 6:8].copy().view("<i2")[0])


def test_hand_names_are_real_cards(envs):
    cards = day.vocab("cards")
    for i in range(envs.n):
        names = day.hand_names(envs.obs[i], cards)
        assert len(names) == day.header(envs.obs[i])["hand"]
        assert names, "a first hand is never empty"
        for n in names:
            assert n in cards and n not in ("<pad>", "<unk>")
    assert any(n.startswith("STRIKE") for i in range(envs.n) for n in day.hand_names(envs.obs[i], cards))


def test_intents_show_an_attack_somewhere(envs):
    attacks = 0
    for i in range(envs.n):
        for kind, damage, hits in day.intents(envs.obs[i]):
            assert 0 <= kind < 16
            if kind == 0:
                assert damage > 0 and hits >= 1
                attacks += 1
    assert attacks > 0


def test_decode_action_covers_the_140_indices():
    assert day.decode_action(0) == ("play", 0, 0)
    assert day.decode_action(9) == ("play", 1, 1)
    assert day.decode_action(79) == ("play", 9, 7)
    assert day.decode_action(80) == ("potion", 0, 0)
    assert day.decode_action(127) == ("potion", 5, 7)
    assert day.decode_action(128) == ("end",)
    assert day.decode_action(129) == ("pick", 0)
    assert day.decode_action(138) == ("pick", 9)
    assert day.decode_action(139) == ("done",)
    assert all(isinstance(x, int) for x in day.decode_action(np.int64(9))[1:])


def test_cards_first_prefers_a_card_then_the_lowest_other_index():
    legal = np.zeros((3, 140), dtype=bool)
    legal[0, [3, 70, 128]] = True
    legal[1, [128]] = True
    legal[2, [130, 139]] = True
    assert day.cards_first(legal).tolist() == [3, 128, 130]


def test_cards_first_never_ends_a_turn_with_a_playable_card():
    envs = day.make_envs(16, seed=4)
    for _ in range(200):
        legal = envs.legal()
        actions = day.cards_first(legal)
        assert legal[np.arange(16), actions].all()
        has_card = legal[:, :128].any(axis=1)
        assert (actions[has_card] < 128).all()
        envs.step(actions)
    envs.close()


def test_cards_first_beats_random():
    wr, n = day.win_rate(day.cards_first, n_envs=64, n_steps=2000, seed=0)
    assert n > 1000
    assert wr >= 0.58
