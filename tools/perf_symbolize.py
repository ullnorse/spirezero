#!/usr/bin/env python3
"""Inclusive/self profile from a `perf record -g` file, for a perf built without libelf.

WSL2's `perf` (`/usr/lib/linux-tools/*/bin/perf`) cannot symbolise, so this maps the raw
frame-pointer call chains of `perf script` to file addresses of BINARY and names them with
`addr2line -f -C -i` (inlined frames included). Build the binary with
`RUSTFLAGS="-C force-frame-pointers=yes" CARGO_PROFILE_RELEASE_DEBUG=line-tables-only`.

    perf record -F 2000 -g -o DATA -- BINARY ARGS
    tools/perf_symbolize.py DATA BINARY [TOP] [FILTER]

Prints the TOP functions (default 40) by inclusive share (each counted once per sample) with
their self share (innermost inlined frame of the sampled instruction). FILTER keeps only names
containing it; `callers:NAME` instead splits NAME's inclusive share by its nearest callers. Frames outside BINARY (libc's malloc and friends) count as their DSO.
"""

import collections
import re
import subprocess
import sys


def segments(binary):
    """(file offset, vaddr, size) of the executable LOAD segments."""
    out = subprocess.run(["readelf", "-lW", binary], capture_output=True, text=True, check=True).stdout
    segs = []
    for line in out.splitlines():
        f = line.split()
        if f and f[0] == "LOAD" and "E" in f[6:-1]:
            segs.append((int(f[1], 16), int(f[2], 16), int(f[5], 16)))
    return segs


def main():
    data, binary = sys.argv[1], sys.argv[2]
    top = int(sys.argv[3]) if len(sys.argv) > 3 else 40
    needle = sys.argv[4] if len(sys.argv) > 4 else None
    segs = segments(binary)
    script = subprocess.run(
        ["perf", "script", "-i", data, "--show-mmap-events", "-F", "ip,dso"],
        capture_output=True, text=True, check=False,
    ).stdout
    maps = []  # (start, end, pgoff) of BINARY's executable mappings
    mmap = re.compile(r"\[0x([0-9a-f]+)\(0x([0-9a-f]+)\) @ 0x([0-9a-f]+).*\]: r-xp (\S+)")
    samples, stack = [], []
    for line in script.splitlines():
        m = mmap.search(line)
        if m and m.group(4) == binary:
            start, size, pgoff = (int(m.group(i), 16) for i in (1, 2, 3))
            maps.append((start, start + size, pgoff))
            continue
        line = line.strip()
        if not line:
            if stack:
                samples.append(stack)
            stack = []
            continue
        if "PERF_RECORD" in line:
            continue
        ip, _, dso = line.partition(" ")
        stack.append((int(ip, 16), dso.strip("()")))
    if stack:
        samples.append(stack)

    def vaddr(ip):
        for start, end, pgoff in maps:
            if start <= ip < end:
                off = ip - start + pgoff
                for s_off, s_vaddr, s_size in segs:
                    if s_off <= off < s_off + s_size:
                        return off - s_off + s_vaddr
        return None

    addrs = sorted({a for s in samples for ip, d in s if d == binary and (a := vaddr(ip)) is not None})
    names = {}
    if addrs:
        out = subprocess.run(
            ["addr2line", "-f", "-C", "-i", "-a", "-e", binary],
            input="\n".join(hex(a) for a in addrs), capture_output=True, text=True, check=True,
        ).stdout.splitlines()
        cur, chain, i = None, [], 0
        while i < len(out):
            if re.fullmatch(r"0x[0-9a-f]+", out[i]):
                if cur is not None:
                    names[cur] = chain
                cur, chain = int(out[i], 16), []
                i += 1
                continue
            func = re.sub(r"::h[0-9a-f]{16}$", "", out[i])
            loc = out[i + 1].split(" ")[0] if i + 1 < len(out) else "?"
            loc = re.sub(r".*/(crates|library|src)/", r"\1/", loc)
            chain.append(f"{func} [{loc}]")
            i += 2
        if cur is not None:
            names[cur] = chain

    incl, self_ = collections.Counter(), collections.Counter()
    callers = collections.Counter()
    for s in samples:
        seen = set()
        chain = []
        for depth, (ip, dso) in enumerate(s):
            # Return addresses point after the call: step back one byte so the call's line shows.
            a = vaddr(ip if depth == 0 else ip - 1) if dso == binary else None
            frames = names.get(a) if a is not None else None
            if a is not None and frames is None and depth > 0:
                frames = names.get(vaddr(ip))
            frames = frames or [f"[{dso.rsplit('/', 1)[-1]}]"]
            if depth == 0:
                self_[frames[0]] += 1
            for f in frames:
                key = f.split(" [")[0] if "[" in f and not f.startswith("[") else f
                chain.append(key)
                if key not in seen:
                    seen.add(key)
                    incl[key] += 1
        if needle is not None and needle.startswith("callers:"):
            target = needle[len("callers:"):]
            if target in chain:
                outer = [k for k in chain[chain.index(target) + 1:] if "sts2_core" in k or re.fullmatch(r"\w+", k)]
                callers[" < ".join(outer[:3])] += 1
    if callers:
        print(f"{len(samples)} samples; callers of the target (inclusive %):")
        for k, v in callers.most_common(top):
            print(f"{100 * v / len(samples):6.1f}  {k[:200]}")
        return
    selfn = collections.Counter()
    for f, n in self_.items():
        selfn[f.split(" [")[0] if not f.startswith("[") else f] += n
    total = len(samples)
    print(f"{total} samples")
    rows = [(k, v) for k, v in incl.most_common() if needle is None or needle in k]
    for k, v in rows[:top]:
        print(f"{100 * v / total:6.1f} {100 * selfn[k] / total:6.1f}  {k[:150]}")


if __name__ == "__main__":
    main()
