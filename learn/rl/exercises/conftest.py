"""pytest setup: puts `python/` (the sts2 binding) and this directory on sys.path, and lets
`RL_SOLUTIONS=1` run the same tests against the reference solutions."""

import importlib
import os
import sys
import pathlib

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
for p in (ROOT / "python", HERE):
    if str(p) not in sys.path:
        sys.path.insert(0, str(p))


def exercise(name):
    """`exercise("week1.day1_loop")` imports your file, or the solution with RL_SOLUTIONS=1."""
    prefix = "solutions." if os.environ.get("RL_SOLUTIONS") else ""
    return importlib.import_module(prefix + name)
