"""RLN1 throughput bench for an end-to-end Necrobinder PPO plan (`docs/plans/necro-e2e.md`).

Three short measurements, run one after another (never alongside another bench):

1. `env`: `Sts2RunEnvs` (Necrobinder, A10, the agent plays the fights) stepped by a vectorised
   random legal policy: agent decisions per second for a few (envs, threads) shapes.
2. `net`: a stand-in entity transformer (d=256, 4 or 6 layers, 96 tokens built from the
   observation bytes, one logit per action index) on the GPU: inference decisions/s at batch
   1,024 and PPO update decisions/s (forward + backward + AdamW) at minibatch 8,192, bf16.
3. `loop`: env step -> host-to-device copy -> forward -> masked sample -> actions back, the
   collection loop of a single-process learner.

The net is a cost proxy, not the planned encoder: its token count and width match the plan.

    cargo build --profile ffi -p sts2-ffi
    ~/spirebot/.venv/bin/python python/rln1_bench.py [env|net|loop|all]
"""

import pathlib
import sys
import time

import numpy as np

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sts2 import Sts2RunEnvs  # noqa: E402

SEEDS = (23_100_000, 23_200_000)  # the SMOKE-adjacent bench range of the plan, unused elsewhere
TOKENS, TOKEN_BYTES = 96, 30


def random_legal(masks, n_actions, rng):
    bits = np.unpackbits(masks.view(np.uint8), axis=1, bitorder="little")[:, :n_actions]
    noise = rng.random(bits.shape, dtype=np.float32) * bits
    return noise.argmax(axis=1)


def bench_env(shapes=((64, 1), (256, 1), (1024, 8), (2048, 14)), secs=8.0):
    rng = np.random.default_rng(0)
    for n, threads in shapes:
        envs = Sts2RunEnvs(n, seed_range=SEEDS, cycle=True, character="Necrobinder",
                           ascension=10, combat="agent", threads=threads)
        steps = runs = 0
        t0 = time.perf_counter()
        while time.perf_counter() - t0 < secs:
            runs += envs.step(random_legal(envs.masks, envs.actions, rng))
            steps += n
        dt = time.perf_counter() - t0
        print(f"env  envs={n:5d} threads={threads:2d}  {steps / dt:12,.0f} decisions/s  "
              f"{runs / dt:8,.0f} runs/s  obs {envs.obs_bytes} B")
        envs.close()


def make_net(layers, n_actions, device):
    import torch
    import torch.nn as nn

    class Net(nn.Module):
        def __init__(self):
            super().__init__()
            self.stem = nn.Linear(TOKEN_BYTES, 256)
            self.pos = nn.Parameter(torch.zeros(TOKENS, 256))
            block = nn.TransformerEncoderLayer(256, 4, 704, 0.0, activation="gelu",
                                               batch_first=True, norm_first=True)
            self.trunk = nn.TransformerEncoder(block, layers, enable_nested_tensor=False)
            self.pi = nn.Linear(256, n_actions)
            self.v = nn.Linear(256, 1)

        def forward(self, obs):
            x = obs[:, : TOKENS * TOKEN_BYTES].view(-1, TOKENS, TOKEN_BYTES).float() / 255.0
            h = self.trunk(self.stem(x) + self.pos)
            g = h.mean(dim=1)
            return self.pi(g), self.v(g).squeeze(-1)

    net = Net().to(device)
    return net, sum(p.numel() for p in net.parameters())


def bench_net(n_actions=355, obs_bytes=2996):
    import torch

    dev = torch.device("cuda")
    for layers in (4, 6):
        net, params = make_net(layers, n_actions, dev)
        opt = torch.optim.AdamW(net.parameters(), lr=1e-4)
        obs = torch.randint(0, 256, (8192, obs_bytes), dtype=torch.uint8, device=dev)
        with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
            for _ in range(5):
                net(obs[:1024])
            torch.cuda.synchronize()
            t0, k = time.perf_counter(), 0
            while time.perf_counter() - t0 < 4.0:
                net(obs[:1024])
                k += 1
            torch.cuda.synchronize()
            inf = k * 1024 / (time.perf_counter() - t0)
        act = torch.randint(0, n_actions, (8192,), device=dev)
        t0, k = None, 0
        for i in range(25):
            if i == 5:
                torch.cuda.synchronize()
                t0 = time.perf_counter()
            opt.zero_grad(set_to_none=True)
            for c in range(0, 8192, 2048):  # 4 micro-batches of 2,048 make one 8,192 step
                with torch.autocast("cuda", dtype=torch.bfloat16):
                    logits, v = net(obs[c : c + 2048])
                    loss = (torch.nn.functional.cross_entropy(logits.float(), act[c : c + 2048])
                            + v.float().pow(2).mean())
                loss.backward()
            opt.step()
            k += i >= 5
        torch.cuda.synchronize()
        upd = k * 8192 / (time.perf_counter() - t0)
        print(f"net  {layers}x256 {params / 1e6:5.2f}M params  inference {inf:10,.0f} decisions/s "
              f"(batch 1024)  update {upd:9,.0f} decisions/s per epoch (minibatch 8192)")


def bench_loop(n=2048, threads=14, layers=4, secs=20.0):
    import torch

    dev = torch.device("cuda")
    envs = Sts2RunEnvs(n, seed_range=SEEDS, cycle=True, character="Necrobinder", ascension=10,
                       combat="agent", threads=threads)
    net, _ = make_net(layers, envs.actions, dev)
    shifts = torch.arange(64, device=dev, dtype=torch.int64)
    steps = 0
    t0 = time.perf_counter()
    with torch.no_grad(), torch.autocast("cuda", dtype=torch.bfloat16):
        while time.perf_counter() - t0 < secs:
            obs = torch.from_numpy(envs.obs).to(dev, non_blocking=True)
            words = torch.from_numpy(envs.masks.view(np.int64)).to(dev)
            legal = ((words.unsqueeze(-1) >> shifts) & 1).flatten(1)[:, : envs.actions].bool()
            logits, _ = net(obs)
            logits = logits.float().masked_fill(~legal, float("-inf"))
            a = torch.distributions.Categorical(logits=logits).sample()
            envs.step(a.to(torch.int64).cpu().numpy())
            steps += n
    dt = time.perf_counter() - t0
    print(f"loop envs={n} threads={threads} {layers}x256  {steps / dt:10,.0f} decisions/s "
          "(env + copy + forward + sample)")
    envs.close()


if __name__ == "__main__":
    what = sys.argv[1] if len(sys.argv) > 1 else "all"
    if what in ("env", "all"):
        bench_env()
    if what in ("net", "all"):
        bench_net()
    if what in ("loop", "all"):
        bench_loop()
