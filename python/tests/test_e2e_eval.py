"""`python/e2e_eval.py` (WP6): `python -m unittest discover -s python/tests` (after
`cargo build --profile ffi -p sts2-ffi`).

The statistics against hand-computed values; a result file that is byte-identical on a repeat run
and after a stop and resume (tiny CPU net); the replay confirms an action list ends where the
file says and flags a fake win; the sealed TEST registry refuses a second read; the gates; the
bot arm's row parser.
"""

import json
import math
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))

import numpy as np  # noqa: E402
import torch  # noqa: E402

import e2e_eval as V  # noqa: E402
import train_e2e as T  # noqa: E402
from sts2 import E2E, HOLD, Sts2RunEnvs  # noqa: E402
from sts2.e2e.net import E2ENet  # noqa: E402


class Stats(unittest.TestCase):
    def test_wilson(self):
        lo, hi = V.wilson(10, 20)  # the textbook interval of 10/20
        self.assertAlmostEqual(lo, 0.2993, places=4)
        self.assertAlmostEqual(hi, 0.7007, places=4)
        lo, hi = V.wilson(0, 100)
        self.assertAlmostEqual(lo, 0.0, places=12)
        self.assertAlmostEqual(hi, 0.0370, places=4)

    def test_mcnemar_exact(self):
        self.assertEqual(V.mcnemar_exact(0, 0), 1.0)
        self.assertAlmostEqual(V.mcnemar_exact(0, 5), 0.0625)
        self.assertAlmostEqual(V.mcnemar_exact(3, 10), 2 * 378 / 8192)
        self.assertEqual(V.mcnemar_exact(5, 5), 1.0)
        self.assertEqual(V.mcnemar_exact(7, 2), V.mcnemar_exact(2, 7))

    def test_newcombe_paired(self):
        # No correlation (an empty margin): the square-and-add of the two Wilson intervals.
        a, b, c, d = 0, 10, 0, 90
        diff, lo, hi = V.newcombe_paired(a, b, c, d)
        l1, u1 = V.wilson(10, 100)
        l2, u2 = V.wilson(0, 100)
        self.assertAlmostEqual(diff, 0.10)
        self.assertAlmostEqual(lo, 0.10 - math.hypot(0.10 - l1, u2 - 0.0), places=12)
        self.assertAlmostEqual(hi, 0.10 + math.hypot(u1 - 0.10, 0.0 - l2), places=12)
        # Concordant pairs tighten the interval; the sign flips with the arguments.
        diff, lo, hi = V.newcombe_paired(40, 12, 3, 45)
        self.assertAlmostEqual(diff, 0.09)
        self.assertTrue(0.0 < lo < diff < hi < 0.2)
        self.assertLess(hi - lo, 0.2)
        d2, lo2, hi2 = V.newcombe_paired(40, 3, 12, 45)
        for got, want in zip((d2, lo2, hi2), (-diff, -hi, -lo)):
            self.assertAlmostEqual(got, want, places=12)
        # No discordant pairs: a zero difference whose CI contains zero.
        diff, lo, hi = V.newcombe_paired(20, 0, 0, 80)
        self.assertEqual(diff, 0.0)
        self.assertTrue(lo < 0.0 < hi)

    def test_paired_mean(self):
        mean, lo, hi = V.paired_mean([2, 4, 6, 8], [1, 2, 3, 4])  # d = 1, 2, 3, 4
        se = math.sqrt(5 / 3) / 2  # sd 1.29099 / sqrt(4)
        self.assertAlmostEqual(mean, 2.5)
        self.assertAlmostEqual(lo, 2.5 - V.Z * se)
        self.assertAlmostEqual(hi, 2.5 + V.Z * se)


def tiny_checkpoint(path):
    cfg = T.Config.from_dict({**T.dataclasses.asdict(T.Config()), "d": 32, "layers": 1,
                              "device": "cpu"})
    torch.manual_seed(0)
    net = E2ENet(d=32, layers=1)
    emas = [[p.detach().clone() * (1 + 0.01 * i) for p in net.parameters()]
            for i in range(len(cfg.ema))]
    torch.save(dict(config=T.dataclasses.asdict(cfg), net=net.state_dict(), emas=emas), path)


def run_cli(*argv):
    return V.main([str(a) for a in argv])


class Runner(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.dir = pathlib.Path(cls.tmp.name)
        cls.ckpt = cls.dir / "state.pt"
        tiny_checkpoint(cls.ckpt)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def play(self, out, *extra, weights="ema0.999"):
        return run_cli("run", self.ckpt, "--weights", weights, "--panel", "SMOKE", "--n", 8,
                       "--batch", 4, "--threads", 2, "--device", "cpu", "--out", out,
                       "--registry", self.dir / "reg.jsonl", *extra)

    def test_repeat_and_resume_are_byte_identical(self):
        a, b, c = (self.dir / f"{x}.jsonl" for x in "abc")
        self.play(a)
        self.play(b)
        self.assertEqual(a.read_bytes(), b.read_bytes())
        with self.assertRaises(SystemExit):  # stops after one of two batches
            self.play(c, "--stop-after", 1)
        meta, runs = V.load_results(c)
        self.assertEqual(len(runs), 4)
        with open(c, "ab") as f:  # a torn line from a kill mid-write
            f.write(b'{"seed":23000')
        self.play(c)
        self.assertEqual(a.read_bytes(), c.read_bytes())
        meta, runs = V.load_results(a)
        self.assertEqual(sorted(runs), list(range(23_000_000, 23_000_008)))
        self.assertEqual(meta["weights"], "ema0.999")
        self.assertEqual(len(meta["ckpt_sha256"]), 64)
        self.assertTrue(meta["env"]["dedupe"] and not meta["env"]["priv"])
        for r in runs.values():
            self.assertIn(r["end"], ("won", "died", "capped", "unsupported", "stuck"))
            self.assertEqual(r["nan"], 0)

    def test_death_and_fights(self):
        out = self.dir / "d.jsonl"
        self.play(out)
        meta, runs = V.load_results(out)
        names = set(V.encounter_names())
        died = 0
        for r in runs.values():
            self.assertTrue(r["fights"], "every run enters a fight")
            self.assertEqual(len(r["fights"]), len(r["fight_acts"]))
            self.assertTrue(set(r["fights"]) <= names)
            if r["end"] == "died":
                died += 1
                if r["death"] is not None:
                    self.assertEqual(r["death"], r["fights"][-1])
            else:
                self.assertIsNone(r["death"])
        self.assertTrue(died, "a random net dies on some SMOKE seed")
        self.assertTrue(any(r["death"] for r in runs.values()))
        rows = V.killers(runs)
        self.assertEqual(sum(d for _, d, _, _ in rows), sum(1 for r in runs.values() if r["death"]))
        for _, d, reached, _ in rows:
            self.assertLessEqual(d, reached)

    def test_weights_change_the_meta(self):
        a = self.dir / "w.jsonl"
        self.play(a)
        with self.assertRaises(SystemExit):  # same file, other weights
            self.play(a, weights="raw")

    def test_replay_checks_the_end(self):
        opts = V.env_options()
        env = Sts2RunEnvs(2, seeds=np.array([23_000_100, 23_000_101], dtype=np.uint64),
                          character="Necrobinder", ascension=10, combat="agent", threads=1,
                          **E2E)
        rng = np.random.default_rng(1)
        log = [[], []]
        end = [None, None]
        while end[0] is None or end[1] is None:
            legal = env.legal()
            a = np.where(legal, rng.random(legal.shape), -1).argmax(1).astype(np.uint16)
            for i in range(2):
                if end[i] is not None:
                    a[i] = HOLD
                else:
                    log[i].append(int(a[i]))
            env.step(a)
            for i in range(2):
                if end[i] is None and env.done[i]:
                    end[i] = int(env.won[i])
        env.close()
        runs = [{"seed": 23_000_100 + i, "actions": V.pack(log[i])} for i in range(2)]
        runs[1]["actions"] = V.pack(log[1][:-1])  # one action short: no end on the last action
        got = V.replay_runs(runs, opts)
        self.assertEqual(got[23_000_100], (True, bool(end[0])))
        self.assertEqual(got[23_000_101], (False, False))

    def test_replay_flags_a_fake_win(self):
        path = self.dir / "fake.jsonl"
        self.play(path)
        meta, runs = V.load_results(path)
        seed = min(runs)
        run = dict(runs[seed], win=1, end="won", actions=V.pack([0, 0, 0]))
        run["hash"] = V.action_hash([0, 0, 0])
        lines = [json.dumps({"meta": meta}), json.dumps(run)]
        path.write_text("\n".join(lines) + "\n")
        wins, replayed, bad = V.replay_file(path)
        self.assertEqual((wins, replayed, bad), (1, 0, [seed]))

    def test_test_panel_is_read_once(self):
        reg = self.dir / "treg.jsonl"
        sha = V.file_sha256(self.ckpt)
        reg.write_text(json.dumps({"ckpt_sha256": sha, "weights": "raw"}) + "\n")
        argv = ("run", self.ckpt, "--weights", "raw", "--panel", "TEST", "--device", "cpu",
                "--out", self.dir / "t.jsonl", "--registry", reg)
        with self.assertRaises(SystemExit) as cm:
            run_cli(*argv)
        self.assertIn("already read", str(cm.exception))
        self.assertFalse((self.dir / "t.jsonl").exists())

    def test_sealed_files_need_unseal(self):
        meta = V.make_meta("net", "TEST", *V.PANELS["TEST"], ckpt_sha256="x", weights="raw")
        with self.assertRaises(SystemExit):
            V.sealed(meta, False)
        V.sealed(meta, True)
        V.sealed(dict(meta, panel="DEV-1000"), False)


def synth(path, panel, wins, floors, a2=(), weights="raw"):
    """A fake result file of the panel's seeds: run `i` wins if `wins[i]` and has `floors[i]`."""
    first, count = V.PANELS[panel]
    meta = {"kind": "net", "panel": panel, "first": first, "count": count, "weights": weights}
    lines = [json.dumps({"meta": meta})]
    for i in range(count):
        floor = floors[i % len(floors)]
        lines.append(json.dumps({
            "seed": first + i, "win": int(wins(i)), "floor": floor + (40 if i in a2 else 0),
            "end": "won" if wins(i) else "died", "bosses": 0, "boss_acts": 0, "kf": [],
            "nan": 0, "hash": "0"}))
    pathlib.Path(path).write_text("\n".join(lines) + "\n")


class Gates(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.dir = pathlib.Path(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def gate(self, *argv):
        import contextlib
        import io

        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            code = run_cli("gate", *argv)
        self.assertEqual(buf.getvalue().strip().splitlines()[-1], "PASS" if code == 0 else "FAIL")
        return code

    def test_k1(self):
        good, bad = self.dir / "g.jsonl", self.dir / "b.jsonl"
        synth(good, "DEV-1000", lambda i: False, [14, 15])
        synth(bad, "DEV-1000", lambda i: False, [13, 14])
        self.assertEqual(self.gate("K1", good), 0)
        self.assertEqual(self.gate("K1", bad), 1)

    def test_k2_k3_k4(self):
        k1, k2 = self.dir / "k1.jsonl", self.dir / "k2.jsonl"
        synth(k1, "DEV-1000", lambda i: False, [10])
        synth(k2, "DEV-1000", lambda i: False, [10], a2=range(0, 1000, 2))  # +20 floors on half
        self.assertEqual(self.gate("K2", k1, k2), 0)
        synth(k2, "DEV-1000", lambda i: False, [10], a2=range(0, 1000, 10))  # reach 10%
        self.assertEqual(self.gate("K2", k1, k2), 1)
        old, new, flat = (self.dir / f"{x}.jsonl" for x in ("o", "n", "f"))
        synth(old, "DEV-2000", lambda i: i % 50 == 0, [10])  # 2%
        synth(new, "DEV-2000", lambda i: i % 50 == 0 or i % 20 == 3, [10])  # 2% + 4.8%: superset
        synth(flat, "DEV-2000", lambda i: i % 50 == 0, [10])
        self.assertEqual(self.gate("K3", old, new), 0)
        self.assertEqual(self.gate("K3", old, flat), 1)  # WR 2% < 3%
        self.assertEqual(self.gate("K4", old, new), 0)  # +4.8pp, p tiny
        self.assertEqual(self.gate("K4", old, flat), 1)

    def test_compare_uses_shared_seeds(self):
        a, b = self.dir / "a.jsonl", self.dir / "b.jsonl"
        synth(a, "DEV-1000", lambda i: i < 30, [10])
        synth(b, "DEV-2000", lambda i: i < 10, [10])
        p = V.paired(V.load_results(a)[1], V.load_results(b)[1])
        self.assertEqual((p["n"], p["both"], p["only_a"], p["only_b"]), (1000, 10, 20, 0))
        self.assertAlmostEqual(p["p"], 2 / 2**20)


class BotRows(unittest.TestCase):
    def test_parse(self):
        rows = self.id() and (
            "RUN24000001\tdeath\tact 1\tfloor 12\thp 0/66\tfight hp 22\tdeck 15\trelics 5\t"
            "PHROG\tactions 141\tms 290\n"
            "RUN24000000\tvictory\tact 3\tfloor 50\thp 5/66\tfight hp 0\tdeck 25\trelics 9\t"
            "X\tactions 900\tms 9000\n")
        with tempfile.TemporaryDirectory() as d:
            path = pathlib.Path(d) / "rows.tsv"
            path.write_text(rows)
            runs = V.parse_bot_rows(path)
        self.assertEqual([r["seed"] for r in runs], [24000000, 24000001])
        self.assertEqual((runs[0]["win"], runs[0]["end"], runs[0]["floor"]), (1, "won", 50))
        self.assertEqual((runs[1]["win"], runs[1]["end"], runs[1]["bosses"]), (0, "died", 0))


if __name__ == "__main__":
    unittest.main()
