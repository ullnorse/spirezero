#!/usr/bin/env python3
"""Extract the game's per-class flags (overrides, interfaces, tags, keywords, pools) from the decomp.

Usage: python3 tools/flag_truth.py [<ref dir>] [--out DIR]
  <ref dir>  the dir that contains decomp/ (default ref/ of this checkout)
  --out DIR  where to write <kind>.tsv (default crates/sts2-core/src/content/flags)

Writes one TSV per kind: the `ModelId.Entry` of every model class plus the value of each flag,
derived from the class declaration (base constructor arguments, property and method overrides,
interfaces), its base classes and the pool files. It copies no decomp code, only ids and values.
A value the regexes cannot decide (a conditional override) is written as `?`; the Rust test
(`content/flags.rs`) skips `?` cells, so check those by hand. Scans with python3 stdlib only.
"""
import re
import sys
from pathlib import Path

ARGS = [a for a in sys.argv[1:]]
OUT = Path(__file__).resolve().parent.parent / "crates/sts2-core/src/content/flags"
if "--out" in ARGS:
    i = ARGS.index("--out")
    OUT = Path(ARGS[i + 1])
    del ARGS[i:i + 2]
REF = Path(ARGS[0]) if ARGS else Path(__file__).resolve().parent.parent / "ref"
DECOMP = REF / "decomp"
NS = "MegaCrit.Sts2.Core.Models"

COMMENT = re.compile(r'//[^\n]*|/\*.*?\*/|@"(?:[^"]|"")*"|"(?:\\.|[^"\\])*"', re.S)


def strip(text):
    """Comments removed, string literals kept."""
    return COMMENT.sub(lambda m: m.group(0) if m.group(0)[0] in '@"' else " ", text)


def slug(name):
    """`StringHelper.Slugify` of a class name: `FlameBarrier` -> `FLAME_BARRIER`."""
    s = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    return s.upper()


class Cls:
    def __init__(self, path):
        self.name = path.stem
        self.code = strip(path.read_text(encoding="utf-8"))
        m = re.search(r"\bclass\s+" + self.name + r"\b\s*(?::\s*([^\n{]+))?", self.code)
        self.bases = [b.strip() for b in m.group(1).split(",")] if m and m.group(1) else []
        self.abstract = bool(re.search(r"\babstract\s+class\s+" + self.name + r"\b", self.code))

    def member(self, name, method=False):
        """The body expression of an override of property (or parameterless-ish method) `name`,
        `None` if not overridden, `?` if it is not a single expression."""
        pat = r"\boverride\s+[\w<>?.]+\s+" + name + (r"\s*\([^)]*\)" if method else r"\b(?!\s*\()")
        m = re.search(pat, self.code)
        if not m:
            return None
        rest = self.code[m.end():].lstrip()
        if rest.startswith("=>"):
            return rest[2:rest.index(";")].strip()
        if not rest.startswith("{"):
            return "?"
        depth = 0
        for i, ch in enumerate(rest):
            depth += ch == "{"
            depth -= ch == "}"
            if depth == 0:
                block = rest[1:i]
                break
        block = re.sub(r"^\s*get\s*", "", block.strip())
        if block.startswith("=>"):
            return block[2:].split(";")[0].strip()
        inner = block.strip()
        if inner.startswith("{") and inner.endswith("}"):
            inner = inner[1:-1].strip()
        m = re.fullmatch(r"return\s+([^;]+);", inner)
        if m:
            return m.group(1).strip()
        m = re.fullmatch(r"if\s*\((!?)(\w+)\)\s*\{?\s*return\s+([\w.]+);\s*\}?\s*return\s+([\w.]+);", inner)
        if m:
            neg, cond, then, other = m.groups()
            yes, no = (other, then) if neg else (then, other)
            return f"{cond} ? {yes} : {no}"
        return "?"


def load(sub):
    return {f.stem: Cls(f) for f in sorted((DECOMP / f"{NS}.{sub}").glob("*.cs"))}


def chain(classes, cls, root):
    """`cls` and its model base classes up to (not including) `root`."""
    out = [cls]
    while out[-1].bases and out[-1].bases[0] != root and out[-1].bases[0] in classes:
        out.append(classes[out[-1].bases[0]])
    return out


def lookup(classes, cls, root, name, method=False):
    for c in chain(classes, cls, root):
        v = c.member(name, method)
        if v is not None:
            return v, c
    return None, None


def literal(expr, enum=None):
    """A literal value as a TSV cell, `?` otherwise."""
    if expr is None:
        return None
    e = expr.strip().rstrip("m")
    if e in ("true", "false") or re.fullmatch(r"-?\d+", e):
        return e
    if enum and re.fullmatch(enum + r"\.\w+", e):
        return e.split(".")[1]
    return "?"


def pools(sub, ref):
    """id -> pool name (without the `<Kind>Pool` suffix), from the pool files."""
    out = {}
    for f in sorted((DECOMP / f"{NS}.{sub}").glob("*.cs")):
        pool = re.sub(r"(Card|Relic|Potion)Pool$", "", f.stem)
        code = strip(f.read_text(encoding="utf-8"))
        # A character potion pool returns its epoch's list (`Ironclad4Epoch.Potions`).
        for epoch in re.findall(r"\breturn\s+(\w+Epoch)\.\w+;", code):
            code += strip((DECOMP / "MegaCrit.Sts2.Core.Timeline.Epochs" / f"{epoch}.cs")
                          .read_text(encoding="utf-8"))
        for name in re.findall(r"ModelDb\." + ref + r"<(\w+)>", code):
            out.setdefault(name, []).append(pool)
    return {k: "+".join(dict.fromkeys(v)) for k, v in out.items()}


def b(value):
    return "true" if value else "false"


def cards():
    classes = load("Cards")
    pool = pools("CardPools", "Card")
    keywords = ["Exhaust", "Ethereal", "Innate", "Retain", "Unplayable", "Sly", "Eternal"]
    tags = ["Strike", "Defend", "OstyAttack", "Shiv", "Minion"]
    header = ["id", "type", "rarity", "target", "cost", "cost_x", "star_cost", "star_cost_x",
              *(k.lower() for k in keywords), "gains_block",
              *("tag_" + re.sub(r"(?<=[a-z])(?=[A-Z])", "_", t).lower() for t in tags),
              "max_upgrade_level", "turn_end_in_hand", "generated_in_combat", "generated_by_modifiers",
              "multiplayer_only", "pool", "visual_pool"]
    rows = []
    for cls in classes.values():
        if cls.abstract or cls.bases[:1] != ["CardModel"]:
            continue
        m = re.search(r":\s*base\(\s*(-?\d+)\s*,\s*CardType\.(\w+)\s*,\s*CardRarity\.(\w+)\s*,"
                      r"\s*TargetType\.(\w+)", cls.code)
        cost, ctype, rarity, target = m.groups() if m else ("?",) * 4
        if cls.member("Type") is not None:
            ctype = literal(cls.member("Type"), "CardType")
        if cls.member("TargetType") is not None:
            target = literal(cls.member("TargetType"), "TargetType")

        def flag(name, default, enum=None):
            v = cls.member(name)
            return default if v is None else literal(v, enum)

        kw = cls.member("CanonicalKeywords")
        kw_cells = (["?"] * len(keywords) if kw == "?" or (kw and "?" in kw)
                    else [b(kw and f"CardKeyword.{k}" in kw) for k in keywords])
        tg = cls.member("CanonicalTags")
        tag_cells = (["?"] * len(tags) if tg == "?" or (tg and " ? " in tg)
                     else [b(tg and f"CardTag.{t}" in tg) for t in tags])
        mp = cls.member("MultiplayerConstraint")
        rows.append([
            slug(cls.name), ctype, rarity, target, cost,
            flag("HasEnergyCostX", "false"), flag("CanonicalStarCost", "-1"),
            flag("HasStarCostX", "false"), *kw_cells, flag("GainsBlock", "false"), *tag_cells,
            flag("MaxUpgradeLevel", "1"), flag("HasTurnEndInHandEffect", "false"),
            flag("CanBeGeneratedInCombat", "true"), flag("CanBeGeneratedByModifiers", "true"),
            "false" if mp is None else literal(mp, "CardMultiplayerConstraint").replace(
                "MultiplayerOnly", "true").replace("None", "false"),
            pool.get(cls.name, "-"),
            visual_pool(cls, pool.get(cls.name, "-")),
        ])
    return header, rows


def visual_pool(cls, pool):
    """`VisualCardPool`: an override returning `ModelDb.CardPool<X>()`, else `Pool`."""
    v = cls.member("VisualCardPool")
    if v is None:
        return pool
    m = re.fullmatch(r"ModelDb\.CardPool<(\w+)CardPool>\(\)", v)
    return m.group(1) if m else "?"


def powers():
    classes = load("Powers")
    header = ["id", "type", "stack_type", "instance_type", "allow_negative", "visible",
              "temporary", "removed_after_owner_death", "owner_death_triggers_fatal",
              "owner_is_secondary_enemy"]
    rows = []
    for cls in classes.values():
        if cls.abstract:
            continue
        line = chain(classes, cls, "PowerModel")

        def get(name, default, enum=None, method=False):
            v, _ = lookup(classes, cls, "PowerModel", name, method)
            return default if v is None else literal(v, enum)

        ptype, _ = lookup(classes, cls, "PowerModel", "Type")
        m = re.fullmatch(r"IsPositive \? PowerType\.(\w+) : PowerType\.(\w+)", ptype or "")
        if m:
            # `IsPositive` is a `protected virtual` of the abstract base, true unless overridden.
            pos, _ = lookup(classes, cls, "PowerModel", "IsPositive")
            ptype_cell = m.group(1) if (pos or "true") == "true" else m.group(2)
        else:
            ptype_cell = literal(ptype, "PowerType")
        temporary = any("ITemporaryPower" in c.bases for c in line)
        rows.append([
            slug(cls.name), ptype_cell, get("StackType", "?", "PowerStackType"),
            get("InstanceType", "None", "PowerInstanceType"), get("AllowNegative", "false"),
            get("IsVisibleInternal", "true"), b(temporary),
            get("ShouldPowerBeRemovedAfterOwnerDeath", "true", method=True),
            get("ShouldOwnerDeathTriggerFatal", "true", method=True),
            get("OwnerIsSecondaryEnemy", "false"),
        ])
    return header, rows


def relics():
    classes = load("Relics")
    pool = pools("RelicPools", "Relic")
    header = ["id", "rarity", "allowed_in_shops", "pool"]
    rows = []
    for cls in classes.values():
        if cls.abstract:
            continue
        rows.append([slug(cls.name), literal(cls.member("Rarity"), "RelicRarity") or "?",
                     literal(cls.member("IsAllowedInShops")) or "true",
                     pool.get(cls.name, "-")])
    return header, rows


def potions():
    classes = load("Potions")
    pool = pools("PotionPools", "Potion")
    header = ["id", "rarity", "usage", "target", "generated_in_combat", "pool"]
    rows = []
    for cls in classes.values():
        if cls.abstract:
            continue
        rows.append([slug(cls.name), literal(cls.member("Rarity"), "PotionRarity") or "?",
                     literal(cls.member("Usage"), "PotionUsage") or "?",
                     literal(cls.member("TargetType"), "TargetType") or "?",
                     literal(cls.member("CanBeGeneratedInCombat")) or "true",
                     pool.get(cls.name, "-")])
    return header, rows


def card_type_set(cls, method):
    """The card types `method(CardType cardType)` accepts, for a `cardType == CardType.X || ...`
    or `cardType != CardType.X && ...` body; `?` for anything else."""
    v = cls.member(method, method=True)
    types = ["Attack", "Skill", "Power", "Status", "Curse", "Quest"]
    if v is None:
        return "*"
    v = v.replace("(", "").replace(")", "")
    eq = re.fullmatch(r"cardType == CardType\.\w+(?:\s*\|\|\s*cardType == CardType\.\w+)*", v)
    ne = re.fullmatch(r"cardType != CardType\.\w+(?:\s*&&\s*cardType != CardType\.\w+)*", v)
    named = re.findall(r"CardType\.(\w+)", v)
    if eq:
        return "|".join(t for t in types if t in named)
    if ne:
        return "|".join(t for t in types if t not in named)
    if v == "true":
        return "*"
    return "?"


def enchantments():
    classes = load("Enchantments")
    header = ["id", "stackable", "card_types", "start_at_bottom"]
    rows = []
    for cls in classes.values():
        if cls.abstract:
            continue
        rows.append([slug(cls.name), literal(cls.member("IsStackable")) or "false",
                     card_type_set(cls, "CanEnchantCardType"),
                     literal(cls.member("ShouldStartAtBottomOfDrawPile")) or "false"])
    return header, rows


def afflictions():
    classes = load("Afflictions")
    header = ["id", "stackable", "card_types", "afflict_unplayable"]
    rows = []
    for cls in classes.values():
        if cls.abstract:
            continue
        rows.append([slug(cls.name), literal(cls.member("IsStackable")) or "false",
                     card_type_set(cls, "CanAfflictCardType"),
                     literal(cls.member("CanAfflictUnplayableCards")) or "true"])
    return header, rows


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for kind, fn in [("cards", cards), ("powers", powers), ("relics", relics),
                     ("potions", potions), ("enchantments", enchantments),
                     ("afflictions", afflictions)]:
        header, rows = fn()
        rows.sort()
        unknown = sum(c == "?" for r in rows for c in r)
        text = "\t".join(header) + "\n" + "".join("\t".join(r) + "\n" for r in rows)
        (OUT / f"{kind}.tsv").write_text(text, encoding="utf-8")
        print(f"{kind}: {len(rows)} rows, {unknown} undecided cells")


if __name__ == "__main__":
    main()
