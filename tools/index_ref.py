#!/usr/bin/env python3
"""Build ref/INDEX.tsv: one row per game entity in the local decomp.

Columns: kind, id, pools (comma-separated), lines, path (relative to ref/).
Pools come from CardPools/RelicPools/PotionPools and, for encounters and
events, from the Acts that list them.
"""
import re
import sys
from collections import defaultdict
from pathlib import Path

REF = Path(sys.argv[1] if len(sys.argv) > 1 else "ref")
DECOMP = REF / "decomp"
NS = "MegaCrit.Sts2.Core.Models."
KINDS = {
    "Cards": "card", "Relics": "relic", "Powers": "power", "Monsters": "monster",
    "Encounters": "encounter", "Events": "event", "Potions": "potion",
    "Enchantments": "enchantment", "Afflictions": "affliction", "Orbs": "orb",
    "Acts": "act", "Characters": "character", "Modifiers": "modifier",
}
POOL_DIRS = ["CardPools", "RelicPools", "PotionPools", "Acts"]
REF_RE = re.compile(r"ModelDb\.(?:Card|Relic|Potion|Encounter|Event|AncientEvent)<(\w+)>")

pools = defaultdict(set)
for pool_dir in POOL_DIRS:
    for f in sorted((DECOMP / (NS + pool_dir)).glob("*.cs")):
        for name in REF_RE.findall(f.read_text(encoding="utf-8")):
            pools[name].add(f.stem)

rows = []
for sub, kind in KINDS.items():
    for f in sorted((DECOMP / (NS + sub)).glob("*.cs")):
        lines = f.read_text(encoding="utf-8").count("\n")
        rows.append((kind, f.stem, ",".join(sorted(pools[f.stem])) or "-", str(lines),
                     str(f.relative_to(REF))))

with open(REF / "INDEX.tsv", "w", encoding="utf-8") as out:
    out.write("kind\tid\tpools\tlines\tpath\n")
    for row in rows:
        out.write("\t".join(row) + "\n")
print(f"{len(rows)} rows -> {REF / 'INDEX.tsv'}")
