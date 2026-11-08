#!/usr/bin/env python3
"""Resolve registry merge conflicts by union: ours, then theirs' lines not already in ours.

For append-only registries (mod lists, enum variants, ALL arrays, match arms). Review the result
and fix array lengths with the compiler. Usage: tools/union_resolve.py FILE...
"""
import re
import sys

ALL_HDR = re.compile(r"pub const ALL: \[Self; \d+\] = \[")


def key(line):
    """Lines that are the same registry slot (e.g. `ALL` headers of different lengths)."""
    return ALL_HDR.sub("pub const ALL", line.strip())


PAT = re.compile(r"<<<<<<< [^\n]*\n(.*?)=======\n(.*?)>>>>>>> [^\n]*\n", re.S)


def union(m):
    ours, theirs = m.group(1).splitlines(True), m.group(2).splitlines(True)
    seen = {key(l) for l in ours}
    out = list(ours)
    for line in theirs:
        if line.strip() and key(line) not in seen:
            out.append(line)
            seen.add(key(line))
    return "".join(out)


for path in sys.argv[1:]:
    with open(path) as f:
        text = f.read()
    with open(path, "w") as f:
        f.write(PAT.sub(union, text))
