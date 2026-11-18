"""RLN41: deck rebuild, HP recovery, id mapping and the obs adapter, on hand-made fixtures (no dump data)."""

import pathlib
import sys

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import rln41_dataset as D  # noqa: E402
import rln41_prior as P  # noqa: E402


def stats(**kw):
    base = dict(current_hp=50, max_hp=70, current_gold=100, hp_healed=0, damage_taken=0, gold_spent=0,
                player_id=1)
    base.update(kw)
    return base


def fixture_run():
    """Neow (relic), a fight (pick), a shop (remove a Strike, buy a card), a rest (Smith), a fight (skip)."""
    hist = [[
        {"map_point_type": "ancient", "rooms": [{"room_type": "event", "model_id": "EVENT.NEOW"}],
         "player_stats": [stats(hp_healed=70, current_hp=70, ancient_choice=[
             {"TextKey": "WINGED_BOOTS", "was_chosen": True}, {"TextKey": "LOST_COFFER", "was_chosen": False}],
             relic_choices=[{"choice": "RELIC.WINGED_BOOTS", "was_picked": True}])]},
        {"map_point_type": "monster", "rooms": [{"room_type": "monster"}],
         "player_stats": [stats(current_hp=40, damage_taken=30, card_choices=[
             {"card": {"id": "CARD.SNAP"}, "was_picked": False},
             {"card": {"id": "CARD.BORROWED_TIME", "floor_added_to_deck": 2}, "was_picked": True},
             {"card": {"id": "CARD.POKE"}, "was_picked": False}],
             cards_gained=[{"id": "CARD.BORROWED_TIME"}])]},
        {"map_point_type": "shop", "rooms": [{"room_type": "shop"}],
         "player_stats": [stats(current_hp=45, hp_healed=5, current_gold=25, gold_spent=150,
                                cards_removed=[{"id": "CARD.STRIKE_NECROBINDER"}],
                                cards_gained=[{"id": "CARD.DEFY"}],
                                card_choices=[{"card": {"id": "CARD.REAP"}, "was_picked": False}],
                                relic_choices=[{"choice": "RELIC.ANCHOR", "was_picked": False}])]},
        {"map_point_type": "rest_site", "rooms": [{"room_type": "rest_site"}],
         "player_stats": [stats(current_hp=45, current_gold=25, rest_site_choices=["SMITH"],
                                upgraded_cards=["CARD.BODYGUARD"])]},
        {"map_point_type": "rest_site", "rooms": [{"room_type": "rest_site"}],
         "player_stats": [stats(current_hp=66, hp_healed=21, current_gold=25, rest_site_choices=["HEAL"])]},
        {"map_point_type": "monster", "rooms": [{"room_type": "monster"}],
         "player_stats": [stats(current_hp=60, current_gold=40, card_choices=[
             {"card": {"id": "CARD.SNAP"}, "was_picked": False},
             {"card": {"id": "CARD.NOT_A_REAL_CARD"}, "was_picked": False},
             {"card": {"id": "CARD.POKE"}, "was_picked": False}])]},
    ]]
    deck = [{"id": "CARD.STRIKE_NECROBINDER"}] * 3 + [{"id": "CARD.DEFEND_NECROBINDER"}] * 4 + [
        {"id": "CARD.BODYGUARD", "current_upgrade_level": 1}, {"id": "CARD.UNLEASH"},
        {"id": "CARD.ASCENDERS_BANE"}, {"id": "CARD.BORROWED_TIME"}, {"id": "CARD.DEFY"}]
    relics = [{"id": "RELIC.BOUND_PHYLACTERY", "floor_added_to_deck": 1},
              {"id": "RELIC.WINGED_BOOTS", "floor_added_to_deck": 1}]
    return {"map_point_history": hist, "players": [{"deck": deck, "relics": relics}]}


def build(run):
    b = D.Builder(D.event_catalog([{"run": run}]))
    b.run({"id": 1, "player": 7, "win": 1, "build": "v0.111.0", "run": run})
    return b


def test_deck_rebuild_matches_final_deck():
    assert D.rebuild_check(fixture_run()) == (True, True, 0)
    broken = fixture_run()
    broken["players"][0]["deck"].append({"id": "CARD.SNAP"})
    assert D.rebuild_check(broken)[0] is False


def test_deck_before_each_decision():
    recs = build(fixture_run()).records
    cv = D.cards()
    shop = next(r for r in recs if r["kind"] == "shop_remove")
    # before the shop: start deck + Borrowed Time, 4 Strikes
    assert sum(c == cv("STRIKE_NECROBINDER") for c, _ in shop["deck"]) == 4
    assert (cv("BORROWED_TIME"), 0) in shop["deck"] and (cv("DEFY"), 0) not in shop["deck"]
    smith = next(r for r in recs if r["kind"] == "smith_target")
    assert sum(c == cv("STRIKE_NECROBINDER") for c, _ in smith["deck"]) == 3
    assert smith["options"][smith["choice"]] == (D.T_CARD, cv("BODYGUARD"), 0)
    # Ascender's Bane is neither upgradable nor removable
    assert all(o[1] != cv("ASCENDERS_BANE") for o in smith["options"] + shop["options"])


def test_hp_and_gold_before_the_choice():
    recs = build(fixture_run()).records
    rests = [r for r in recs if r["kind"] == "rest"]
    assert [r["hp"] for r in rests] == [45, 45]  # the HEAL visit: 66 after, 21 healed
    assert rests[1]["options"][rests[1]["choice"]][0] == D.T_HEAL
    shop = next(r for r in recs if r["kind"] == "shop_card")
    assert shop["hp"] == 40 and shop["gold"] == 175
    reward = next(r for r in recs if r["kind"] == "card_reward")
    assert reward["hp"] == 40  # post-fight HP


def test_id_mapping_and_drops():
    b = build(fixture_run())
    cv, rv = D.cards(), D.relics()
    assert cv("BORROWED_TIME") > 1 and cv("NOT_A_REAL_CARD") == 1 and rv("WINGED_BOOTS") > 1
    assert cv.name(cv("SNAP")) == "SNAP"
    rewards = [r for r in b.records if r["kind"] == "card_reward"]
    assert len(rewards) == 1 and rewards[0]["choice"] == 1  # the unknown-card reward is dropped
    assert b.drops[("v0.111.0", "card_reward")] == 1
    anc = next(r for r in b.records if r["kind"] == "ancient")
    assert anc["options"][anc["choice"]] == (D.T_RELIC, rv("WINGED_BOOTS"), 0)
    # the start relic is held from the first decision, Neow's relic only after Neow
    assert anc["relics"] == [rv("BOUND_PHYLACTERY")]
    assert rv("WINGED_BOOTS") in rewards[0]["relics"]


def test_shop_sequences():
    recs = build(fixture_run()).records
    cv = D.cards()
    shop = [r for r in recs if r["kind"] == "shop_card"]
    assert [r["options"][r["choice"]] for r in shop] == [(D.T_CARD, cv("DEFY"), 0), (D.T_NONE, 0, 0)]
    use = next(r for r in recs if r["kind"] == "shop_removal")
    assert use["choice"] == 0
    relic = next(r for r in recs if r["kind"] == "shop_relic")
    assert relic["options"][relic["choice"]] == (D.T_NONE, 0, 0)


def test_text_key_id_matches_rust_fold():
    # FNV-1a 64 of "" is the basis; folded and truncated to 16 bits
    h = 0xCBF29CE484222325
    f = (h ^ (h >> 16) ^ (h >> 32) ^ (h >> 48)) & 0xFFFF
    assert D.text_key_id("") == max(f, 1)
    assert D.text_key_id("A.pages.INITIAL.options.X") != D.text_key_id("A.pages.INITIAL.options.Y")


def obs_row():
    o = np.zeros(P.SHOP_AT + 96, dtype=np.uint8)
    cv, rv = D.cards(), D.relics()

    def put16(at, v):
        o[at], o[at + 1] = v & 0xFF, (v >> 8) & 0xFF

    o[2], o[7] = 1, 20
    put16(8, 40), put16(10, 70), put16(12, 123)
    for i, (c, u) in enumerate([("STRIKE_NECROBINDER", 0), ("BODYGUARD", 1)]):
        put16(P.DECK_AT + 4 * i, cv(c))
        o[P.DECK_AT + 4 * i + 2] = u
    put16(P.RELICS_AT, rv("BOUND_PHYLACTERY"))
    return o, put16


def test_obs_adapter_rest_and_reward():
    o, put16 = obs_row()
    cv = D.cards()
    o[3] = P.PHASE_REST
    o[P.REST_AT:P.REST_AT + 4] = [1, 1, 2, 1]  # Heal, Smith (kind + 1, enabled)
    (r,) = P.obs_records(o, P.REST_BASE + 1, set())
    assert r["kind"] == "rest" and r["options"][r["choice"]][0] == D.T_SMITH
    assert r["hp"] == 40 and r["max_hp"] == 70 and r["gold"] == 123 and r["act"] == 1
    assert r["deck"] == [(cv("STRIKE_NECROBINDER"), 0), (cv("BODYGUARD"), 1)]
    o[3] = P.PHASE_REWARDS
    at = P.REWARDS_AT + 2 * P.REWARD_BYTES  # reward slot 2 is a card reward
    o[at] = 5
    put16(at + 6, cv("SNAP")), put16(at + 9, cv("POKE"))
    (r,) = P.obs_records(o, P.PICK_CARD_BASE + 5 * 2 + 1, set())
    assert r["options"][r["choice"]] == (D.T_CARD, cv("POKE"), 0)
    (r,) = P.obs_records(o, P.PROCEED, set())
    assert r["options"][r["choice"]] == (D.T_SKIP, 0, 0)
    assert P.obs_records(o, P.CLAIM_BASE, set()) == []


def test_prior_scores_obs_records():
    o, _ = obs_row()
    o[3] = P.PHASE_REST
    o[P.REST_AT:P.REST_AT + 4] = [1, 1, 2, 1]
    recs = P.obs_records(o, P.REST_BASE, set())
    for expert in (False, True):
        p = P.scores(P.Prior(expert=expert).eval(), recs)
        assert len(p) == 1 and abs(float(p[0].sum()) - 1) < 1e-5


def test_nave_tier_values():
    assert P.tier_value("S") == 6 and P.tier_value("B+") > 4 and P.tier_value("S-?") < 6
    assert P.tier_value(None) is None and P.tier_value("?") is None
