#!/usr/bin/env python3
"""Build docs/inventory/<character>-<act>.{json,md}: every entity one character's Act 1 run can touch.

Usage: python3 tools/inventory.py <ref dir> [--character C --act A | --all]
  <ref dir>      the dir that contains decomp/
  --character C  Ironclad, Silent, Defect, Regent or Necrobinder (default Necrobinder)
  --act A        overgrowth, underdocks, hive or glory (default overgrowth)
  --all          every (character, act) pair

Scans the decomp with regexes only (python3 stdlib, deterministic output). It copies
no decomp code, only ids, numbers, names and paths. Whatever a regex cannot decide is
written as null and counted in the md.
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

REF = Path(next((a for a in sys.argv[1:] if not a.startswith("--")), "ref"))
DECOMP = REF / "decomp"
OUT = Path(__file__).resolve().parent.parent / "docs" / "inventory"
NS = "MegaCrit.Sts2.Core.Models."

CHARACTERS = ["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"]
ACT_FILES = {"overgrowth": "Overgrowth", "underdocks": "Underdocks", "hive": "Hive", "glory": "Glory"}
# Acts 2 and 3: the shared ancients (Darv, in no act's `AllAncients`) can be drawn there.
SHARED_ANCIENT_ACTS = {"Hive", "Glory"}


def root_pools(character):
    """The pools a run as `character` draws from: its own, plus the shared and colorless ones."""
    return {
        "card": [f"{character}CardPool", "ColorlessCardPool"],
        "relic": [f"{character}RelicPool", "SharedRelicPool"],
        "potion": [f"{character}PotionPool", "SharedPotionPool"],
    }


DIRS = {
    "Cards": "card", "Relics": "relic", "Potions": "potion", "Powers": "power",
    "Monsters": "monster", "Encounters": "encounter", "Events": "event",
    "Enchantments": "enchantment", "Afflictions": "affliction",
}
KIND_ORDER = list(DIRS.values())
# Support code in-scope content hands control to: its model references count as the caller's own.
# Namespaces under `MegaCrit.Sts2.Core.`; `exclude` files are skipped, with the reason.
SUPPORT_DIRS = {
    "Entities.RestSite": set(),    # rest-site options (Hatch, Dig, Lift, Clone, Kindle, ...)
    "Rewards": {"RewardsSet"},     # RewardsSet: only the tutorial branches name models (BlockPotion, Vajra...)
    "Commands": set(),             # static command classes (OstyCmd applies DieForYouPower)
    "Factories": set(),            # RelicFactory.FallbackRelic (Circlet)
}
RARITY_KINDS = {"card", "relic", "potion", "encounter"}
POOL_DIRS = {"CardPools": "card", "RelicPools": "relic", "PotionPools": "potion"}
MODEL_NAME = {"card": "Card", "relic": "Relic", "potion": "Potion", "event": "Event",
              "encounter": "Encounter", "monster": "Monster"}
ENTITY_REF_KINDS = {"Card": "card", "Relic": "relic", "Potion": "potion", "Event": "event",
                    "AncientEvent": "event", "Encounter": "encounter", "Monster": "monster"}

COSMETIC_CMDS = {"SfxCmd", "VfxCmd", "TalkCmd", "ThinkCmd"}
COSMETIC_CALLS = {"CreatureCmd.TriggerAnim", "CardCmd.PreviewCardPileAdd", "CardCmd.Preview",
                  "Cmd.Wait", "ForgeCmd.PlayCombatRoomForgeVfx", "CardPileCmd.AddToCombatAndPreview"}
ATTACK_BUILDERS = ["FromCard", "FromOsty", "FromMonster", "Targeting", "TargetingAllOpponents",
                   "TargetingRandomOpponents", "Unpowered", "WithHitCount", "BeforeDamage"]
HOOK_EXCLUDE = {"OnPlay", "OnUse", "OnUpgrade", "OnEnqueuePlayVfx", "ShouldShowMoveInBestiary",
                "CompareTo"}
HOOK_BASES = ["CardModel", "RelicModel", "PotionModel", "PowerModel", "MonsterModel",
              "EncounterModel", "EventModel", "EnchantmentModel", "AfflictionModel"]

LITERAL = r"-?\d+(?:\.\d+)?m?"
STRING_OR_COMMENT = re.compile(
    r'@"(?:[^"]|"")*"|"(?:\\.|[^"\\\n])*"|//[^\n]*|/\*.*?\*/', re.S)


def read(path):
    return Path(path).read_text(encoding="utf-8")


def strip_comments(text):
    return STRING_OR_COMMENT.sub(lambda m: m.group(0) if m.group(0)[0] in '@"' else " ", text)


def blank_strings(text):
    return STRING_OR_COMMENT.sub(lambda m: '""' if m.group(0)[0] in '@"' else " ", text)


def num(text):
    """Literal like 6m / -1 / 0.5m -> int when integral, else a decimal string; no floats."""
    text = text.strip().rstrip("m")
    if re.fullmatch(r"-?\d+", text):
        return int(text)
    if re.fullmatch(r"-?\d+\.\d+", text):
        whole, frac = text.split(".")
        frac = frac.rstrip("0")
        return int(whole) if not frac else f"{whole}.{frac}"
    return None


class Entity:
    def __init__(self, kind, eid, path, raw):
        self.kind = kind
        self.id = eid
        self.path = path
        self.code = strip_comments(raw)
        self.blank = blank_strings(raw)
        m = re.search(r"\bclass\s+\w+\s*(?::\s*([^\n{]+))?", self.blank)
        self.bases = [b.strip() for b in m.group(1).split(",")] if m and m.group(1) else []
        self.abstract = bool(re.search(r"\babstract\s+class\b", self.blank))


def load_entities():
    ents = {}
    for sub, kind in DIRS.items():
        for f in sorted((DECOMP / (NS + sub)).glob("*.cs")):
            ents[(kind, f.stem)] = Entity(kind, f.stem, str(f.relative_to(REF)), read(f))
    return ents


def load_support():
    """class name -> Entity for the support namespaces."""
    out = {}
    for sub, skip in SUPPORT_DIRS.items():
        for f in sorted((DECOMP / ("MegaCrit.Sts2.Core." + sub)).glob("*.cs")):
            if f.stem not in skip:
                out[f.stem] = Entity("support", f.stem, str(f.relative_to(REF)), read(f))
    return out


def model_refs(text):
    """ModelDb.Card<X>() style references -> [(kind, id)] in file order."""
    return [(ENTITY_REF_KINDS[k], i)
            for k, i in re.findall(r"ModelDb\.(Card|Relic|Potion|Event|AncientEvent|Encounter|Monster)<(\w+)>", text)]


def pool_membership():
    """(kind, id) -> sorted pool names, from the pool, act and epoch files."""
    pools = defaultdict(set)
    for sub in POOL_DIRS:
        for f in sorted((DECOMP / (NS + sub)).glob("*.cs")):
            text = strip_comments(read(f))
            refs = model_refs(text)
            for epoch in re.findall(r"\b(\w+Epoch)\.(?:Potions|Relics|Cards)\b", text):
                refs += model_refs(read(DECOMP / "MegaCrit.Sts2.Core.Timeline.Epochs" / f"{epoch}.cs"))
            for ref in refs:
                pools[ref].add(f.stem)
    for f in sorted((DECOMP / (NS + "Acts")).glob("*.cs")):
        for ref in model_refs(strip_comments(read(f))):
            pools[ref].add(f.stem)
    shared = strip_comments(read(DECOMP / (NS[:-1]) / "ModelDb.cs"))
    block = re.search(r"AllSharedEvents\s*=>.*?\}\)\);", shared, re.S)
    for name in re.findall(r"\bEvent<(\w+)>", block.group(0)):
        pools[("event", name)].add("SharedEvents")
    return {k: sorted(v) for k, v in pools.items()}


def epoch_gates():
    gates = defaultdict(set)
    for f in sorted((DECOMP / "MegaCrit.Sts2.Core.Timeline.Epochs").glob("*Epoch.cs")):
        for ref in model_refs(strip_comments(read(f))):
            gates[ref].add(f.stem)
    return {k: sorted(v) for k, v in gates.items()}


def roots(pools, ents, character, act):
    """(kind, id) -> sorted list of reasons it is a root."""
    out = defaultdict(set)
    for kind, names in root_pools(character).items():
        for key, pool_names in pools.items():
            if key[0] == kind and key in ents:
                for p in pool_names:
                    if p in names:
                        out[key].add(p)
    text = strip_comments(read(DECOMP / (NS + "Acts") / f"{act}.cs"))
    for key in model_refs(text):
        if key[0] in ("encounter", "event") and key in ents:
            out[key].add(act)
    for key, pool_names in pools.items():
        if "SharedEvents" in pool_names and key in ents:
            out[key].add("SharedEvents")
    if act in SHARED_ANCIENT_ACTS:
        # ModelDb.AllSharedAncients: the ancients that belong to no act (Darv), drawn for Acts 2 and 3.
        modeldb = strip_comments(read(DECOMP / (NS[:-1]) / "ModelDb.cs"))
        block = re.search(r"AllSharedAncients\s*=>(.*?);", modeldb, re.S)
        for key in model_refs(re.sub(r"\b(AncientEvent)<", r"ModelDb.\1<", block.group(1))):
            if key in ents:
                out[key].add("SharedAncients")
    # NeowsBones (offered by Neow) adds random curses from CurseCardPool, minus the curses that opt
    # out through `CanBeGeneratedByModifiers => false`.
    if ("relic", "NeowsBones") in ents:
        for key, pool_names in pools.items():
            if key[0] == "card" and "CurseCardPool" in pool_names and key in ents and \
                    not re.search(r"CanBeGeneratedByModifiers\s*=>\s*false", ents[key].code):
                out[key].add("NeowsBones")
    return {k: sorted(v) for k, v in out.items()}


def hook_names():
    text = strip_comments(read(DECOMP / (NS[:-1]) / "AbstractModel.cs"))
    names = set(re.findall(r"public\s+virtual\s+[^\n(=]*?\b(\w+)\s*\(", text))
    for base in HOOK_BASES:
        btext = strip_comments(read(DECOMP / (NS[:-1]) / f"{base}.cs"))
        for n in re.findall(r"(?:public|protected)\s+virtual\s+[^\n(=]*?\b((?:Before|After|Modify|Should|On|Try)\w*)\s*\(", btext):
            names.add(n)
    return names - HOOK_EXCLUDE


OVERRIDE_RE = re.compile(
    r"\b(?:public|protected|internal)\s+override\s+(?:async\s+)?[\w<>\[\]?,.() ]+?\s+(\w+)\s*\(")


def extract_hooks(ent, hooks):
    return sorted({n for n in OVERRIDE_RE.findall(ent.blank) if n in hooks})


def extract_commands(ent):
    found = set()
    for cls, meth, gen in re.findall(r"\b([A-Z]\w*Cmd)\.(\w+)(?:<([\w, ]+)>)?\(", ent.blank):
        if cls in COSMETIC_CMDS or f"{cls}.{meth}" in COSMETIC_CALLS:
            continue
        found.add(f"{cls}.{meth}" + (f"<{gen.replace(' ', '')}>" if gen else ""))
    if "DamageCmd.Attack" in found:
        for b in ATTACK_BUILDERS:
            if re.search(rf"\.{b}\(", ent.blank):
                found.add(f"AttackCommand.{b}")
    return sorted(found)


def extract_rng(ent):
    streams = set()
    blank = ent.blank
    for s in re.findall(r"\bPlayerRng\.(\w+)", blank):
        streams.add(f"Player.{s}")
    for s in re.findall(r"(?<!Player)\bRng\.([A-Z]\w+)", blank):
        if not s.startswith("Next") and s != "Shuffle":
            streams.add(s)
    for s in re.findall(r"\bRunRng\.([A-Z]\w+)", blank):
        streams.add(s)
    if re.search(r"(?<![\w.])Rng\b", blank) or re.search(r"(?<![\w.])Rng\.(?:Next\w+|Shuffle)", blank):
        streams.add({"encounter": "EncounterRng", "event": "EventRng"}.get(ent.kind, "OwnRng"))
    if re.search(r"\bRng\.Shuffle\b", blank) and not re.search(r"(?<![\w.])Rng\.Shuffle", blank):
        streams.add("Shuffle")
    return sorted(streams)


def var_name(cls, generic, args):
    if cls == "PowerVar" and generic:
        return generic[:-5] if generic.endswith("Power") else generic
    if cls in ("DynamicVar", "IntVar", "StringVar", "CalculatedVar") and args:
        m = re.match(r'\s*"(\w+)"', args)
        if m:
            return m.group(1)
    return cls[:-3] if cls.endswith("Var") else cls


def extract_numbers(ent):
    """Literal values only; anything computed is left out or null."""
    numbers = {}
    code = ent.code
    m = re.search(r"\bbase\(\s*(-?\d+)\s*,\s*CardType\.(\w+)\s*,\s*CardRarity\.(\w+)\s*,\s*TargetType\.(\w+)\s*\)", code)
    if ent.kind == "card":
        numbers["cost"] = int(m.group(1)) if m else None
    vars_ = []
    region = re.search(r"CanonicalVars\s*=>(.*?)(?:\n\t(?:protected|public|private|internal)\b|\n\}|\Z)", code, re.S)
    if region:
        for cls, gen, args in re.findall(r"new\s+(\w*Var)(?:<(\w+)>)?\(((?:[^()]|\([^()]*\))*)\)", region.group(1)):
            if cls == "StringVar":
                continue
            lit = re.search(rf"(?:^|,)\s*({LITERAL})\s*(?:,|$)", args)
            value = num(lit.group(1)) if lit else None
            entry = {"var": var_name(cls, gen, args), "type": cls + (f"<{gen}>" if gen else ""),
                     "value": value}
            prop = re.search(r"ValueProp\.(\w+)", args)
            if prop:
                entry["prop"] = prop.group(1)
            vars_.append(entry)
    if vars_:
        numbers["vars"] = vars_
    upgrades = []
    for name, delta in re.findall(r'DynamicVars\.(\w+)\.UpgradeValueBy\(\s*([^)]*?)\s*\)', code):
        upgrades.append({"var": name, "delta": num(delta) if re.fullmatch(LITERAL, delta) else None})
    for name, delta in re.findall(r'DynamicVars\["(\w+)"\]\.UpgradeValueBy\(\s*([^)]*?)\s*\)', code):
        upgrades.append({"var": name, "delta": num(delta) if re.fullmatch(LITERAL, delta) else None})
    for delta in re.findall(r"EnergyCost\.UpgradeBy\(\s*([^)]*?)\s*\)", code):
        upgrades.append({"var": "EnergyCost", "delta": num(delta) if re.fullmatch(LITERAL, delta) else None})
    if upgrades:
        numbers["upgrade"] = upgrades
    props = {}
    for name, level, hi, lo in re.findall(
            r"\b(\w+)\s*=>\s*AscensionHelper\.GetValueIfAscension\(\s*AscensionLevel\.(\w+)\s*,\s*([^,()]+?)\s*,\s*([^,()]+?)\s*\)\s*;", code):
        props[name] = {"base": num(lo) if re.fullmatch(LITERAL, lo) else None,
                       "ascension": {level: num(hi) if re.fullmatch(LITERAL, hi) else None}}
    for name, value in re.findall(r"\b(?:int|decimal)\s+(\w+)\s*=>\s*(-?\d+m?)\s*;", code):
        props.setdefault(name, {"base": num(value)})
    for name, value in re.findall(r"\bconst\s+(?:int|decimal)\s+(\w+)\s*=\s*(-?\d+m?)\s*;", code):
        props.setdefault(name, {"base": num(value)})
    if props:
        numbers["props"] = dict(sorted(props.items()))
    return numbers


def enum_attr(ent, prop, enum):
    m = re.search(rf"\b{prop}\s*=>\s*{enum}\.(\w+)", ent.code)
    return m.group(1) if m else None


def extract_attrs(ent):
    a = {}
    if ent.kind == "card":
        m = re.search(r"\bbase\(\s*-?\d+\s*,\s*CardType\.(\w+)\s*,\s*CardRarity\.(\w+)\s*,\s*TargetType\.(\w+)\s*\)", ent.code)
        a["card_type"] = m.group(1) if m else None
        a["target"] = m.group(3) if m else None
        kw = sorted(set(re.findall(r"CardKeyword\.(\w+)", ent.code.split("OnPlay")[0])))
        if kw:
            a["keywords"] = kw
        tags = sorted(set(re.findall(r"CardTag\.(\w+)", ent.code.split("OnPlay")[0])))
        if tags:
            a["tags"] = tags
        mp = re.search(r"MultiplayerConstraint\s*=>\s*CardMultiplayerConstraint\.(\w+)", ent.code)
        if mp:
            a["multiplayer_constraint"] = mp.group(1)
    elif ent.kind == "power":
        a["power_type"] = enum_attr(ent, "Type", "PowerType")
        a["stack_type"] = enum_attr(ent, "StackType", "PowerStackType")
    elif ent.kind == "potion":
        a["usage"] = enum_attr(ent, "Usage", "PotionUsage")
        a["target"] = enum_attr(ent, "TargetType", "TargetType")
    elif ent.kind in ("encounter",):
        a["room_type"] = enum_attr(ent, "RoomType", "RoomType")
        a["weak"] = bool(re.search(r"\bIsWeak\s*=>\s*true", ent.code))
        tags = sorted(set(re.findall(r"EncounterTag\.(\w+)", ent.code)))
        if tags:
            a["tags"] = tags
    elif ent.kind == "monster":
        branches = sorted({b for b in ("RandomBranchState", "ConditionalBranchState") if b in ent.blank})
        a["move_branches"] = branches
    return a


def extract_rarity(ent):
    if ent.kind == "card":
        m = re.search(r"\bbase\(\s*-?\d+\s*,\s*CardType\.\w+\s*,\s*CardRarity\.(\w+)", ent.code)
        return m.group(1) if m else None
    if ent.kind == "relic":
        return enum_attr(ent, "Rarity", "RelicRarity")
    if ent.kind == "potion":
        return enum_attr(ent, "Rarity", "PotionRarity")
    if ent.kind == "encounter":
        room = enum_attr(ent, "RoomType", "RoomType")
        if room is None:
            return None
        if room == "Monster":
            return "Weak" if re.search(r"\bIsWeak\s*=>\s*true", ent.code) else "Normal"
        return room
    return None


def extract_multiplayer_only(ent):
    if ent.kind == "card":
        return bool(re.search(r"MultiplayerConstraint\s*=>\s*CardMultiplayerConstraint\.MultiplayerOnly", ent.code))
    if re.search(r"return\s+[\w.]*Players\.Count\s*>\s*1\s*;", ent.code):
        return True
    if ent.kind == "power" and "NetService" in ent.blank:
        return True
    return False


def raw_refs(ent):
    """Every type name the entity's code mentions (generic args, typeof, new, is/as, casts, bases)."""
    refs = set()
    b = ent.blank
    for group in re.findall(r"<\s*([\w\s,.?]+?)\s*>", b):
        refs.update(t.strip().split(".")[-1] for t in group.split(","))
    refs.update(re.findall(r"\b(?:is|as)\s+(\w+)", b))
    refs.update(re.findall(r"\btypeof\(\s*(\w+)\s*\)", b))
    refs.update(re.findall(r"\bnew\s+(\w+)\s*[(\[]", b))
    refs.update(re.findall(r"\(\s*(\w+)\s*\)\s*(?:ModelDb|\w+\.)", b))
    refs.update(x.split(".")[-1] for x in ent.bases)
    refs.update(re.findall(r"\b([A-Z]\w*(?:Cmd|Factory))\s*\.", b))
    return refs


def extract_deps(ent, ents, support):
    refs = raw_refs(ent)
    todo, seen = sorted(refs), set()
    while todo:  # follow support classes (rest options, rewards) to the models they name
        r = todo.pop()
        if r in support and r not in seen:
            seen.add(r)
            new = raw_refs(support[r]) - refs
            refs |= new
            todo.extend(sorted(new))
    out = set()
    for r in refs:
        for kind in KIND_ORDER:
            if (kind, r) in ents and (kind, r) != (ent.kind, ent.id):
                out.add((kind, r))
    return out


def closure(root_keys, ents, support):
    deps = {}
    todo = sorted(root_keys)
    seen = set(todo)
    while todo:
        key = todo.pop()
        deps[key] = extract_deps(ents[key], ents, support)
        for d in sorted(deps[key]):
            if d not in seen:
                seen.add(d)
                todo.append(d)
    return deps


def inherit(ent, ents, field, cache):
    """Union of a per-entity set with that of its same-kind base classes."""
    out = set(cache[(ent.kind, ent.id)][field])
    for b in ent.bases:
        key = (ent.kind, b.split(".")[-1])
        if key in ents and key != (ent.kind, ent.id):
            out |= inherit(ents[key], ents, field, cache)
    return out


def tarjan(nodes, edges):
    index, low, stack, on, comps, idx = {}, {}, [], set(), [], [0]

    def visit(v):
        index[v] = low[v] = idx[0]
        idx[0] += 1
        stack.append(v)
        on.add(v)
        for w in sorted(edges[v]):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in on:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = stack.pop()
                on.discard(w)
                comp.append(w)
                if w == v:
                    break
            comps.append(sorted(comp))

    sys.setrecursionlimit(10000)
    for v in sorted(nodes):
        if v not in index:
            visit(v)
    return comps


def waves(nodes, edges):
    """Wave = longest dependency chain below the entity's strongly connected group."""
    comps = tarjan(nodes, edges)
    comp_of = {v: i for i, c in enumerate(comps) for v in c}
    cedges = {i: {comp_of[w] for v in c for w in edges[v] if comp_of[w] != i} for i, c in enumerate(comps)}
    depth = {}

    def d(i):
        if i not in depth:
            depth[i] = 1 + max((d(j) for j in cedges[i]), default=-1)
        return depth[i]

    return {v: d(comp_of[v]) for v in nodes}, comps, comp_of


CHOICE_CMDS = ("CardSelectCmd.", "RelicSelectCmd.")


def classify(rec, ent):
    """Tier rule (documented in the md). Uses only extracted fields."""
    cmds = rec["commands"]
    choice = any(c.startswith(CHOICE_CMDS) for c in cmds)
    osty = (any(c.startswith("OstyCmd.") for c in cmds) or
            bool(re.search(r"\bOsty\b|\bFromOsty\b|\.Osty\b|IsOstyMissing", ent.blank)))
    power_cmd = any(c.startswith("PowerCmd.") for c in cmds)
    hooks = len(rec["hooks"])
    rng = bool(rec["rng"])
    kind = rec["kind"]
    if kind == "card":
        if choice or osty:
            return "hard"
        if rec["attrs"].get("card_type") == "Power" or power_cmd or hooks or \
                re.search(r"\bDoom|\bSoul\b", ent.blank):
            return "medium"
        return "simple"
    if kind == "monster":
        if rec["attrs"]["move_branches"] or rng or "CreatureCmd.Add" in cmds or osty or hooks >= 3:
            return "hard"
        if power_cmd or hooks or any(c.startswith("CardPileCmd.") for c in cmds):
            return "medium"
        return "simple"
    if kind == "encounter":
        if rng or len(rec["deps"]) > 3:
            return "hard" if rng else "medium"
        return "simple"
    if kind == "event":
        if choice or osty or rng or hooks >= 3:
            return "hard"
        return "medium"
    # relic, potion, power, enchantment, affliction
    if choice or osty or hooks >= 4 or (rng and hooks):
        return "hard"
    if hooks or power_cmd or rng or len(cmds) > 1:
        return "medium"
    return "simple"


def null_fields(value, path=""):
    if value is None:
        yield path
    elif isinstance(value, dict):
        for k, v in value.items():
            yield from null_fields(v, f"{path}.{k}" if path else k)
    elif isinstance(value, list):
        for v in value:
            yield from null_fields(v, path)


def build(character, act):
    ents = load_entities()
    pools = pool_membership()
    gates = epoch_gates()
    hooks = hook_names()
    root_reasons = roots(pools, ents, character, act)
    deps = closure(root_reasons.keys(), ents, load_support())
    nodes = set(deps)
    cache = {}
    for key in nodes:
        e = ents[key]
        cache[key] = {"hooks": set(extract_hooks(e, hooks)), "commands": set(extract_commands(e)),
                      "rng": set(extract_rng(e))}
    # base classes are scanned too (they are deps), so inheritance is already in `nodes`.
    records = {}
    for key in sorted(nodes, key=lambda k: (KIND_ORDER.index(k[0]), k[1])):
        e = ents[key]
        rec = {
            "kind": e.kind,
            "id": e.id,
            "path": e.path,
            "rarity": extract_rarity(e),
            "pool": pools.get(key, []),
            "scope": root_reasons.get(key, ["dependency"]),
            "numbers": extract_numbers(e),
            "attrs": extract_attrs(e),
            "hooks": sorted(inherit(e, ents, "hooks", cache)),
            "commands": sorted(inherit(e, ents, "commands", cache)),
            "rng": sorted(inherit(e, ents, "rng", cache)),
            "deps": sorted(f"{k}:{i}" for k, i in deps[key]),
            "multiplayer_only": extract_multiplayer_only(e),
        }
        for attr in ("power_type", "stack_type"):
            base = e
            while rec["attrs"].get(attr, 1) is None:
                nxt = [(e.kind, b.split(".")[-1]) for b in base.bases if (e.kind, b.split(".")[-1]) in ents]
                if not nxt:
                    break
                base = ents[nxt[0]]
                rec["attrs"][attr] = extract_attrs(base).get(attr)
        if key in gates:
            rec["attrs"]["epoch"] = gates[key]
        if e.abstract:
            rec["attrs"]["abstract"] = True
        rec["tier"] = classify(rec, e)
        records[key] = rec
    wave, comps, comp_of = waves(nodes, deps)
    for key, rec in records.items():
        rec["wave"] = wave[key]
        rec["cycle"] = [f"{k}:{i}" for k, i in comps[comp_of[key]]] if len(comps[comp_of[key]]) > 1 else None
    return records, hooks


def counts_by(records, field):
    out = defaultdict(lambda: defaultdict(int))
    for r in records.values():
        out[r["kind"]][r[field]] += 1
    return out


def render_md(records, hooks, character, act):
    L = []
    kinds = [k for k in KIND_ORDER if any(r["kind"] == k for r in records.values())]
    slug = f"{character.lower()}-{act.lower()}"
    L.append(f"# {character} {act} content inventory (generated)")
    L.append("")
    L.append(f"Generated by `python3 tools/inventory.py ref --character {character} --act {act.lower()}`; "
             f"do not edit. Data: `docs/inventory/{slug}.json`.")
    L.append("")
    if act in SHARED_ANCIENT_ACTS:
        ancients = "the act's ancients and the shared ancient (Darv)"
        out_of_scope = "Act 1 (Overgrowth, Underdocks) and the other of Hive and Glory are out of scope."
    else:
        ancients = "the act's ancient (Neow)"
        out_of_scope = "The other act 1, Hive and Glory are out of scope."
    L.append(f"Scope: {character} card/relic/potion pools, the shared relic and potion pools, "
             f"{act} encounters and their monsters (incl. summons), {act} and "
             f"shared events, {ancients} with every relic it offers, plus everything they "
             "reference transitively (powers, token/status/curse cards, enchantments, afflictions...), "
             "also through the support code they hand control to (rest-site options, rewards, `*Cmd` "
             "commands, relic factory: " + ", ".join(SUPPORT_DIRS) + ") so that e.g. the Hatch option "
             "reaches `Byrdpip`. The "
             "Colorless card pool is in scope (the shop sells it). " + out_of_scope + " Other-character cards appear only when something in scope references "
             "them by id; cards, relics and potions created by pool lookups (`GetUnlockedCards` etc.) are "
             "not tracked.")
    L.append("")
    L.append("## Counts per kind and tier")
    L.append("")
    L.append("| kind | simple | medium | hard | total | multiplayer-only |")
    L.append("|---|---:|---:|---:|---:|---:|")
    tot = defaultdict(int)
    for k in kinds:
        rs = [r for r in records.values() if r["kind"] == k]
        row = [sum(1 for r in rs if r["tier"] == t) for t in ("simple", "medium", "hard")]
        mp = sum(1 for r in rs if r["multiplayer_only"])
        L.append(f"| {k} | {row[0]} | {row[1]} | {row[2]} | {len(rs)} | {mp} |")
        for t, n in zip(("simple", "medium", "hard"), row):
            tot[t] += n
        tot["all"] += len(rs)
        tot["mp"] += mp
    L.append(f"| **all** | {tot['simple']} | {tot['medium']} | {tot['hard']} | {tot['all']} | {tot['mp']} |")
    L.append("")
    L.append("### Tier rule")
    L.append("")
    L.append("Computed from the extracted fields only (`choice` = any `CardSelectCmd`/`RelicSelectCmd` command; "
             "`osty` = any `OstyCmd` command or a mention of Osty; `hooks` counts include inherited ones).")
    L.append("")
    L.append("* card: **hard** if choice or osty; **medium** if Power card, any `PowerCmd`, any hook override or a Doom/Soul mention; else **simple**.")
    L.append("* monster: **hard** if it has a `RandomBranchState`/`ConditionalBranchState`, uses rng, adds creatures (`CreatureCmd.Add`), involves Osty or overrides 3+ hooks; "
             "**medium** if it applies powers, overrides a hook or moves cards; else **simple**.")
    L.append("* encounter: **hard** if it uses rng (random composition); **medium** if it references more than 3 entities; else **simple**.")
    L.append("* event: **hard** if choice, osty, rng or 3+ hooks; else **medium** (every event has a decision point).")
    L.append("* relic, potion, power, enchantment, affliction: **hard** if choice, osty, 4+ hooks, or rng together with a hook; "
             "**medium** if any hook, `PowerCmd`, rng or 2+ distinct commands; else **simple**.")
    L.append("")
    L.append("## Hooks overridden (union, with usage count)")
    L.append("")
    L.append("Hook names are the virtual methods of `AbstractModel` plus the `Before*/After*/Modify*/Should*/On*/Try*` "
             "lifecycle methods of the kind base classes (entry points `OnPlay`/`OnUse`/`OnUpgrade` excluded). "
             "Count = number of in-scope entities overriding it (own or inherited).")
    L.append("")
    usage = defaultdict(int)
    for r in records.values():
        for h in r["hooks"]:
            usage[h] += 1
    L.append("| hook | entities |")
    L.append("|---|---:|")
    for h, n in sorted(usage.items(), key=lambda kv: (-kv[1], kv[0])):
        L.append(f"| {h} | {n} |")
    L.append("")
    L.append("## Null fields (regex could not decide)")
    L.append("")
    leaf = defaultdict(int)
    affected = set()
    for key, r in records.items():
        view = {k: v for k, v in r.items() if k != "cycle"}
        if r["kind"] not in RARITY_KINDS:
            del view["rarity"]
        for field in null_fields(view):
            leaf[field] += 1
            affected.add(key)
    L.append(f"Total null leaves: {sum(leaf.values())} across {len(affected)} entities.")
    L.append("")
    L.append("| field | null count |")
    L.append("|---|---:|")
    for f, n in sorted(leaf.items()):
        L.append(f"| {f} | {n} |")
    L.append("")
    L.append("`rarity` is only counted for kinds that have one (card, relic, potion, encounter; null in the JSON "
             "for the other kinds). `numbers.vars[].value`/`upgrade[].delta` are null when the argument is not a "
             "literal (e.g. `CalculatedDamageVar`); `power_type`/`stack_type` are null when the power computes them "
             "at runtime; `cycle` is null when the entity is in no dependency cycle (by design, not counted).")
    L.append("")
    L.append("## Dependency-ordered porting list (leaves first)")
    L.append("")
    L.append("Wave 0 has no in-scope dependencies; wave N depends only on waves below N. Entities in a "
             "dependency cycle share a wave and are marked `cycle` in the JSON. Within a wave: kind order, then id.")
    L.append("")
    by_wave = defaultdict(lambda: defaultdict(list))
    for r in records.values():
        by_wave[r["wave"]][r["kind"]].append(r["id"])
    for w in sorted(by_wave):
        size = sum(len(v) for v in by_wave[w].values())
        L.append(f"### Wave {w} ({size})")
        for k in kinds:
            if by_wave[w][k]:
                L.append(f"* {k} ({len(by_wave[w][k])}): " + ", ".join(sorted(by_wave[w][k])))
        L.append("")
    return "\n".join(L).rstrip() + "\n"


def write(character, act_key):
    act = ACT_FILES[act_key]
    records, hooks = build(character, act)
    ordered = sorted(records.values(), key=lambda r: (r["wave"], KIND_ORDER.index(r["kind"]), r["id"]))
    doc = {
        "scope": {"character": character, "acts": [act], "root_pools": root_pools(character)},
        "entities": [records[(r["kind"], r["id"])] for r in
                     sorted(records.values(), key=lambda r: (KIND_ORDER.index(r["kind"]), r["id"]))],
        "porting_order": [f"{r['kind']}:{r['id']}" for r in ordered],
    }
    OUT.mkdir(parents=True, exist_ok=True)
    slug = f"{character.lower()}-{act_key}"
    (OUT / f"{slug}.json").write_text(json.dumps(doc, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    (OUT / f"{slug}.md").write_text(render_md(records, hooks, character, act), encoding="utf-8")
    print(f"{character} {act}: {len(records)} entities -> {OUT / slug}.json")


def flag(name):
    args = sys.argv[1:]
    return args[args.index(name) + 1] if name in args else None


def main():
    if "--all" in sys.argv:
        pairs = [(c, a) for c in CHARACTERS for a in ACT_FILES]
    else:
        character = flag("--character") or "Necrobinder"
        act = (flag("--act") or "overgrowth").lower()
        if character not in CHARACTERS or act not in ACT_FILES:
            sys.exit(f"unknown --character {character} or --act {act}")
        pairs = [(character, act)]
    for character, act in pairs:
        write(character, act)


if __name__ == "__main__":
    main()
