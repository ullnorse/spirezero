"""RLN39: the fight benchmark's panel selection, paired comparison and summary (no engine needed)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import rln39_fightbench as FB  # noqa: E402
import rln39_trace as T  # noqa: E402


def fight(seed, boss, floor=33):
    return {"seed": seed, "boss": boss, "act": 2, "floor": floor, "prefix": "AA==", "sha_v1": "x",
            "entry": {"hp": 50, "max_hp": 80}}


def test_select_limits_and_dedup():
    fights = [fight(s, "KAISER_CRAB_BOSS") for s in range(5)] + [fight(1, "KAISER_CRAB_BOSS")]
    fights += [fight(s, "QUEEN_BOSS") for s in range(10, 13)] + [fight(9, "VANTOM_BOSS")]
    panel = FB.select(fights, {"KAISER_CRAB_BOSS": 3, "QUEEN_BOSS": 10})
    assert [f["id"] for f in panel] == ["0-33", "1-33", "2-33", "10-33", "11-33", "12-33"]
    assert sum(f["boss"] == "KAISER_CRAB_BOSS" for f in panel) == 3
    assert not any(f["boss"] == "VANTOM_BOSS" for f in panel)


def run(wins, boss="KAISER_CRAB_BOSS"):
    return {"fights": [{"id": f"f{i}", "boss": boss, "win": w} for i, w in enumerate(wins)]}


def test_paired_counts_and_sign():
    a, b = run([1, 1, 1, 0, 0, 1]), run([1, 0, 0, 0, 0, 1])
    row = FB.paired(a, b)[0]
    assert (row["both"], row["only_a"], row["only_b"]) == (2, 2, 0)
    assert row["a"] == 4 and row["b"] == 2 and abs(row["diff"] - 2 / 6) < 1e-9
    assert row["lo"] < row["diff"] < row["hi"]
    assert FB.paired(a, b)[-1]["boss"] == "ALL"


def test_summarize_wilson():
    panel = [{"id": f"f{i}", "hp": 40, "max_hp": 80} for i in range(10)]
    res = [{"id": f"f{i}", "boss": "QUEEN_BOSS", "win": int(i < 3)} for i in range(10)]
    row = FB.summarize(panel, res)[0]
    assert row["wins"] == 3 and row["n"] == 10 and row["lo"] < 0.3 < row["hi"]
    assert abs(row["entry_hp"] - 0.5) < 1e-9


def test_insatiable_sandpit_death():
    rows = [{"t": 3, "hp": 40, "blk": 0, "inc": 10, "sand": 1, "what": "end"}]
    s = T.insat_stats({"end": "died", "rows": rows})
    assert s["sandpit_death"] and not s["won"]
    rows[0]["inc"] = 60
    assert not T.insat_stats({"end": "died", "rows": rows})["sandpit_death"]
