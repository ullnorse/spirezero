"""WEB51 outcome checks: every choice does what was picked. Each scenario starts a fresh run at a fixed seed, reaches a screen (by
real input; `sts2.debug` sets up the screens a seed will not reach), picks a NON-FIRST option by real input (touch on the
phone, mouse on the desktop) and reads the result from the game's own state (`sts2.view()`: gold, relics, potions, enemy HP,
and the deck, piles and hand sheets opened by their buttons). A pick that lands on the wrong thing fails by name.
Used by realtest.py: `run(r)` with r a Run. Env SCEN=a,b runs only those scenarios."""
import json, os, re, time


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

URL = os.environ.get("URL", "http://127.0.0.1:8765/")
QUICK = bool(os.environ.get("QUICK"))


def plain(r):
    return "".join(s["text"] for s in (r or []))


def state(b):
    s = b.get("state")
    return s if isinstance(s, str) else next(iter(s))


def rows_of(v):
    return [r for b in v["screen"]["blocks"] if "List" in b for r in b["List"]["rows"]]


def enemies(v):
    return [b["Enemy"] for b in v["screen"]["blocks"] if "Enemy" in b]


def price(row):
    m = re.search(r"(\d+)", plain(row.get("right")))
    return int(m.group(1)) if m else None


def relic_name(row):
    return re.sub(r"^[^\w]+", "", plain(row["label"])).replace("Take the relic:", "").strip()


class Skip(Exception):
    pass


class O:
    def __init__(self, r):
        self.r = r
        self.p = r.p
        self.tag = r.name

    # -- plumbing -----------------------------------------------------------------------------
    def check(self, name, ok, detail=""):
        return self.r.check(name, ok, detail)

    def view(self):
        return self.r.view()

    def top(self):
        return self.view()["screen"]["top"]

    def gold(self):
        return self.top()["gold"]

    def relics(self):
        return [x["name"] for x in self.top()["relics"]]

    def potions(self):
        return [x["name"] for x in self.top()["potions"]]

    def settle(self, s=0.5):
        self.p.settle(s)

    def tap(self, sel, wait=0.5):
        ok = self.r.click_sel(sel)
        self.settle(wait)
        return ok

    def tap_js(self, expr, wait=0.5):
        """Real tap in the middle of the element `expr` (a JS expression) names, scrolled clear of the bars."""
        pos = self.p.js(f"(() => {{ const e = {expr}; if (!e) return null; if (!e.closest('#dock, #top')) e.scrollIntoView({{block: 'center'}}); const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }})()")
        if not pos:
            return False
        self.p.click(*pos)
        self.settle(wait)
        return True

    def tap_bid(self, bid, scope="#screen", confirm=True):
        """Taps the control of button `bid`; when the page then asks (Tap again), taps again."""
        ok = self.tap(f'{scope} [data-bid="{bid}"]')
        if ok and confirm:
            row = next((x for x in rows_of(self.view()) if x["id"] == bid), None)
            if row and state(row) == "Picked" and row.get("confirm"):
                ok = self.tap(f'{scope} [data-bid="{bid}"]')
        return ok

    def preview_open(self):
        return bool(self.p.js("!!document.querySelector('#sheet:not([hidden]) .pickview')"))

    def pick_card(self, bid, label, wait=1.0):
        """WEB65: a deck pick's last card: the tap opens the preview and changes nothing; Confirm applies it."""
        # read from the engine's view: the deck sheet's button sits under the preview's backdrop
        size = lambda: self.view()["run"]["deck"]
        n0, g0 = size(), self.gold()
        self.tap(f'#screen [data-bid="{bid}"]', 0.6)
        shown = self.preview_open()
        same = size() == n0 and self.gold() == g0 and self.view()["decision"] == "card-select"
        self.check(f"{label}: the tap previews (Confirm/Cancel), nothing applied yet", shown and same, f"preview {shown}; unchanged {same}")
        if shown:
            self.tap("#sheet .pickgo", wait)

    def deck(self):
        return self.r.deck_counts()

    def pile(self, nth):
        """A pile's contents (draw 1, discard 2, exhaust 3) from its sheet, opened by the dock's button."""
        if not self.tap(f"#dock .pile:nth-of-type({nth})", 0.5):
            return None
        got = self.p.js("""Object.fromEntries([...document.querySelectorAll('#sheet .cards .card')].map((c) =>
            [c.querySelector('.g-title, .cname').textContent.trim(), Number(c.querySelector('.count')?.textContent.replace('×', '') ?? 1)]))""")
        self.r.close_sheet()
        return got

    def delta(self, before, after):
        keys = set(before) | set(after)
        return {k: after.get(k, 0) - before.get(k, 0) for k in keys if after.get(k, 0) != before.get(k, 0)}

    # -- runs ---------------------------------------------------------------------------------
    def setup(self, char, seed):
        p = self.p
        p.open(URL)
        p.js("localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full'})); 0")
        p.open(URL)
        p.settle(1.5)
        p.js(f"sts2.start({char}, {json.dumps(seed)}, 0); 0")
        p.settle(1.0)
        if not self.view():
            raise Skip("run did not start")

    def to_map(self, ancient=0):
        """Neow's pick (the tile `ancient`), then whatever it rewards, by real input, to the map."""
        for _ in range(14):
            v = self.view()
            d = v["decision"]
            if d == "map":
                return v
            if d == "ancient":
                if not self.tap(f'#screen .row[data-bid="{rows_of(v)[ancient]["id"]}"]', 0.8):
                    self.tap_bid(rows_of(v)[ancient]["id"])
            elif d == "rewards" and self.p.js("!!document.querySelector('.peekreward .card')"):
                self.tap(".peekreward .card:last-child", 0.6)
            elif d == "card-select":
                self.tap("#screen .card[data-bid]:not(:disabled)", 0.8)
                if self.preview_open():
                    self.tap("#sheet .pickgo", 0.8)
            elif not self.tap("#screen .row:not(:disabled), #screen .ware:not(:disabled)", 0.5):
                self.tap("#dock .primary", 0.5)
        raise Skip("never reached the map")

    def debug(self, spec):
        err = self.p.js(f"sts2.debug({json.dumps(json.dumps(spec))})")
        self.settle(0.8)
        if err:
            raise Skip(f"debug {spec}: {err}")

    def enter_fight(self, spec):
        self.debug(spec)
        v = self.view()
        if v["decision"] == "map":
            node = rows_of(v)
            if not self.tap("g.node.reach", 1.5) and not self.tap_bid(node[0]["id"]):
                raise Skip("no map node")
        for _ in range(8):
            v = self.view()
            if v["fight"]:
                break
            self.settle(0.5)
        if not self.view()["fight"]:
            raise Skip("no fight")
        self.settle(1.0)
        return self.view()

    # -- fight helpers ------------------------------------------------------------------------
    def hand(self, v=None):
        return [r for r in rows_of(v or self.view()) if r.get("card")]

    def dmg(self, row):
        m = re.search(r"Deal (\d+) damage", plain((row.get("detail") or [[]])[0]))
        return int(m.group(1)) if m else None

    def enemy_hp(self, v=None):
        return {e["id"]: e["hp"] for e in enemies(v or self.view())}

    def enemy_pos(self, eid):
        return self.p.js(f"(() => {{ const e = document.querySelector('#screen .enemy[data-id=\"{eid}\"]'); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }})()")

    def card_pos(self, bid):
        return self.p.js(f"(() => {{ const e = document.querySelector('#screen .hand .card[data-bid=\"{bid}\"]'); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }})()")


# ---- scenarios -------------------------------------------------------------------------------
SCENARIOS = []


def scenario(f):
    SCENARIOS.append(f)
    return f


@scenario
def neow_pick(o):
    """The ancient: the relic gained is the one tapped (not the first), and its choices are one list with no tile row."""
    for how, idx in (("row", 2), ("row", 1)):
        o.setup(4, "WEBTEST1")
        v = o.view()
        picks = rows_of(v)
        if len(picks) < 3 or v["decision"] != "ancient":
            raise Skip("no ancient")
        want = relic_name(picks[idx])
        before = o.relics()
        o.check("ancient choices only as rows", not o.p.js("!!document.querySelector('#screen .showcase')"))
        ok = o.tap(f'#screen .row[data-bid="{picks[idx]["id"]}"]', 1.0)
        got = [x for x in o.relics() if x not in before]
        o.check(f"ancient relic as tapped ({how} {idx + 1})", ok and want in got, f"tapped {want}, relics gained {got}")


@scenario
def neow_info(o):
    """WEB57: a Neow row's text is on the row, so its i and hold read only what the row does not say (the game's
    glossary); a row with nothing more has no i, and holding it opens nothing."""
    o.setup(4, "WEBTEST1")
    v = o.view()
    if v["decision"] != "ancient":
        raise Skip("no ancient")
    p = o.p
    rows = p.js("""[...document.querySelectorAll('#screen .list .row')].map((e) => ({bid: e.dataset.bid,
        sub: [...e.querySelectorAll('.sub')].map((s) => s.textContent).join(' ').replace(/\\s+/g, ' ').trim(), info: !!e.querySelector('.info')}))""") or []
    with_info = [r for r in rows if r["info"]]
    without = [r for r in rows if not r["info"]]
    o.check("neow: some rows have an i and some none (seed WEBTEST1)", with_info and without, f"rows {rows}")
    for r in with_info:
        o.tap(f'#screen .row[data-bid="{r["bid"]}"] .info', 0.6)
        sheet = p.js("document.querySelector('#sheet:not([hidden]) .sheetbody')?.innerText.replace(/\\s+/g, ' ').trim()")
        terms = p.js("document.querySelectorAll('#sheet:not([hidden]) .term').length")
        o.check("neow: a row's sheet does not repeat the row's text", sheet is not None and r["sub"] and r["sub"] not in sheet and terms > 0, f"row {r['sub']!r}; sheet {sheet!r}")
        o.r.close_sheet()
    for r in without:
        pos = p.js(f"(() => {{ const e = document.querySelector('#screen .row[data-bid=\"{r['bid']}\"] .lead-ic'); const b = e.getBoundingClientRect(); return [b.left + b.width / 2, b.top + b.height / 2]; }})()")
        if p.touch:
            p.mx, p.my = pos
        else:
            p.move(*pos, 3)
        p.down(); time.sleep(0.6)
        opened = p.js("!!document.querySelector('#sheet:not([hidden])')")
        p.up()  # the end of a hold is not a tap
        o.settle(0.5)
        o.check("neow: a row with nothing more has no i and its hold opens nothing", not opened and o.view()["decision"] == "ancient", f"row {r['sub']!r}: sheet {opened}, decision {o.view()['decision']}")


@scenario
def reward_rows(o):
    """Neow's Lost Coffer: a card reward and a potion. The potion row gives that potion; the last card tile that card."""
    o.setup(4, "WEBTEST1")
    v = o.view()
    idx = next((i for i, r in enumerate(rows_of(v)) if "Lost Coffer" in plain(r["label"])), None)
    if idx is None:
        raise Skip("no Lost Coffer")
    o.tap(f'#screen .row[data-bid="{rows_of(v)[idx]["id"]}"]', 1.0)
    v = o.view()
    if v["decision"] != "rewards":
        raise Skip(f"decision {v['decision']}")
    pot = next((r for r in rows_of(v) if r.get("info", {}) and (r["info"] or {}).get("Potion")), None)
    if pot:
        want = re.sub(r"^[^\w]+", "", plain(pot["label"])).strip()
        before = o.potions()
        o.tap_bid(pot["id"])
        got = [x for x in o.potions() if x and x not in before]
        o.check("potion reward as tapped", want in got, f"tapped {want}, belt {before} -> {o.potions()}")
    names = o.p.js("[...document.querySelectorAll('.peekreward .card')].map((c) => c.querySelector('.g-title, .cname').textContent.trim())") or []
    if len(set(names)) >= 2:
        before = o.deck()
        n = len(names)
        o.tap(f".peekreward .card:nth-child({n})", 0.8)
        d = o.delta(before, o.deck())
        o.check("card reward as tapped (last tile)", d == {names[-1]: 1}, f"tapped {names[-1]} of {names}; deck change {d}")


@scenario
def treasure(o):
    """A treasure's relic: the relic gained is the one shown on the chest."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"room": "treasure"})
    v = o.view()
    rel = [r for r in rows_of(v) if "relic" in plain(r["label"]).lower() or (r.get("info") or {}).get("Relic")]
    if v["decision"] != "treasure" or not rel:
        raise Skip(f"decision {v['decision']}")
    o.check("treasure: the relic shows once, as its row", not o.p.js("!!document.querySelector('#screen .showcase')"))
    o.r.shot("treasure")
    pick = rel[-1]
    want = relic_name(pick)
    before = o.relics()
    o.tap_bid(pick["id"])
    got = [x for x in o.relics() if x not in before]
    o.check("treasure relic as tapped", want in got, f"offered {want}, relics gained {got}")


@scenario
def event_picture(o):
    """An event's (or the treasure's) picture: shown in its fixed box, the choices on screen without scrolling, and nothing
    moves when the picture loads."""
    o.setup(4, "WEBTEST1")
    o.r.shot("neow")
    probe = """(() => { const q = (s) => document.querySelector(s); const box = q('#screen .picstory, #screen .picture'), img = q('#screen .picstory img, #screen .picture img');
      const rows = [...document.querySelectorAll('#screen .list .row')], last = rows[rows.length - 1], first = rows[0], r = box && box.getBoundingClientRect();
      const gap = q('#screen .picgap'), list = q('#screen .list'), lb = last && Math.round(last.getBoundingClientRect().bottom), dock = Math.round(q('#dock').getBoundingClientRect().top);
      // where the choices would end with the art ceded (the title and story alone): what the page did before pictures
      const bare = r && list ? Math.round(r.bottom - (gap ? gap.offsetHeight : 0) + 12 + list.getBoundingClientRect().height) : null;
      return { pic: !!box, card: !!q('#screen .picstory'), done: !!img && img.complete && img.naturalWidth > 0, pt: r && Math.round(r.top), ph: r && Math.round(r.height), ft: first && Math.round(first.getBoundingClientRect().top),
               gap: gap ? gap.offsetHeight : null, lb, h: dock, bare }; })()"""
    tour = [x for x in os.environ.get("EVENTS", "").split(",") if x] or ["REFLECTIONS", "PUNCH_OFF", "SYMBIOTE", "SAPPHIRE_SEED", "TEA_MASTER", "BUGSLAYER"]
    best = 0
    for i, room in enumerate([f"event:{e}" for e in tour] + ["treasure"]):
        if i:
            o.setup(4, "WEBTEST1")
        o.to_map(2)
        o.debug({"room": room})
        o.settle(1.2)
        after = o.p.js(probe)
        # load it again (a new address): until it arrives the box must be where and as big as it was
        o.p.js("(() => { const i = document.querySelector('#screen .picstory img, #screen .picture img'); if (i) i.src = i.src.split('?')[0] + '?' + Date.now(); return 0; })()")
        at = o.p.js(probe)
        o.settle(1.0)
        after = o.p.js(probe)
        name = room.split(":")[-1].lower()
        if not after["pic"]:
            raise Skip(f"no picture for {room} (art not imported?)")
        o.check(f"{name}: the picture loads", after["done"], str(after))
        o.check(f"{name}: nothing moves when the picture loads", (at["pt"], at["ph"], at["ft"], at["lb"]) == (after["pt"], after["ph"], after["ft"], after["lb"]), f"{at} -> {after}")
        # the art gives way first: the choices end above the dock whenever the title and story alone leave them there
        fits = after["lb"] is not None and after["lb"] <= after["h"] + 2
        bare_fits = after["bare"] is not None and after["bare"] <= after["h"] + 2
        o.check(f"{name}: the choices are on screen without scrolling (as before the picture)", fits or not bare_fits, str(after))
        if after["card"]:
            best = max(best, after["gap"])
        print(f"event_picture {name}: art clear {after['gap']}px, choices end {after['lb']} of {after['h']}, text alone {after['bare']}", flush=True)
        o.r.shot(name)
    if o.r.touch and o.p.js("innerWidth") >= 380:
        o.check("a short event leaves 140px of clear art on a phone", best >= 140, f"most clear art {best}px")


@scenario
def shop_buy(o):
    """Shop: a card, a relic and a potion, none of them the first: that item gained, its shown price gone from gold."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 900, "room": "shop"})
    if o.view()["decision"] != "shop":
        raise Skip("no shop")
    for kind in ("card", "relic", "potion"):
        v = o.view()
        shop = [r for r in rows_of(v) if state(r) != "Disabled"]
        if kind == "card":
            items = [r for r in shop if r.get("card") and "Remove" not in plain(r["label"])]
        elif kind == "relic":
            items = [r for r in shop if (r.get("info") or {}).get("Relic")]
        else:
            items = [r for r in shop if (r.get("info") or {}).get("Potion")]
        if len(items) < 2:
            o.check(f"shop buys a {kind} (not the first)", False, f"only {len(items)} on offer")
            continue
        pick = items[1]
        cost = price(pick)
        g0, d0, r0, p0 = o.gold(), o.deck(), o.relics(), o.potions()
        name = pick["card"]["name"] if kind == "card" else re.sub(r"^[^\w]+", "", plain(pick["label"])).strip()
        o.tap_bid(pick["id"])
        g1 = o.gold()
        if kind == "card":
            ok = o.delta(d0, o.deck()) == {name: 1}
            gain = f"deck change {o.delta(d0, o.deck())}"
        elif kind == "relic":
            gained = [x for x in o.relics() if x not in r0]
            ok, gain = gained == [name], f"relics gained {gained}"
        else:
            gained = [x for x in o.potions() if x and x not in p0]
            ok, gain = gained == [name], f"potions gained {gained}"
        o.check(f"shop buys the {kind} tapped (2nd)", ok and g0 - g1 == cost, f"tapped {name} for {cost}; {gain}; gold {g0} -> {g1}")


@scenario
def shop_remove(o):
    """Card removal at the shop: the card tapped is gone and the price paid; Back leaves everything as it was."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 900, "room": "shop"})
    v = o.view()
    rem = next((r for r in rows_of(v) if plain(r["label"]).startswith("Remove")), None)
    if not rem:
        raise Skip("no removal row")
    cost = price(rem)
    g0, d0 = o.gold(), o.deck()
    # Back out of the grid first: nothing spent, nothing removed
    o.tap_bid(rem["id"])
    if o.view()["decision"] != "card-select":
        raise Skip("no grid")
    if not o.tap("#dock .primary.back, #dock .primary", 0.8):
        o.p.key("Escape")
    o.check("shop removal: Back spends nothing", o.gold() == g0 and o.deck() == d0 and o.view()["decision"] == "shop", f"gold {g0} -> {o.gold()}; deck change {o.delta(d0, o.deck())}")
    rem = next(r for r in rows_of(o.view()) if plain(r["label"]).startswith("Remove"))
    o.tap_bid(rem["id"])
    grid = [r for r in rows_of(o.view()) if r.get("card")]
    if len(grid) < 2:
        raise Skip("grid too small")
    pick = grid[1]
    name = pick["card"]["name"]
    # Cancel in the preview: nothing spent, the grid still up
    o.tap(f'#screen [data-bid="{pick["id"]}"]', 0.6)
    shown = o.preview_open()
    o.tap("#sheet .pickback", 0.6)
    o.check("shop removal: Cancel in the preview spends nothing", shown and not o.preview_open() and o.gold() == g0 and o.deck() == d0 and o.view()["decision"] == "card-select",
            f"preview {shown}; gold {g0} -> {o.gold()}; decision {o.view()['decision']}")
    o.pick_card(pick["id"], "shop removal")
    d = o.delta(d0, o.deck())
    o.check("shop removes the card tapped", d == {name: -1} and o.gold() == g0 - cost, f"tapped {name}; deck change {d}; gold {g0} -> {o.gold()} (price {cost})")


@scenario
def shop_back(o):
    """A shop item asked about (Tap again) and then not confirmed spends nothing."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 900, "room": "shop"})
    v = o.view()
    items = [r for r in rows_of(v) if (r.get("info") or {}).get("Relic")]
    if len(items) < 2:
        raise Skip("no relics on offer")
    g0, r0 = o.gold(), o.relics()
    o.tap(f'#screen [data-bid="{items[1]["id"]}"]')  # asks; not confirmed
    o.tap("#dock .primary", 0.6)  # Leave the shop
    o.check("shop: asked but not confirmed spends nothing", o.gold() == g0 and o.relics() == r0, f"gold {g0} -> {o.gold()}; relics {o.relics()}")


@scenario
def event_tea(o):
    """Tea Master's second tea: that tea's relic, and its price from gold (not the first tea)."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 300, "room": "event:TEA_MASTER"})
    v = o.view()
    opts = rows_of(v)
    if v["decision"] != "event" or len(opts) < 3:
        raise Skip("no event")
    pick = opts[1]
    want = plain(pick["label"]).strip()
    cost = int(re.search(r"Pay (\d+) Gold", plain(pick["detail"][0])).group(1))
    g0, r0 = o.gold(), o.relics()
    o.tap_bid(pick["id"], confirm=True)
    got = [x for x in o.relics() if x not in r0]
    o.check("event option as tapped (Tea Master, 2nd)", want in got and o.gold() == g0 - cost, f"tapped {want} ({cost}); relics gained {got}; gold {g0} -> {o.gold()}")


@scenario
def event_pick(o):
    """Events with a deck pick: the option tapped runs, and the card tapped is the one it takes (not the first card)."""
    for ev, opt, gold_cost, note in (("ZEN_WEAVER", 1, 125, "remove"), ("WOOD_CARVINGS", 2, 0, "transform")):
        o.setup(4, "WEBTEST1")
        o.to_map(2)
        o.debug({"gold": 300, "room": f"event:{ev}"})
        v = o.view()
        opts = rows_of(v)
        if v["decision"] != "event" or len(opts) <= opt:
            raise Skip(f"{ev} not an event")
        g0, d0 = o.gold(), o.deck()
        o.tap_bid(opts[opt]["id"])
        v = o.view()
        grid = [r for r in rows_of(v) if r.get("card")]
        if v["decision"] != "card-select" or len(grid) < 2:
            o.check(f"event {ev}: option {opt + 1} opens a card pick", False, f"decision {v['decision']}")
            continue
        pick = grid[1]
        name = pick["card"]["name"]
        o.pick_card(pick["id"], f"event {ev}")
        d = o.delta(d0, o.deck())
        if note == "remove":
            ok = d == {name: -1} and o.gold() == g0 - gold_cost
        else:
            ok = d.get(name) == -1 and sorted(d.values()) == [-1, 1] and o.gold() == g0
        o.check(f"event deck pick takes the card tapped ({ev}, option {opt + 1})", ok, f"tapped {name}; deck change {d}; gold {g0} -> {o.gold()}")
        if note == "remove":
            # a Back from a pick is nothing spent
            pass


@scenario
def event_two_cards(o):
    """A grid that takes two cards (Zen Weaver, remove 2): tap a card, tap another, tap the first again (put back), tap a third:
    the cards left picked are the ones removed."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 400, "room": "event:ZEN_WEAVER"})
    v = o.view()
    opts = rows_of(v)
    if v["decision"] != "event" or len(opts) < 3:
        raise Skip("no event")
    g0, d0 = o.gold(), o.deck()
    o.tap_bid(opts[2]["id"])
    grid = [r for r in rows_of(o.view()) if r.get("card")]
    if len(grid) < 4:
        raise Skip("grid too small")
    # the grid stacks equal cards and re-numbers after each pick: find each card by its name and state afresh
    names = [r["card"]["name"] for r in grid]
    a, b, c = names[2], names[3], names[4]

    def tap_card(name, picked):
        row = next((r for r in rows_of(o.view()) if r.get("card") and r["card"]["name"] == name and (state(r) == "Picked") == picked), None)
        if row:
            o.tap(f'#screen [data-bid="{row["id"]}"]', 0.6)
        return row

    def picked_now():
        return sorted(r["card"]["name"] for r in rows_of(o.view()) if r.get("card") and state(r) == "Picked")
    tap_card(a, False)
    picked = picked_now()
    tap_card(a, True)  # again: put it back
    after = picked_now()
    o.check("multi-pick grid: tap again puts a card back", picked == [a] and after == [], f"picked {picked}, after tap-again {after}")
    tap_card(b, False)
    row = next((r for r in rows_of(o.view()) if r.get("card") and r["card"]["name"] == c and state(r) != "Picked"), None)
    if row:
        o.pick_card(row["id"], "multi-pick grid's last card")
    d = o.delta(d0, o.deck())
    want = {}
    for n in (b, c):
        want[n] = want.get(n, 0) - 1
    o.check("multi-pick grid: the cards left picked are removed", d == want and o.gold() == g0 - 250, f"left picked {want}; deck change {d}; gold {g0} -> {o.gold()}")


def fight_card_checks(o, enc, tap_to, drag_to, label):
    """Plays attacks by tap and by drag with two or more enemies; the card touched is the one played and the enemy aimed at takes the hit."""
    from collections import Counter
    spec = {"hp": 300, "encounter": enc, "cards": ["STRIKE_IRONCLAD", "BASH", "ANGER", "STRIKE_IRONCLAD", "BASH", "ANGER"]}
    v = o.enter_fight(spec)
    if len([e for e in enemies(v) if e["alive"]]) < 2:
        raise Skip("one enemy")
    stray = o.p.js("document.querySelectorAll('#screen .enemy .ghost, #screen .enemy .hprow .hit').length")
    o.check(f"fight {label}: no damage shown on enemies before a card is chosen", not stray, f"{stray} marks")
    played = None
    for mode, to in (("tap", tap_to), ("drag", drag_to)):
        v = o.view()
        ens = [e for e in enemies(v) if e["alive"]]
        to = min(to, len(ens) - 1)
        att = [r for r in o.hand(v) if state(r) == "Enabled" and o.dmg(r) and r["card"]["kind"] == "Attack"]
        # not the first card of the hand, and (when the hand allows) not the card the other mode played
        pool = [r for r in att[1:] if r["card"]["name"] != played] or att[1:] or att
        if not pool:
            o.check(f"fight {label} {mode}: an attack to play", False, "no attack in hand")
            continue
        row = pool[-1]
        name, n = row["card"]["name"], o.dmg(row)
        played = name
        hp0, hand0 = o.enemy_hp(v), Counter(r["card"]["name"] for r in o.hand(v))
        target = ens[to]["id"]
        pos = o.card_pos(row["id"])
        if mode == "tap":
            o.p.click(*pos)
            o.settle(0.4)
            ok = o.tap_js(f"document.querySelector('#screen .enemy[data-id=\"{target}\"]')", 0.9)
        else:
            tp = o.enemy_pos(target)
            o.p.drag(pos[0], pos[1], tp[0], tp[1])
            o.settle(0.9)
            ok = True
        v2 = o.view()
        hp1 = o.enemy_hp(v2)
        hit = {k: hp0[k] - hp1.get(k, 0) for k in hp0 if hp0[k] != hp1.get(k, 0)}
        hand1 = Counter(r["card"]["name"] for r in o.hand(v2))
        want = hand0.copy()
        want[name] -= 1
        want += Counter()
        o.check(f"fight {label} {mode}: {name} on enemy {to + 1} of {len(ens)}", ok and hit == {target: n} and hand1 == want,
                f"played {name} for {n} on {target}; enemy HP lost {hit}; hand {dict(hand0)} -> {dict(hand1)}")
        o.r.verify(f"{label} {mode}")


@scenario
def fight_two(o):
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    fight_card_checks(o, "CULTISTS_NORMAL", 1, 1, "2 enemies")


@scenario
def fight_four(o):
    o.setup(0, "WEBTEST3")
    o.to_map(0)
    fight_card_checks(o, "SLIMES_NORMAL", 2, 3, "4 enemies")


def potion_slot_js(slot):
    return f"[...document.querySelectorAll('#top .pot')][{slot}]"


@scenario
def crowded_fit(o):
    """WEB53: four enemies and ten cards (two Swift Potions drunk): every enemy sits below the top bar and clear of the hand and
    the dock, and the page does not scroll (cmp_view checks the rectangles)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "SLIMES_NORMAL", "potions": ["SWIFT_POTION", "SWIFT_POTION"], "cards": ["STRIKE_IRONCLAD"] * 10})
    for slot in (0, 1):
        potion_do(o, slot, "Use")
    o.settle(1.0)
    v = o.view()
    en = len(enemies(v))
    n = len(o.hand(v))
    o.check(f"crowded fight: {en} enemies and {n} cards", en >= 4 and n >= 8, f"{en} enemies, {n} cards in hand")
    bad = o.r.cmp_rects()
    o.check("crowded fight: every enemy below the top bar and clear of hand and dock", not bad, "; ".join(bad[:3]))
    scroll = o.p.js("document.documentElement.scrollHeight - innerHeight")
    o.check("crowded fight: the page does not scroll", scroll <= 1, f"{scroll}px of scroll")
    o.r.shot("crowded-fight")


@scenario
def start_tiles(o):
    """WEB53: the start screen's tiles (Compendium, History) end above the sticky Start button, scrolled to the end if the screen needs it."""
    p = o.p
    if not o.r.touch:
        return  # the desktop start screen is two columns with the button beside them
    p.open(URL)
    p.js("localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full'})); 0")
    p.open(URL)
    p.settle(1.5)
    o.r.shot("start")
    covered = """(() => { const go = document.querySelector('#start .go button').getBoundingClientRect();
      return [...document.querySelectorAll('#start .dailybtn, #start .foot > *')].filter((e) => e.getBoundingClientRect().bottom > go.top + 1).map((e) => e.textContent.trim().slice(0, 14)); })()"""
    if p.js("innerWidth >= 375 && innerHeight >= 660"):
        bad = p.js(covered)
        o.check("start: the tiles show whole without scrolling (390 x 664 and up)", not bad, f"covered: {bad}")
    p.js("document.getElementById('start').scrollTop = 9999; 0")
    p.settle(0.4)
    bad = p.js("""(() => { const go = document.querySelector('#start .go button').getBoundingClientRect();
      return [...document.querySelectorAll('#start .dailybtn, #start .foot > *')].filter((e) => e.getBoundingClientRect().bottom > go.top + 1).map((e) => e.textContent.trim().slice(0, 14)); })()""")
    o.check("start: the tiles are clear of the Start button", not bad, f"covered: {bad}")


@scenario
def ascension_detail(o):
    """WEB55: stepping the ascension up with real taps shows that level's title and description from ascension.json; the line keeps its size; the list names levels 1..n."""
    p = o.p
    path = os.path.join(cache_dir("data"), "text/ascension.json")
    if not os.path.exists(path):
        raise Skip("no ascension.json (no import)")
    asc = json.load(open(path))
    p.open(URL)
    p.js("localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full'})); 0")
    p.open(URL)
    p.settle(1.5)
    shown = "(() => { const e = document.querySelector('#start .ascinfo'); return e && [e.querySelector('.asctitle').textContent, e.querySelector('.ascdesc').textContent.trim(), Math.round(e.getBoundingClientRect().height), Math.round(e.getBoundingClientRect().top - document.querySelector('#start .stepper').getBoundingClientRect().top)]; })()"
    plainx = lambda t: re.sub(r"\[/?[a-z]+\]", "", t)
    bad, geo = [], set()
    for n in range(0, 11):
        if n:
            if not o.tap_js("document.querySelectorAll('#start .stepper button')[1]", 0.3):
                bad.append(f"no + at {n}")
                break
        got = p.js(shown)
        want = (asc[f"LEVEL_{n:02d}.title"], plainx(asc[f"LEVEL_{n:02d}.description"]))
        if not got or (got[0], got[1]) != want:
            bad.append(f"A{n}: {got and got[:2]} != {want}")
        if got:
            geo.add(tuple(got[2:]))
        if n in (0, 1, 10):
            o.r.shot(f"ascension-{n}")
    o.check("ascension: each level's title and description match ascension.json", not bad, "; ".join(bad[:3]))
    o.check("ascension: the line keeps its place and size at every level", len(geo) == 1, f"{sorted(geo)}")
    overflow = p.js("(() => { const e = document.querySelector('#start .ascinfo'); const d = e.querySelector('.ascdesc'); return d.getBoundingClientRect().bottom - e.getBoundingClientRect().bottom; })()")
    o.check("ascension: the longest description fits its line", overflow is not None and overflow <= 0, f"{overflow}px over")
    o.tap_js("document.querySelector('#start .ascinfo')", 0.5)
    rows = p.js("[...document.querySelectorAll('#sheet .asclvl b')].map((b) => b.textContent)")
    want = [f"{i} · {asc[f'LEVEL_{i:02d}.title']}" for i in range(1, 11)]
    o.check("ascension: tapping the line lists levels 1 to 10", rows == want, f"{rows}")
    o.r.shot("ascension-list")


@scenario
def potion_use(o):
    """A potion used is that potion, on that target: Fire Potion from slot 2 on the second enemy; the others stay on the belt."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "CULTISTS_NORMAL", "potions": ["BLOCK_POTION", "FIRE_POTION", "SKILL_POTION"]})
    v = o.view()
    if o.potions()[:3] != ["Block Potion", "Fire Potion", "Skill Potion"]:
        raise Skip(f"belt {o.potions()}")
    ens = [e for e in enemies(v) if e["alive"]]
    hp0, belt0 = o.enemy_hp(v), o.potions()
    target = ens[1]["id"]
    o.tap_js(potion_slot_js(1), 0.6)
    ok = o.tap("#sheet button.primary, #sheet .row:first-of-type, #sheet [data-bid]", 0.6) if False else None
    # the potion's sheet: its Use button
    used = o.p.js("(() => { const b = [...document.querySelectorAll('#sheet button, #sheet .row')].find((x) => /^Use\\b/.test(x.textContent.trim())); if (!b) return false; b.scrollIntoView({block: 'center'}); const r = b.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()")
    if not used:
        raise Skip("no Use button")
    o.p.click(*used)
    o.settle(0.6)
    o.tap_js(f"document.querySelector('#screen .enemy[data-id=\"{target}\"]')", 1.0)
    v2 = o.view()
    hp1 = o.enemy_hp(v2)
    hit = {k: hp0[k] - hp1.get(k, 0) for k in hp0 if hp0[k] != hp1.get(k, 0)}
    belt1 = o.potions()
    o.check("potion used is the one tapped, on the enemy aimed at", hit == {target: 20} and belt1[:3] == ["Block Potion", "", "Skill Potion"], f"Fire Potion on {target}: hp lost {hit}; belt {belt0} -> {belt1}")
    # throw away the third (Skill Potion): only it goes
    o.tap_js(potion_slot_js(2), 0.6)
    away = o.p.js("(() => { const b = [...document.querySelectorAll('#sheet button, #sheet .row')].find((x) => /^Throw away\\b/.test(x.textContent.trim())); if (!b) return false; b.scrollIntoView({block: 'center'}); const r = b.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()")
    if not away:
        raise Skip("no Throw away button")
    o.p.click(*away)
    o.settle(0.6)
    if state_asks(o):
        o.p.click(*away)
        o.settle(0.6)
    o.check("throw away removes the potion tapped", o.potions()[:3] == ["Block Potion", "", ""], f"belt {belt1} -> {o.potions()}")


def state_asks(o):
    return bool(o.p.js("!!document.querySelector('#sheet:not([hidden])') && [...document.querySelectorAll('#sheet button, #sheet .row')].some((x) => /Throw away/.test(x.textContent) && /again|sure/i.test(x.textContent))"))


def potion_do(o, slot, word):
    """Taps belt slot `slot`, then the sheet's button `word` (Use, Throw away), asking again when it asks."""
    o.tap_js(potion_slot_js(slot), 0.6)
    js = ("(() => { const b = [...document.querySelectorAll('#sheet button, #sheet .row')].find((x) => /^%s\\b/.test(x.textContent.trim())); "
          "if (!b) return false; b.scrollIntoView({block: 'center'}); const r = b.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()") % word
    pos = o.p.js(js)
    if not pos:
        return False
    o.p.click(*pos)
    o.settle(0.7)
    return True


def choice(o, v=None):
    v = v or o.view()
    sh = v["screen"].get("sheet")
    if not sh:
        return None
    kind, body = next(iter(sh.items()))
    return [r for r in (body.get("rows") or []) if r.get("card")]


def to_hand(o, name, turns=8):
    """The row of an enabled `name` in the hand, ending turns until one is drawn."""
    for _ in range(turns):
        v = o.view()
        if not v["fight"]:
            return None
        row = next((r for r in o.hand(v) if r["card"]["name"] == name and state(r) == "Enabled"), None)
        if row:
            return row
        o.tap("#dock .fightbar .primary", 2.0)
    return None


@scenario
def fight_choices(o):
    """Choices in a fight: pick 1 of 3 (Skill Potion, Discovery), discard (Survivor), exhaust (Burning Pact), upgrade (Armaments),
    and a multi-pick grid (Gambler's Brew) with a tap-again to put one back: the cards tapped are the ones acted on."""
    from collections import Counter
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    cards = ["BURNING_PACT"] * 3 + ["ARMAMENTS"] * 3 + ["DISCOVERY"] * 3 + ["SURVIVOR"] * 3
    o.enter_fight({"hp": 300, "encounter": "AXEBOTS_NORMAL", "cards": cards, "potions": ["BLOCK_POTION", "SKILL_POTION", "GAMBLERS_BREW"]})
    names = lambda v: Counter(r["card"]["name"] for r in o.hand(v))
    # pick 1 of 3: Skill Potion, the third card
    h0 = names(o.view())
    if potion_do(o, 1, "Use"):
        ch = choice(o)
        if ch and len(ch) >= 3:
            want = ch[2]["card"]["name"]
            o.tap(f'#sheet [data-bid="{ch[2]["id"]}"]', 0.9)
            d = names(o.view()) - h0
            o.check("fight choice: pick 1 of 3 (Skill Potion, 3rd)", d == Counter({want: 1}) and o.potions()[1] == "", f"offered {[c['card']['name'] for c in ch]}, tapped {want}; hand gained {dict(d)}")
        else:
            o.check("fight choice: Skill Potion offers cards", False, f"{ch}")
    # multi-pick: Gambler's Brew. Pick two, tap the first again to put it back (the game's NPlayerHand.DeselectCard), confirm:
    # only the one still picked is discarded.
    d0 = o.pile(2) or {}
    if potion_do(o, 2, "Use"):
        ch = choice(o)
        if ch and len(ch) >= 3:
            first = ch[1]["card"]["name"]
            o.tap(f'#sheet [data-bid="{ch[1]["id"]}"]', 0.6)
            ch = choice(o)
            second = ch[len(ch) - 2]["card"]["name"]
            o.tap(f'#sheet [data-bid="{ch[-2]["id"]}"]', 0.6)
            ch = choice(o)
            picked = [r for r in ch if state(r) == "Picked"]
            o.check("fight multi-pick: both picked cards show as picked", len(picked) == 2, f"{[(r['card']['name'], state(r)) for r in ch]}")
            o.r.shot("brew-two-picked")
            back = next((r for r in picked if r["card"]["name"] == first), None)
            if back:
                o.tap(f'#sheet [data-bid="{back["id"]}"]', 0.8)
            ch = choice(o)
            left = [r["card"]["name"] for r in ch if state(r) == "Picked"]
            o.check("fight multi-pick: tapping a picked card puts it back", len(left) == 1 and left == [second], f"picked after the put-back: {left}")
            o.r.shot("brew-put-back")
            prim = o.view()["screen"].get("primary")
            o.tap(f'#dock [data-bid="{prim["id"]}"], #sheet [data-bid="{prim["id"]}"]', 1.0) if prim else None
            d1 = o.pile(2) or {}
            got = o.delta(d0, d1)
            o.check("fight multi-pick: exactly the card still picked is discarded", got == {second: 1}, f"picked {first}, {second}, put back {first}; discard pile change {got}")
        else:
            o.check("fight choice: Gambler's Brew offers cards", False, f"{ch}")
    # discard 1 (Survivor), exhaust 1 (Burning Pact), upgrade 1 (Armaments), discover (Discovery): the second card offered each time
    for card, word in (("Survivor", "discard"), ("Burning Pact", "exhaust"), ("Armaments", "upgrade"), ("Discovery", "discover")):
        row = to_hand(o, card)
        if not row:
            o.check(f"fight choice: {word} ({card})", False, "card never drawn")
            continue
        v = o.view()
        h0, dis0, exh0 = names(v), o.pile(2) or {}, o.pile(3) or {}
        o.tap(f'#screen .hand .card[data-bid="{row["id"]}"]', 0.9)
        ch = choice(o)
        if not ch or len(ch) < 2:
            o.check(f"fight choice: {word} ({card}) offers cards", False, f"{ch}")
            continue
        pick = ch[1]
        pname = pick["card"]["name"]
        o.tap(f'#sheet [data-bid="{pick["id"]}"]', 0.9)
        prim = o.view()["screen"].get("primary")
        if o.view()["screen"].get("sheet") and prim:
            o.tap(f'#dock [data-bid="{prim["id"]}"], #sheet [data-bid="{prim["id"]}"]', 0.9)
        h1 = names(o.view())
        if word == "discard":
            got = o.delta(dis0, o.pile(2) or {})
            want = Counter({card: 1}) + Counter({pname: 1})
            ok = Counter({k: v for k, v in got.items()}) == want
            detail = f"tapped {pname}; discard change {got}"
        elif word == "exhaust":
            got = o.delta(exh0, o.pile(3) or {})
            ok, detail = got == {pname: 1}, f"tapped {pname}; exhaust change {got}"
        elif word == "upgrade":
            up = pname + "+" if not pname.endswith("+") else pname
            ok = h1[up] - h0[up] == 1 or h1.get(up, 0) > h0.get(up, 0)
            detail = f"tapped {pname}; hand {dict(h0)} -> {dict(h1)}"
        else:
            ok = (h1 - h0 + Counter()) and h1[pname] - h0[pname] >= 1
            detail = f"tapped {pname}; hand gained {dict(h1 - h0)}"
        o.check(f"fight choice: {word} takes the card tapped ({card}, 2nd)", bool(ok), detail)


@scenario
def map_pick(o):
    """The map: the room tapped (not the first) is the room entered: the map's marker stands on it, and the room is its kind."""
    o.setup(0, "WEBTEST2")
    v = o.to_map(0)
    m = next(b["Map"] for b in v["screen"]["blocks"] if "Map" in b)
    cells = [c for c in m["cells"] if c["reachable"] and c["button"]]
    if len(cells) < 2:
        raise Skip("one room to go to")
    pick = cells[-1]
    o.tap(f'g.node[data-button="{pick["button"]}"]', 2.0)
    v = o.view()
    kind_ok = {"Monster": v["fight"], "Elite": v["fight"], "Boss": v["fight"], "Shop": v["decision"] == "shop", "RestSite": v["decision"] == "rest",
               "Treasure": v["decision"] == "treasure"}.get(pick["room"], v["decision"] != "map")
    o.tap("#top .iconbtn:nth-of-type(2)", 0.9)
    sh = o.view().get("sheet") or {}
    you = next((b["Map"]["you"] for b in sh.get("blocks", []) if isinstance(b, dict) and "Map" in b), None)
    o.r.close_sheet()
    o.check("map: the room tapped is the room entered", kind_ok and you == [pick["x"], pick["y"]], f"tapped ({pick['x']},{pick['y']}) {pick['room']} of {[(c['x'], c['y']) for c in cells]}; entered {v['decision']}, marker {you}")


@scenario
def map_near_tap(o):
    """The map on touch: a tap a little off a next room (toward the rooms after it) still travels there."""
    if not o.r.touch:
        return
    o.setup(0, "WEBTEST2")
    v = o.to_map(0)
    m = next(b["Map"] for b in v["screen"]["blocks"] if "Map" in b)
    cells = [c for c in m["cells"] if c["reachable"] and c["button"]]
    if not cells:
        raise Skip("no room to go to")
    pick = cells[-1]
    r = o.p.js(f"(() => {{ const r = document.querySelector('g.node[data-button=\"{pick['button']}\"]').getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; }})()")
    o.p.click(r[0] + 6, r[1] - 30)
    o.settle(2.0)
    v = o.view()
    o.check("map: a tap 30px off a next room travels to it", v["decision"] != "map" or v["fight"], f"still {v['decision']}")


@scenario
def boss_tap(o):
    """The map: when the boss is the next room, a tap on its icon starts the boss fight (no button in the dock for it on a phone)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.debug({"room": "boss"})
    o.settle(1.0)
    v = o.view()
    m = next((b["Map"] for b in v["screen"]["blocks"] if "Map" in b), None)
    if v["decision"] != "map" or not m or not m.get("boss_button"):
        o.check("map: the boss is the next room", False, f"decision {v['decision']}, boss_button {m and m.get('boss_button')}")
        return
    o.r.shot("boss-next")
    if o.r.touch:
        o.check("map: no dock button for the boss on a phone", not o.p.js("!!document.querySelector('#dock .next button, #dock .next .row')"))
    o.tap("g.node.boss[data-boss]", 2.5)
    v = o.view()
    o.check("map: a tap on the boss starts the boss fight", bool(v["fight"]), f"decision {v['decision']} room {v.get('room')}")


def win_fight(o, turns=16):
    for _ in range(turns):
        v = o.view()
        if not v["fight"]:
            return True
        while v and v["fight"] and not v["screen"].get("sheet"):
            nv = o.r.play_one(v, "tap")
            if nv is None:
                break
            v = nv
        if v and v["fight"]:
            o.tap("#dock .fightbar .primary", 2.0)
    return not o.view()["fight"]


@scenario
def after_fight(o):
    """A won elite: the relic, potion and gold rewards and the card tile tapped give what they show; then Rest heals the shown
    amount and Smith upgrades the card tapped (second)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 60, "encounter": "BYRDONIS_ELITE", "cards": ["ANGER"] * 14, "potions": ["", "", ""][:0]})
    if not win_fight(o):
        raise Skip("the fight was not won")
    o.settle(1.0)
    v = o.view()
    if v["decision"] != "rewards":
        raise Skip(f"decision {v['decision']}")
    rows = rows_of(v)
    log = [plain(r["label"]) for r in rows]
    relic = next((r for r in rows if (r.get("info") or {}).get("Relic")), None)
    if relic:
        want, r0 = relic_name(relic), o.relics()
        o.tap_bid(relic["id"])
        got = [x for x in o.relics() if x not in r0]
        o.check("relic reward as tapped", want in got, f"tapped {want}, relics gained {got}")
    gold = next((r for r in rows_of(o.view()) if re.search(r"\d+ gold", plain(r["label"]), re.I)), None)
    if gold:
        n, g0 = int(re.search(r"(\d+) gold", plain(gold["label"]), re.I).group(1)), o.gold()
        o.tap_bid(gold["id"])
        o.check("gold reward as tapped", o.gold() == g0 + n, f"gold {g0} -> {o.gold()} (shown {n})")
    pot = next((r for r in rows_of(o.view()) if (r.get("info") or {}).get("Potion")), None)
    if pot:
        want, p0 = re.sub(r"^[^\w]+", "", plain(pot["label"])).strip(), o.potions()
        o.tap_bid(pot["id"])
        got = [x for x in o.potions() if x and x not in p0]
        o.check("potion reward as tapped (after a fight)", want in got, f"tapped {want}, belt {p0} -> {o.potions()}")
    names = o.p.js("[...document.querySelectorAll('.peekreward .card')].map((c) => c.querySelector('.g-title, .cname').textContent.trim())") or []
    if len(set(names)) >= 2:
        d0 = o.deck()
        o.tap(f".peekreward .card:nth-child({len(names) - 1})", 0.8)
        d = o.delta(d0, o.deck())
        o.check("card reward as tapped (2nd tile)", d == {names[-2]: 1}, f"tapped {names[-2]} of {names}; deck change {d}")
    print("# rewards were", log, flush=True)
    o.tap("#dock .primary", 0.8)
    if o.view()["decision"] != "map":
        raise Skip(f"decision {o.view()['decision']} after rewards")
    # rest: heal as shown
    o.debug({"room": "rest"})
    v = o.view()
    rest = next((r for r in rows_of(v) if plain(r["label"]).startswith("Rest")), None)
    if v["decision"] != "rest" or not rest:
        raise Skip("no rest site")
    t = o.top()
    said = plain(rest["label"]) + " " + plain(rest.get("right")) + " " + " ".join(plain(d) for d in rest.get("detail", []))
    # The game's text is "Heal for 30% of your Max HP (18).": the amount is the one in parentheses (WEB64).
    m = re.search(r"\((\d+)\)", said) or re.search(r"(\d+)", said)
    heal = int(m.group(1)) if m else 0
    shown = o.p.js("document.querySelector('.resthp .healed')?.textContent")
    want = min(t["max_hp"], t["hp"] + heal)
    o.tap_bid(rest["id"], confirm=True)
    o.check("rest heals the amount shown", o.top()["hp"] == want and (shown in (None, str(want))), f"hp {t['hp']}/{t['max_hp']}, shown heal {heal} -> {want} (panel {shown}); hp now {o.top()['hp']}")
    o.tap("#dock .primary", 0.8)
    # smith
    o.debug({"room": "rest"})
    smith = next((r for r in rows_of(o.view()) if plain(r["label"]).startswith("Smith")), None)
    if not smith:
        raise Skip("no smith")
    d0 = o.deck()
    o.tap_bid(smith["id"])
    grid = [r for r in rows_of(o.view()) if r.get("card")]
    if len(grid) < 2:
        raise Skip("smith grid")
    pick = grid[1]
    name = pick["card"]["name"].rstrip("+")  # an upgrade grid names the card as it will be
    o.tap(f'#screen [data-bid="{pick["id"]}"]', 0.6)
    pair = o.p.js("document.querySelectorAll('#sheet .pickview .uppair .card').length")
    o.check("smith: the preview shows the card now and upgraded", pair == 2, f"{pair} cards in the pair")
    o.r.shot("smith-preview")
    if o.preview_open():
        o.tap("#sheet .pickgo", 1.0)
    d = o.delta(d0, o.deck())
    o.check("smith upgrades the card tapped (2nd)", d == {name: -1, name + "+": 1}, f"tapped {name}; deck change {d}")


@scenario
def skips(o):
    """Proceed past unclaimed rewards takes nothing and spends nothing; Proceed after a treasure's relic goes on to the map."""
    o.setup(4, "WEBTEST1")
    o.tap(f'#screen .row[data-bid="{rows_of(o.view())[0]["id"]}"]', 1.0)
    if o.view()["decision"] != "rewards":
        raise Skip("no rewards")
    g0, d0, p0, r0 = o.gold(), o.deck(), o.potions(), o.relics()
    for _ in range(3):
        if o.view()["decision"] != "rewards":
            break
        o.tap("#dock .primary", 0.8)  # Proceed; asks to leave the rewards behind: again
    o.check("rewards: Proceed past unclaimed rewards takes nothing", o.view()["decision"] == "map" and o.gold() == g0 and o.deck() == d0 and o.potions() == p0 and o.relics() == r0,
            f"decision {o.view()['decision']}; gold {g0} -> {o.gold()}; deck change {o.delta(d0, o.deck())}; belt {p0} -> {o.potions()}")
    o.debug({"room": "treasure"})
    v = o.view()
    rel = next((r for r in rows_of(v) if "relic" in plain(r["label"]).lower() or (r.get("info") or {}).get("Relic")), None)
    if v["decision"] != "treasure" or not rel:
        raise Skip(f"no treasure ({v['decision']})")
    o.tap_bid(rel["id"])
    o.tap("#dock .primary", 1.0)
    o.check("treasure: Proceed after the relic goes on to the map", o.view()["decision"] == "map", f"decision {o.view()['decision']}")


@scenario
def menu_tiles(o):
    """The menu's tiles open what they name; the sheet's close button closes it."""
    o.setup(4, "WEBTEST1")
    o.to_map(0)
    names = ("Relics", "Potions", "Rooms", "Help")  # Deck and Map are the top bar's buttons (WEB60)
    for word in names:
        if (o.p.js("document.querySelector('#sheet:not([hidden]) .sheethead h2')?.textContent") or "") != "Menu":
            o.tap("#top .iconbtn:nth-of-type(3)", 0.6)
        tile = o.p.js(f"[...document.querySelectorAll('#sheet .tile')].findIndex((t) => t.textContent.includes({json.dumps(word)}))")
        if tile is None or tile < 0:
            o.check(f"menu tile {word} exists", False, "missing")
            o.r.close_sheet()
            continue
        o.tap(f"#sheet .tile:nth-child({tile + 1})", 0.8)
        title = o.p.js("document.querySelector('#sheet:not([hidden]) .sheethead h2')?.textContent") or ""
        o.check(f"menu tile {word} opens its sheet", word.lower()[:4] in title.lower() or (word == "Potions" and title == "Info") or (word == "Rooms" and "oom" in title) or (word == "Help"), f"title {title!r}")
        if not o.tap("#sheet .iconbtn.close", 0.6):
            o.r.close_sheet()
        now = o.p.js("document.querySelector('#sheet:not([hidden]) .sheethead h2')?.textContent")
        o.check(f"sheet close button closes ({word})", now in (None, "Menu"), f"sheet now {now!r}")  # Help is drawn over the menu, which shows again


@scenario
def run_review(o):
    """A real defeat (End Turn only, by real input) reviewed from the end screen and from History: the picks are real moves, the board is that moment, leaving keeps the work."""
    o.setup(0, "WEBREVIEW1")
    for _ in range(60):
        v = o.view()
        if v["over"]:
            break
        if v["fight"]:
            o.tap(".endturn", 0.9)
        elif v["decision"] == "map":
            if not o.tap("#screen g.node.reach", 1.0):
                raise Skip("no room to tap on the map")
        else:
            o.to_map()
    v = o.view()
    if not v["over"]:
        raise Skip(f"the run did not end ({v['decision']})")
    o.settle(3.6)  # the Defeat beat clears
    moves = len(v["save"]["actions"])
    o.check("review: the end screen offers it, and nothing has been worked out yet", o.p.js("!!document.querySelector('#screen .reviewrun') && !localStorage.getItem('sts2-reviews-v1') && !document.getElementById('review')"))
    o.tap("#screen .reviewrun", 0.1)
    o.check("review: the row opens it", o.p.js("!!document.getElementById('review')"))
    o.tap("#review .rvclose", 0.3)
    o.check("review: close leaves the end screen as it was", o.p.js("!document.getElementById('review') && !!document.querySelector('#screen .reviewrun')") and o.view()["over"])
    done = False
    for _ in range(240):
        if "Ready" in (o.p.js("document.querySelector('#screen .reviewrun .sub')?.textContent") or ""):
            done = True
            break
        o.settle(0.5)
    o.check("review: goes on after it is left, and the row says when it is ready", done, (o.p.js("document.querySelector('#screen .reviewrun .sub')?.textContent") or "") + " | saved " + (o.p.js("(localStorage.getItem('sts2-reviews-v1') || 'none').slice(0, 120)") or "") + " | " + "; ".join(o.p.errors()[-2:]))
    o.tap("#screen .reviewrun", 0.5)
    cards = o.p.js("document.querySelectorAll('#review .rvcard').length")
    o.check("review: coming back shows the picks", 1 <= (cards or 0) <= 3, f"{cards} cards")
    r = json.loads(o.p.js("localStorage.getItem('sts2-reviews-v1')"))[0]["result"]
    picks = r["picks"]
    o.check("review: each fight pick is the End Turn played there, against another move the search scores higher",
            bool(picks) and all(p["at"] < moves and p["you"] == "End Turn" and p["bot"] != "End Turn" and p["score"]["bot"] > p["score"]["you"] for p in picks if p["kind"] == "combat"), json.dumps(picks)[:300])
    shown = o.p.js("[...document.querySelectorAll('#review .rvcard')].map(c => [...c.querySelectorAll('.rvmove b')].map(b => b.textContent))")
    o.check("review: the cards show the picks' moves, biggest gap first", shown == [[p["you"], p["bot"]] for p in picks] and [p["rank"] for p in picks] == sorted((p["rank"] for p in picks), reverse=True), str(shown))
    o.r.shot("review-cards")
    if o.r.touch:
        def at():
            return o.p.js("[...document.querySelectorAll('#review .rvdot')].findIndex(d => d.classList.contains('on'))")
        if cards > 1:
            box = o.p.js("(() => { const r = document.querySelector('#review .rvtrack').getBoundingClientRect(); return [r.left, r.top, r.width, r.height]; })()")
            y = box[1] + box[3] * 0.45
            o.p.drag(box[0] + box[2] * 0.85, y, box[0] + box[2] * 0.1, y)
            o.settle(0.5)
            o.check("review: a swipe goes to the next decision", at() == 1, f"dot {at()}")
            o.tap("#review .rvprev", 0.5)
            o.check("review: the ‹ button goes back", at() == 0, f"dot {at()}")
            o.tap("#review .rvnext", 0.5)
            o.check("review: the › button goes on", at() == 1, f"dot {at()}")
            o.tap("#review .rvdot:nth-child(1)", 0.5)
            o.check("review: a dot goes to its decision", at() == 0, f"dot {at()}")
    else:
        lefts = o.p.js("[...document.querySelectorAll('#review .rvcard')].map(c => c.getBoundingClientRect().left)")
        o.check("review: desktop shows the decisions side by side, no pager", lefts == sorted(set(lefts)) and not o.p.js("document.querySelector('#review .rvnav')?.getClientRects().length"), str(lefts))
    o.tap("#review .rvcard .rvsee", 0.3)
    for _ in range(40):
        if o.p.js("!!document.getElementById('rvboard')"):
            break
        o.settle(0.25)
    o.settle(0.5)
    b = o.view()
    p0 = picks[0]
    o.check("review: See the board shows that moment", o.p.js("!!document.getElementById('rvboard')") and b["save"]["floor"] == p0["floor"] and b["fight"] == (p0["kind"] == "combat") and not b["over"], f"floor {b['save']['floor']} fight {b['fight']}")
    o.check("review: the bot's move is lit on the board", o.p.js("document.querySelectorAll('.rvlitbot').length") > 0 or p0["kind"] != "combat")
    if b["fight"]:
        bad = o.r.cmp_rects()
        o.check("review: the board's enemies sit below the top bar and clear of the hand and dock", not bad, "; ".join(bad[:3]))
    o.r.shot("review-board")
    before = json.dumps(b)
    if b["fight"] and o.hand(b):
        o.tap("#screen .hand .card", 0.5)
    o.check("review: the board is read only", json.dumps(o.view()) == before)
    o.tap("#rvboard .rvback", 0.6)
    o.check("review: Back returns to the same decision, the run still over, no second Defeat", o.p.js("!!document.querySelector('#review .rvcard') && !document.getElementById('review').hidden && !document.querySelector('.moment')") and o.view()["over"])
    if o.r.touch:
        o.tap("#review .rvclose", 0.5)
    else:
        o.p.key("Escape")
        o.settle(0.5)
    o.check("review: closes back to the end screen with its row", o.p.js("!document.getElementById('review') && !!document.querySelector('#screen .reviewrun')") and o.view()["over"])
    # From History, after a reload: the saved picks, at once.
    o.p.open(URL)
    o.settle(1.5)
    o.tap(".startx .dailybtn:last-child", 0.6)
    o.tap("#sheet .list button", 0.6)
    o.check("review: one finished run in History, not two", o.p.js("JSON.parse(localStorage.getItem('sts2-history-v1')).length") == 1)
    o.tap("#sheet .review-run", 0.6)
    again = o.p.js("[...document.querySelectorAll('#review .rvcard')].map(c => [...c.querySelectorAll('.rvmove b')].map(b => b.textContent))")
    o.check("review: History opens the same picks, kept through a reload", again == shown, str(again))
    o.tap("#review .rvclose", 0.5)
    o.check("review: closing goes back to History's run", o.p.js("!!document.querySelector('#sheet:not([hidden]) .review-run')"))


THIEF = {"hp": 300, "encounter": "THIEVING_HOPPER_WEAK", "cards": ["ANGER"] * 14}


def thief_chip(o):
    return o.p.js("(() => { const c = document.querySelector('#screen .enemy .powers .stolen'); return c ? c.textContent.trim() : null; })()")


def thief_start(o):
    """A Thieving Hopper fight, one turn ended: the hopper has stolen a card (Thievery is its first move)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight(THIEF)
    o.check("thief: before the theft the hopper shows no stolen card", thief_chip(o) is None)
    o.tap("#dock .fightbar .primary", 2.0)
    name = thief_chip(o)
    o.check("thief: after Thievery the hopper's card shows the stolen card's name", bool(name), f"chip {name!r}")
    o.r.shot("thief-holds")
    if name:
        o.tap("#screen .enemy .powers .stolen", 0.6)
        o.check("thief: tapping the stolen card opens it to read", o.p.js("!!document.querySelector('#sheet:not([hidden]) .bigcard, #sheet:not([hidden]) .inspect')"))
        o.r.shot("thief-card-read")
        o.r.close_sheet()
    return name


@scenario
def thief_kill(o):
    """Killing the hopper returns the stolen card as a reward that adds it to the deck."""
    name = thief_start(o)
    if not name:
        raise Skip("nothing stolen")
    if not win_fight(o, 30):
        raise Skip("hopper not killed")
    o.settle(1.0)
    v = o.view()
    rows = rows_of(v)
    o.r.shot("thief-returned")
    card = next((r for r in rows if re.search(re.escape(name), plain(r["label"]))), None)
    o.check("thief kill: the stolen card is offered back", v["decision"] == "rewards" and card is not None, f"{name!r} in {[plain(r['label']) for r in rows]}")
    if card:
        d0 = o.deck()
        o.tap_bid(card["id"])
        o.check("thief kill: taking it puts the card back in the deck", o.delta(d0, o.deck()).get(name) == 1, f"deck change {o.delta(d0, o.deck())}")


@scenario
def thief_escape(o):
    """Left alone, the hopper escapes with the card: the fight says so and the deck is a card short."""
    name = thief_start(o)
    if not name:
        raise Skip("nothing stolen")
    d0 = o.deck()
    for _ in range(8):
        if not o.view()["fight"]:
            break
        o.tap("#dock .fightbar .primary", 1.5)
        o.r.shot("thief-turn")
    v = o.view()
    o.r.shot("thief-escaped")
    o.settle(1.0)
    said = o.p.js("document.getElementById('screen').innerText")
    o.check("thief escape: the screen says the hopper escaped with the stolen card", f"escaped with {name}" in said, f"{name!r} in {said[:160]!r}")
    rows = rows_of(o.view())
    o.r.shot("thief-after")
    o.check("thief escape: the card is not offered back", not any(re.search(re.escape(name), plain(r["label"])) for r in rows), str([plain(r["label"]) for r in rows]))


SLOW = {"after_fight", "fight_choices", "fight_four", "skips", "menu_tiles"}


@scenario
def web60_rest_menu_map(o):
    """WEB60: a full-HP rest site shows no HP before-after; the menu has no Deck or Map tile (the top bar has them); the map has no
    "Map" heading, keeps "Act 1" and its place; the boss as the next room looks reachable."""
    o.setup(0, "WEBTEST2")
    v = o.to_map(0)
    head = o.p.js("""(() => { const h = document.querySelector('#screen .head'); const m = document.querySelector('#screen .map');
      const t = h && h.querySelector('h1'); const a = h && h.querySelector('.aside');
      return { h1: t ? getComputedStyle(t).visibility : null, aside: a ? a.textContent : null, mapTop: m ? Math.round(m.getBoundingClientRect().top) : null, headBottom: h ? Math.round(h.getBoundingClientRect().bottom) : null }; })()""")
    o.check("map: no visible Map heading, Act 1 kept", head and head["h1"] in (None, "hidden") and head["aside"] == "Act 1", f"{head}")
    o.check("map: the map starts below the header", head and head["mapTop"] is not None and head["mapTop"] >= head["headBottom"] - 1, f"{head}")
    o.r.shot("map-after")
    o.tap("#top .iconbtn:nth-of-type(3)", 0.7)
    tiles = o.p.js("[...document.querySelectorAll('#sheet .tile')].map((t) => t.textContent.trim())") or []
    o.r.shot("menu-after")
    o.check("menu: no Deck or Map tile, the rest stay", not any(t in ("Deck", "Map") for t in tiles) and "Relics" in tiles and any("etting" in t for t in tiles), f"{tiles}")
    o.r.close_sheet()
    o.debug({"room": "rest"})
    v = o.view()
    t = o.top()
    if v["decision"] != "rest":
        raise Skip("no rest site")
    full = t["hp"] == t["max_hp"]
    if not full:
        raise Skip(f"hp {t['hp']}/{t['max_hp']} is not full")
    o.r.shot("rest-after")
    o.check("rest at full HP: no HP before-after", not o.p.js("!!document.querySelector('#screen .restpanel, #screen .resthp')"))
    o.check("rest at full HP: the Rest row is there", any(plain(r["label"]).startswith("Rest") for r in rows_of(v)))
    o.tap("#top .iconbtn:nth-of-type(2)", 0.7)
    o.r.close_sheet()


@scenario
def web60_boss_reach(o):
    """WEB60: the boss as the next room is lit like the other next rooms (a glow and a bob)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.debug({"room": "boss"})
    o.settle(1.0)
    o.r.shot("boss-after")
    lit = o.p.js("""(() => { const n = document.querySelector('g.node.boss.go'); if (!n) return null; const i = n.querySelector('.mi, rect');
      const c = getComputedStyle(i); return { anim: c.animationName, filter: c.filter, stroke: c.stroke }; })()""")
    o.check("map: the next-room boss is lit (animated, glowing)", bool(lit) and lit["anim"] not in (None, "none") and (("drop-shadow" in (lit["filter"] or "")) or lit["stroke"] != "none"), f"{lit}")


@scenario
def web60_compendium_filter(o):
    """WEB60: on a phone the compendium's filters sit behind one Filter button with the active count; desktop keeps the chips."""
    o.setup(0, "WEBTEST2")
    o.p.js("document.querySelector('#sheet') && 0")
    # the start screen is where the compendium opens from
    o.p.open(URL)
    o.p.settle(1.5)
    if not o.tap_js("[...document.querySelectorAll('#start .dailybtn')].find((b) => b.textContent.includes('Compendium'))", 1.0):
        raise Skip("no Compendium tile")
    chips = "(() => { const f = document.querySelector('.cfilters'); return !!f && f.getBoundingClientRect().height > 0; })()"
    if o.r.touch:
        o.check("compendium (phone): the filter chips are behind a button", not o.p.js(chips))
        btn = o.p.js("document.querySelector('.cfilterbtn')?.textContent")
        o.check("compendium (phone): the button says Filter", btn == "Filter", f"{btn!r}")
        first = o.p.js("document.querySelector('.cgrid .card, .cgrid > *')?.getBoundingClientRect().top")
        o.check("compendium (phone): the list starts in the top half", first is not None and first < o.p.js("innerHeight") / 2, f"first tile top {first}")
        o.r.shot("compendium-after")
        o.tap(".cfilterbtn", 0.7)
        o.r.shot("compendium-filter-after")
        o.check("compendium (phone): the button opens the filters in a sheet", (o.p.js("document.querySelector('#sheet:not([hidden]) .sheethead h2')?.textContent") or "") == "Filter")
        o.tap_js("[...document.querySelectorAll('#sheet .cfilter .chipbtn')].find((b) => b.textContent === 'Attack')", 0.5)
        o.tap_js("[...document.querySelectorAll('#sheet .primary')].find((b) => b.textContent === 'Done')", 0.7)
        btn = o.p.js("document.querySelector('.cfilterbtn')?.textContent")
        o.check("compendium (phone): the button shows the active count", btn == "Filter (1)", f"{btn!r}")
    else:
        o.r.shot("compendium-after")
        o.check("compendium (desktop): the filter chips show, no button", o.p.js(chips) and not o.p.js("(() => { const b = document.querySelector('.cfilterbtn'); return !!b && b.getBoundingClientRect().height > 0; })()"))


@scenario
def web60_start_desktop(o):
    """WEB60: the desktop start screen fits the window without scrolling at every ascension, its title whole."""
    if o.r.touch:
        return
    o.p.open(URL)
    o.p.js("localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full'})); 0")
    o.p.open(URL)
    o.p.settle(1.5)
    geo = """(() => { const s = document.getElementById('start'); const t = s.querySelector('.brand .big').getBoundingClientRect(); const g = s.querySelector('.go button').getBoundingClientRect();
      return { over: s.scrollHeight - s.clientHeight, titleTop: Math.round(t.top), goBottom: Math.round(g.bottom), h: innerHeight }; })()"""
    bad = []
    for n in range(0, 11):
        if n and not o.tap_js("document.querySelectorAll('#start .stepper button')[1]", 0.3):
            bad.append(f"no + at {n}")
            break
        g = o.p.js(geo)
        if g["over"] > 1 or g["titleTop"] < 0 or g["goBottom"] > g["h"]:
            bad.append(f"A{n}: {g}")
        if n in (0, 10):
            o.r.shot(f"start-a{n}")
    o.check("start (desktop): fits the window at ascension 0 to 10, title whole", not bad, "; ".join(bad[:2]))


# ---- WEB64: the game's text, nothing twice --------------------------------------------------------
MADE_UP = ["offers a blessing", "once you have rested", "reward(s) left", "Victory over", "Nothing left to take", "(heal ", "‹", "{", "Next move"]


def screen_text(o):
    return o.p.js("document.querySelector('#screen').innerText") or ""


@scenario
def web64_future_of_potions(o):
    """The Future of Potions: each option says its own potion, rarity and card type (no placeholders), and they differ."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"potions": ["BLOCK_POTION", "FIRE_POTION", "STRENGTH_POTION"], "room": "event:THE_FUTURE_OF_POTIONS"})
    rows = o.p.js("""[...document.querySelectorAll('#screen .list .row')].map((e) => e.innerText.replace(/\\s+/g, ' ').trim())""") or []
    o.check("future of potions: three options", len(rows) == 3, f"{rows}")
    o.check("future of potions: no unfilled placeholder", rows and not any(re.search(r"[‹›{}]", r) for r in rows), f"{rows}")
    o.check("future of potions: options differ and name their potion", len(set(rows)) == len(rows) and all(n in r for n, r in zip(("Block Potion", "Fire Potion", "Strength Potion"), rows)), f"{rows}")
    o.check("future of potions: rarity and type are the game's words", all(re.search(r"Insert (Common|Uncommon|Rare|Event) Potion", r) and re.search(r"Upgraded (Common|Uncommon|Rare) (Attack|Skill|Power)", r) for r in rows), f"{rows}")


@scenario
def web64_game_text(o):
    """Neow, the rest site and the reward screen use the game's words: no made-up lines, no dead Leave row, no repeated heal."""
    o.setup(4, "WEBTEST1")
    t = screen_text(o)
    o.check("neow: the game's epithet, no made-up line", "Mother of Resurrection (Exiled)" in t and not any(m in t for m in MADE_UP), t[:200])
    v = o.view()
    idx = next((i for i, r in enumerate(rows_of(v)) if "Lost Coffer" in plain(r["label"])), None)
    if idx is None:
        raise Skip("no Lost Coffer")
    o.tap(f'#screen .row[data-bid="{rows_of(v)[idx]["id"]}"]', 1.0)
    t = screen_text(o)
    o.check("rewards: no made-up lines", o.view()["decision"] == "rewards" and not any(m in t for m in MADE_UP), t[:200])
    o.to_map(0)
    o.debug({"hp": 40, "room": "rest"})
    t = screen_text(o)
    o.check("rest site: the game's prompt, names and text", "What shall I do?" in t and "Rest" in t and "Heal for 30% of your Max HP" in t and "Upgrade a card in your Deck." in t, t[:300])
    o.check("rest site: no dead Leave row, no made-up text, the heal said once", "Leave" not in t and not any(m in t for m in MADE_UP) and "→" not in t, t[:300])


@scenario
def web64_choose_a_card(o):
    """Lead Paperweight's pick: one title, one skip button."""
    o.setup(4, "WEBTEST1")
    v = o.view()
    idx = next((i for i, r in enumerate(rows_of(v)) if "Lead Paperweight" in plain(r["label"])), None)
    if idx is None:
        raise Skip("no Lead Paperweight")
    o.tap(f'#screen .row[data-bid="{rows_of(v)[idx]["id"]}"]', 1.0)
    t = screen_text(o)
    o.check("choose a card: the title once", t.lower().count("choose a card") == 1, t[:200])
    buttons = [plain(r["label"]) for r in rows_of(o.view()) if not r.get("card")]
    o.check("choose a card: one skip button", len(buttons) == 1 and buttons[0].startswith("Skip"), f"{buttons}")


@scenario
def web64_aim_once(o):
    """Aiming a card shows its damage once on each enemy: the HP bar's -N, no second badge."""
    o.setup(4, "WEBTEST1")
    o.to_map(0)
    spec = {"hp": 300, "encounter": "SLIMES_NORMAL", "cards": ["STRIKE_IRONCLAD", "BASH", "ANGER"]}
    v = o.enter_fight(spec)
    row = next((r for r in o.hand(v) if state(r) == "Enabled" and o.dmg(r) and r["card"]["kind"] == "Attack"), None)
    if not row:
        raise Skip("no attack")
    o.p.click(*o.card_pos(row["id"]))
    o.settle(0.5)
    marks = o.p.js("""[...document.querySelectorAll('#screen .enemy.aim')].map((e) => ({hit: e.querySelectorAll('.hprow .hit').length, badge: e.querySelectorAll('.pvx .pvt').length}))""") or []
    o.check("aim: each enemy shows the damage once (the bar's -N)", marks and all(m["hit"] == 1 and m["badge"] == 0 for m in marks), f"{marks}")


@scenario
def web64_done_no_hint(o):
    """A hand selection turns End Turn into Done: it shows no HP cost then."""
    o.setup(4, "WEBTEST1")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "SLIMES_NORMAL", "cards": ["PURITY"] * 30})
    row = next((r for r in o.hand() if r["card"]["name"].startswith("Purity") and state(r) == "Enabled"), None)
    if not row:
        raise Skip("no Purity")
    hint0 = o.p.js("!!document.querySelector('#dock .primary.endturn .endhint')")
    o.p.click(*o.card_pos(row["id"]))
    o.settle(0.8)
    if not o.p.js("/^Done/.test(document.querySelector('#dock .primary')?.textContent ?? '')"):
        raise Skip("no Done button")
    hint = o.p.js("!!document.querySelector('#dock .primary .endhint')")
    o.check("selection: End Turn's hint is hidden while it reads Done", hint0 and not hint, f"before {hint0}, during {hint}")


@scenario
def web64_removal_copies(o):
    """Card removal lists copies as xN like the Deck sheet."""
    o.setup(4, "WEBTEST1")
    o.to_map(0)
    o.debug({"gold": 900, "room": "shop"})
    rem = next((r for r in rows_of(o.view()) if plain(r["label"]).startswith("Remove")), None)
    if not rem:
        raise Skip("no removal row")
    o.tap_bid(rem["id"])
    if o.view()["decision"] != "card-select":
        raise Skip("no grid")
    badges = o.p.js("""Object.fromEntries([...document.querySelectorAll('#screen .card')].map((c) => [c.querySelector('.g-title, .cname')?.textContent.trim(), c.querySelector('.count')?.textContent]))""") or {}
    o.check("removal grid: Strike and Defend show x4", badges.get("Strike") == "×4" and badges.get("Defend") == "×4", f"{badges}")


# -- WEB67 ---------------------------------------------------------------------------------------
def frame_sheet(o, label, anims_js, times, prep_js=""):
    """A frame sheet: the animations `anims_js` (a JS expression giving a list of Animations) paused and set to each time in
    `times` (ms), one screenshot each, side by side in $OUT/<view>-<label>-frames.png. Returns the path."""
    from PIL import Image, ImageDraw
    import tempfile
    out = os.environ.get("OUT", "target/web32")
    if prep_js:
        o.p.js(prep_js)
    o.p.js(f"(() => {{ window.__ffxA = {anims_js}; for (const a of window.__ffxA) a.pause(); return window.__ffxA.length; }})()")
    shots = []
    with tempfile.TemporaryDirectory(dir=out) as tmp:
        for i, t in enumerate(times):
            o.p.js(f"(() => {{ for (const a of window.__ffxA) a.currentTime = {t}; return 0; }})()")
            time.sleep(0.06)
            path = os.path.join(tmp, f"{i}.png")
            o.p.shot(path)
            shots.append(Image.open(path).convert("RGB"))
        w, h = shots[0].size
        scale = 0.5 if w > 1000 else 1.0
        tw, th = int(w * scale), int(h * scale)
        sheet = Image.new("RGB", (tw * len(shots), th + 28), (16, 17, 22))
        d = ImageDraw.Draw(sheet)
        for i, (im, t) in enumerate(zip(shots, times)):
            sheet.paste(im.resize((tw, th)), (i * tw, 28))
            d.text((i * tw + 8, 8), f"{t} ms", fill=(230, 230, 230))
        dest = os.path.join(out, re.sub(r"[^\w.-]+", "_", f"{o.tag}-{label}-frames") + ".png")
        sheet.save(dest)
    o.p.js("(() => { for (const a of window.__ffxA ?? []) a.play(); window.__ffxA = null; return 0; })()")
    return dest


@scenario
def web67_transform_reveal(o):
    """WEB67: a transform (Wood Carvings' pick) shows the old card turning into the new one, held to be read; a tap skips it and
    presses nothing under it."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 300, "room": "event:WOOD_CARVINGS"})
    v = o.view()
    opts = rows_of(v)
    if v["decision"] != "event" or len(opts) < 3:
        raise Skip("no Wood Carvings")
    d0 = o.deck()
    o.tap_bid(opts[2]["id"])
    grid = [r for r in rows_of(o.view()) if r.get("card")]
    if o.view()["decision"] != "card-select" or len(grid) < 2:
        raise Skip("no transform pick")
    name = grid[1]["card"]["name"]
    o.tap(f'#screen [data-bid="{grid[1]["id"]}"]', 0.6)
    if not o.preview_open():
        raise Skip("no pick preview")
    o.tap("#sheet .pickgo", 0.25)
    shown = o.p.js("""(() => { const r = document.querySelector('.reveal:not(.done)'); if (!r) return null;
        const t = (sel) => r.querySelector(sel + ' .g-title, ' + sel + ' .cname')?.textContent.trim();
        const c = r.querySelector('.rv-card').getBoundingClientRect();
        return { from: t('.rv-old'), to: t('.rv-new'), w: c.width, h: c.height, inside: c.left >= 0 && c.right <= innerWidth && c.top >= 0 && c.bottom <= innerHeight }; })()""")
    dec = o.view()["decision"]
    o.check("transform reveal: shows the card picked turning into another", bool(shown) and shown["from"] == name and shown["to"] and shown["to"] != name,
            f"picked {name}; reveal {shown}")
    if not shown:
        return
    o.check("transform reveal: the card fits the screen, large", shown["inside"] and shown["w"] >= 120, f"{shown}")
    frame_sheet(o, "transform", "document.querySelector('.reveal').getAnimations({subtree: true})", [0, 250, 900, 1400, 1700, 2600, 3900])
    # still showing after 1.5 s (long enough to read), and the new card is the deck's
    o.p.js("(() => { for (const a of document.querySelector('.reveal')?.getAnimations({subtree: true}) ?? []) a.currentTime = 2400; return 0; })()")
    o.settle(0.3)
    still = o.p.js("!!document.querySelector('.reveal:not(.done)')")
    o.check("transform reveal: still up while it is read", still, "")
    # a tap anywhere skips it; nothing under it is pressed
    w, h = o.p.js("[innerWidth, innerHeight]")
    o.p.click(w / 2, h / 2, False)
    o.settle(0.6)
    gone = o.p.js("!document.querySelector('.reveal')")
    o.check("transform reveal: a tap skips it and presses nothing under it", gone and o.view()["decision"] == dec, f"gone {gone}; decision {dec} -> {o.view()['decision']}")
    d = o.delta(d0, o.deck())
    o.check("transform reveal: the card shown is the card gained", d.get(name) == -1 and d.get(shown["to"]) == 1, f"deck change {d}; shown {shown['to']}")


@scenario
def web67_exhaust_burn(o):
    """WEB67: a card exhausted by a choice (Burning Pact's "Exhaust 1 card") burns where it stood, not only a played Exhaust card."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "AXEBOTS_NORMAL", "cards": ["BURNING_PACT"] * 8})
    row = to_hand(o, "Burning Pact")
    if not row:
        raise Skip("Burning Pact never drawn")
    exh0 = o.pile(3) or {}
    o.tap(f'#screen .hand .card[data-bid="{row["id"]}"]', 0.9)
    ch = choice(o)
    if not ch or len(ch) < 2:
        raise Skip(f"no choice {ch}")
    pick = ch[1]
    pname = pick["card"]["name"]
    burnt = None
    taps = [f'#sheet [data-bid="{pick["id"]}"]']
    for i in range(2):
        r = o.r.rect(taps[-1])
        if not r:
            break
        o.p.click(r[0], r[1])
        # hold the burning tile (its timer removes it) before it ends
        burnt = o.p.js("""(() => { const g = document.querySelector('body > .burncard'); if (!g) return null; g.remove = () => {};
            for (const a of g.getAnimations()) a.pause();
            return { name: g.querySelector('.g-title, .cname')?.textContent.trim() }; })()""")
        if burnt:
            break
        prim = o.view()["screen"].get("primary")
        if not (o.view()["screen"].get("sheet") and prim):
            break
        taps.append(f'#dock [data-bid="{prim["id"]}"], #sheet [data-bid="{prim["id"]}"]')
    ev = [e for e in (o.view().get("events") or []) if e.get("kind") == "Exhausted"]
    o.check("exhaust by a choice: the engine's view has the Exhausted event from the hand", any(e.get("from") == "Hand" for e in ev), f"{ev}")
    o.check("exhaust by a choice: the card tapped burns where it stood", bool(burnt) and burnt["name"] == pname, f"tapped {pname}; burning {burnt}")
    if burnt:
        frame_sheet(o, "exhaust", "[...document.querySelectorAll('body > .burncard')].flatMap((g) => g.getAnimations())", [0, 120, 200, 320, 420, 500])
        o.p.js("(() => { for (const g of document.querySelectorAll('body > .burncard')) Element.prototype.remove.call(g); return 0; })()")
    o.settle(0.6)
    got = o.delta(exh0, o.pile(3) or {})
    o.check("exhaust by a choice: the exhaust pile took the card tapped", got == {pname: 1}, f"{got}")


@scenario
def web67_spawn_side(o):
    """WEB67: Fogmog's Eye with Teeth appears where the game puts it: the encounter's slots are ["illusion", "fogmog"]
    (FogmogNormal.cs), the enemies sorted by them (CombatManager.AddCreature), so left of Fogmog. The turn line names it whole."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "FOGMOG_NORMAL"})
    for _ in range(3):
        if len(enemies(o.view())) >= 2:
            break
        o.tap("#dock .fightbar .primary", 2.0)
    es = enemies(o.view())
    if len(es) < 2:
        raise Skip(f"no summon: {[e['name'] for e in es]}")
    o.settle(1.5)
    xs = o.p.js("""[...document.querySelectorAll('#screen .enemy[data-id]')].map((e) => [e.querySelector('.ename, .name, h3')?.textContent.trim() ?? '', e.getBoundingClientRect().left, Number(e.dataset.id)])""")
    order = [e["id"] for e in es]
    dom = [x[2] for x in sorted(xs, key=lambda x: x[1])]
    names = {e["id"]: e["name"] for e in es}
    o.r.shot("spawn")
    o.check("spawn: the enemies stand in the engine's (slot) order, left to right", dom == order, f"engine {[names.get(i) for i in order]}; screen {[names.get(i) for i in dom]}")
    eye = next((e["id"] for e in es if e["name"].startswith("Eye")), None)
    fog = next((e["id"] for e in es if e["name"] == "Fogmog"), None)
    o.check("spawn: Eye with Teeth is left of Fogmog", eye in dom and fog in dom and dom.index(eye) < dom.index(fog), f"{[names.get(i) for i in dom]}")
    said = o.p.js("document.querySelector('#screen .last')?.textContent ?? ''") or ""
    o.check("spawn: the turn line names Eye with Teeth whole", "Eye with Teeth" in said, f"{said[:160]!r}")


@scenario
def web67_chest_gold(o):
    """WEB67: the chest's gold is gained as it opens (OneOffSynchronizer.DoTreasureRoomRewards: PlayerCmd.GainGold, then the gold
    burst; NTreasureRoom.cs) and only the relic is offered. The page says what the chest gave."""
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    g0 = o.gold()
    o.debug({"room": "treasure"})
    v = o.view()
    if v["decision"] != "treasure":
        raise Skip(f"decision {v['decision']}")
    gained = o.gold() - g0
    shown = o.p.js("document.querySelector('#screen .chestgold')?.textContent.trim() ?? null")
    o.check("chest: the gold is gained as it opens, 42-52", 42 <= gained <= 52, f"{g0} -> {o.gold()}")
    o.check("chest: the page shows the gold the chest gave", shown == f"+{gained} Gold" and v.get("chest_gold") == gained, f"shown {shown!r}; view {v.get('chest_gold')}; gained {gained}")
    o.check("chest: no gold row to take (only the relic)", not any(re.search(r"\d+ gold", plain(r["label"]), re.I) for r in rows_of(v)), str([plain(r["label"]) for r in rows_of(v)]))
    o.r.shot("chest")
    rel = [r for r in rows_of(v) if (r.get("info") or {}).get("Relic") or "relic" in plain(r["label"]).lower()]
    if rel:
        # the line sits on the chest's picture; the picture's own place after the take is the room layout's, not this line's
        at = "(() => { const g = document.querySelector('#screen .chestgold'); const p = g?.closest('.picture') ?? document.querySelector('#screen'); if (!g) return null; return [g.getBoundingClientRect().top - p.getBoundingClientRect().top, p.getBoundingClientRect().top]; })()"
        y0 = o.p.js(at)
        o.tap_bid(rel[-1]["id"])
        y1 = o.p.js(at)
        o.check("chest: the gold line keeps its place on the chest after the relic is taken", y0 is not None and y1 is not None and y0[0] == y1[0], f"offset {y0} -> {y1}")
        o.r.shot("chest-taken")
        # WEB68: nothing moves during play: the chest's picture keeps its place when the relic row is claimed
        o.check("chest: the picture moves 0 px after the relic is taken", y0 is not None and y1 is not None and y0[1] == y1[1], f"picture top {y0[1] if y0 else None} -> {y1[1] if y1 else None}")


@scenario
def web68_routebar(o):
    """WEB68: the desktop map shows the room counts once (the legend's chips); the line above them, which named the same
    counts for a hovered room, is gone. The phone keeps it (its legend sits under the map)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.settle(1.0)
    p = o.p
    seen = "(() => { const l = document.querySelector('.planner .routelabel'); return l ? [getComputedStyle(l).display, (l.textContent || '').trim()] : null; })()"
    if not o.r.touch:
        n = p.js("""(() => { const a = [...document.querySelectorAll('#screen g.node.reach[data-at]')][0]; if (!a) return null; const r = a.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2]; })()""")
        if n:
            p.move(n[0], n[1], 3)
            o.settle(0.4)
        v = p.js(seen)
        chips = p.js("document.querySelector('.planner .legend')?.textContent.trim() ?? ''")
        o.r.shot("web68-map-hover")
        o.check("desktop map: no repeated count line above the legend (hovering a room)", v is None or v[0] == "none", f"{v}")
        o.check("desktop map: the legend still shows the counts", bool(re.search(r"\d", chips or "")), f"{chips!r}")
    else:
        o.r.shot("web68-map")


def run(r):
    only = [x for x in os.environ.get("SCEN", "").split(",") if x]
    o = O(r)
    for f in SCENARIOS:
        if only and f.__name__ not in only:
            continue
        if QUICK and f.__name__ in SLOW:  # a quick pass has a minute: the fights won and the long choice runs are the whole test's
            continue
        t0 = time.time()
        r.p.renew()  # a fresh tab per scenario (same saves): one tab for the whole test outgrows Chrome's memory cap
        r.p.open(URL)
        r.p.settle(0.5)
        try:
            f(o)
        except Skip as e:
            r.check(f"scenario {f.__name__} reached its screen", False, str(e))
        except Exception as e:  # a scenario that crashes is a failure, not a skip
            import traceback
            r.check(f"scenario {f.__name__} ran", False, traceback.format_exc()[-300:])
        r.scan()
        print(f"# {f.__name__} {time.time() - t0:.0f}s", flush=True)


# ---- WEB65: audit fixes (docs/reviews/web63-playtest.md findings 1, 3, 5, 6, 11, 19, 20) -------------------------------
def _rects(o, sel, key):
    """{key: [x, y, w, h]} of the elements `sel`, keyed by their attribute `key`."""
    return o.p.js(f"""Object.fromEntries([...document.querySelectorAll({json.dumps(sel)})].map((e) => {{ const r = e.getBoundingClientRect();
        return [e.getAttribute({json.dumps(key)}), [r.left, r.top, r.width, r.height].map(Math.round)]; }}))""") or {}


def _moved(a, b, tol=1.5):
    return {k: (a[k], b[k]) for k in a if k in b and any(abs(x - y) > tol for x, y in zip(a[k], b[k]))}


@scenario
def web65_hand_select(o):
    """Finding 1: a hand selection keeps its cards in place while picking (Gambler's Brew, discard any): the cards picked stay
    where they were, marked."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.enter_fight({"hp": 300, "encounter": "CULTISTS_NORMAL", "potions": ["GAMBLERS_BREW"],
                   "cards": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "BASH", "ANGER", "SHRUG_IT_OFF"] * 2})
    if not potion_do(o, 0, "Use") or not choice(o):
        raise Skip("no hand selection")
    grid = "#sheet .cards .card"
    order = lambda: o.p.js(f"[...document.querySelectorAll('{grid}')].map((c) => [c.dataset.card, c.classList.contains('picked') || c.classList.contains('armed')])")
    first = order()
    seq = []
    for i in (1, 3):
        if i < len(first):
            o.tap_js(f"document.querySelectorAll('{grid}')[{i}]", 0.6)
            seq.append(order())
    o.r.shot("web65-hand-select")
    ids = [c for c, _ in first]
    same = all([c for c, _ in s] == ids for s in seq)
    marked = len(seq) == 2 and [p for _, p in seq[0]] == [j == 1 for j in range(len(ids))] and [p for _, p in seq[1]] == [j in (1, 3) for j in range(len(ids))]
    o.check("hand selection: picked cards keep their place, marked", same and marked, f"before {first}; after taps {seq}")
    o.tap("#dock .primary", 0.8)


@scenario
def web65_map_spacing(o):
    """Finding 5: no two rooms of a map row overlap, as icons or as tap areas."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.settle(1.0)
    bad = o.p.js("""(() => {
      const out = [];
      const nodes = [...document.querySelectorAll('#screen g.node[data-at]')].map((g) => {
        const [x, y] = g.dataset.at.split(',').map(Number);
        const icon = (g.querySelector('image.mi, circle:not(.hit):not(.ring)') ?? g).getBoundingClientRect();
        const hit = g.querySelector('circle.hit')?.getBoundingClientRect();
        return { at: g.dataset.at, x, y, icon, hit };
      });
      const over = (a, b) => Math.min(a.right, b.right) - Math.max(a.left, b.left) > 1 && Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 1;
      const touch = (a, b) => Math.hypot(a.left + a.width / 2 - b.left - b.width / 2, a.top + a.height / 2 - b.top - b.height / 2) < (a.width + b.width) / 2 - 1;
      for (const a of nodes) for (const b of nodes) {
        if (a.at >= b.at || a.y !== b.y) continue;
        if (over(a.icon, b.icon)) out.push(`icons ${a.at}/${b.at}`);
        if (a.hit && b.hit && touch(a.hit, b.hit)) out.push(`taps ${a.at}/${b.at}`);
      }
      return { n: nodes.length, out };
    })()""")
    o.r.shot("web65-map")
    o.check("map: rooms in a row never overlap (icons, tap areas)", bad and bad["n"] > 10 and not bad["out"], f"{bad}")


@scenario
def web65_top_opaque(o):
    """Finding 6: the top bar is opaque; a scrolled screen's text does not show through it."""
    o.setup(0, "WEBTEST1")
    o.to_map(2)
    o.debug({"gold": 900, "room": "shop"})
    if o.r.touch:
        for _ in range(2):
            o.p.drag(o.r.w // 2, o.r.h // 2 + 110, o.r.w // 2, o.r.h // 2 - 110, steps=10)
            o.settle(0.3)
    else:
        o.p.wheel(o.r.w // 2, o.r.h // 2, 100, 3, 10)
    o.settle(0.6)
    if o.r.touch and o.p.js("scrollY") < 40:
        o.p.wheel(o.r.w // 2, o.r.h // 2, 150, 3, 10)  # WebKit on Linux has no touch swipe (wkdrv)
        o.settle(0.6)
    # a screen with no room to scroll (a desktop's shop) passes on the bar's colour alone
    scrolled = o.p.js("document.documentElement.scrollHeight - innerHeight > 40 ? scrollY : 41")
    bg = o.p.js("(() => { const s = getComputedStyle(document.getElementById('top')); return [s.backgroundColor, s.backdropFilter || s.webkitBackdropFilter || 'none']; })()")
    alpha = re.findall(r"[\d.]+", bg[0])
    opaque = len(alpha) == 3 or (len(alpha) == 4 and float(alpha[3]) >= 0.999)
    o.r.shot("web65-top-scrolled")
    o.check("top bar opaque over a scrolled screen", opaque and scrolled > 40, f"{bg}; scrolled {scrolled}px")


@scenario
def web65_slots(o):
    """Finding 11: an enemy killed keeps its slot (faded) and the others stay put; a card played leaves its slot empty and the
    other cards stay put."""
    o.setup(0, "WEBTEST3")
    o.to_map(0)
    v = o.enter_fight({"hp": 300, "encounter": "SLIMES_NORMAL", "cards": ["STRIKE_IRONCLAD"] * 10})
    alive = [e for e in enemies(v) if e["alive"]]
    if len(alive) < 3:
        raise Skip("fewer than 3 enemies")
    # the smallest enemy, which does not split
    target = min(alive, key=lambda e: e["hp"])
    e0 = _rects(o, "#screen .enemy", "data-id")
    for k in range(4):
        v = o.view()
        if not v["fight"] or not any(e["id"] == target["id"] and e["alive"] for e in enemies(v)):
            break
        hand = o.hand(v)
        playable = [r for r in hand if state(r) == "Enabled"]
        if not playable:
            break
        h0 = _rects(o, "#screen .hand .card", "data-card")
        card = playable[0]
        o.tap(f'#screen .hand .card[data-bid="{card["id"]}"]', 0.5)
        if o.p.js("!!document.querySelector('#screen .enemy.aim')"):
            o.tap(f'#screen .enemy[data-id="{target["id"]}"]', 1.2)
        o.settle(1.0)
        h1 = _rects(o, "#screen .hand .card", "data-card")
        if k == 0:
            gone = str(card["card"]["card"]) not in h1
            o.check("hand: a card played leaves a gap, the others stay put", gone and not _moved(h0, h1) and len(h1) == len(h0) - 1, f"moved {_moved(h0, h1)}; {len(h0)} -> {len(h1)} cards")
            o.r.shot("web65-hand-gap")
    v = o.view()
    dead = not any(e["id"] == target["id"] and e["alive"] for e in enemies(v))
    if not dead:
        raise Skip("the enemy did not die")
    o.settle(1.0)
    e1 = _rects(o, "#screen .enemy", "data-id")
    slot = o.p.js(f"!!document.querySelector('#screen .enemy.gone[data-id=\"{target['id']}\"]')") or o.p.js(f"!!document.querySelector('#screen .enemy.dead[data-id=\"{target['id']}\"]')")
    o.r.shot("web65-enemy-dead")
    o.check("enemies: a dead enemy keeps its slot, the others stay put", slot and not _moved(e0, e1) and set(e0) <= set(e1), f"slot kept {slot}; moved {_moved(e0, e1)}; ids {sorted(e0)} -> {sorted(e1)}")


@scenario
def web65_relic_row(o):
    """Finding 19: the relic row stays put when the belt changes (a potion thrown away empties its slot)."""
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.debug({"potions": ["BLOCK_POTION"]})
    r0 = o.p.js("[...document.querySelectorAll('#top .relic')].map((e) => Math.round(e.getBoundingClientRect().left))")
    if not r0:
        raise Skip("no relics")
    o.r.shot("web65-relic-row-potion")
    potion_do(o, 0, "Throw away")
    potion_do_again = o.p.js("!!document.querySelector('#sheet:not([hidden])')")
    if potion_do_again:
        potion_do(o, 0, "Throw away")
    o.r.close_sheet()
    r1 = o.p.js("[...document.querySelectorAll('#top .relic')].map((e) => Math.round(e.getBoundingClientRect().left))")
    empty = all(p == "" for p in o.potions())
    o.check("top bar: the relic row stays put when a potion slot empties", empty and r0 == r1, f"potions {o.potions()}; relics x {r0} -> {r1}")


@scenario
def web65_hand_text(o):
    """Finding 20: a phone's hand cards show their whole text (sized to fit, never under 10px)."""
    if not o.r.touch:
        return
    o.setup(4, "WEBTEST1")
    o.to_map(2)
    o.enter_fight({"hp": 300, "encounter": "CULTISTS_NORMAL", "cards": ["UNLEASH", "REAVE", "BLIGHT_STRIKE", "GRAVE_WARDEN", "PULL_AGGRO", "UNLEASH", "REAVE", "BLIGHT_STRIKE"]})
    o.settle(1.0)
    cut = o.p.js("""[...document.querySelectorAll('#screen .hand .card')].map((c) => { const t = c.querySelector('.ctext'); const f = parseFloat(getComputedStyle(t).fontSize);
        return [c.querySelector('.cname')?.textContent, t.scrollHeight - t.clientHeight, f, c.classList.contains('cut')]; })""")
    bad = [c for c in cut if c[1] > 1 or c[2] < 10 or c[3]]
    o.r.shot("web65-hand-text")
    o.check("hand: every card's text shows whole at 10px or more", cut and not bad, f"cut {bad} of {len(cut)}")


# ---- WEB66: the desktop layout pass ----------------------------------------------------------------------------------
@scenario
def web66_desktop(o):
    """WEB66 (desktop only): an ancient's title, text and choices in one centred column; the map has no column of room
    buttons (its rooms are the choice, by click or by the arrows and Enter), is wide, and keeps Hint; a deck pick's Back
    sits at the header bar's left, on the title's line, aligned with the grid's left edge, and stays in view while the
    grid scrolls."""
    if o.r.touch:
        return
    p = o.p
    o.setup(0, "WEBTEST2")
    o.settle(0.8)
    col = p.js("""(() => { const c = (s) => { const e = document.querySelector(s); if (!e) return null; const r = e.getBoundingClientRect();
        return r.width ? [Math.round(r.left + r.width / 2), Math.round(r.left), Math.round(r.width)] : null; };
        return {w: innerWidth, docw: document.documentElement.clientWidth, h1: c('#screen > .head h1'), text: c('#screen > .story, #screen > .prompt'), list: c('#screen > .list')}; })()""")
    mid = col["docw"] / 2
    ok = col["h1"] and col["text"] and col["list"] and all(abs(x[0] - mid) <= 3 for x in (col["h1"], col["text"], col["list"])) and col["text"][1:] == col["list"][1:]
    o.r.shot("web66-ancient")
    o.check("desktop: the ancient's title, text and choices share one centred column", ok, f"{col}")
    o.to_map(0)
    o.settle(1.0)
    m = p.js("""(() => { const s = document.querySelector('#screen .mapscroll').getBoundingClientRect(); const h = document.querySelector('#askbot');
        const hr = h && h.getBoundingClientRect();
        return {w: innerWidth, map: Math.round(s.width), next: document.querySelectorAll('#dock .next, #dock .nextroom').length,
          hint: !!hr && hr.width > 0 && hr.right <= innerWidth && hr.bottom <= innerHeight, lg: Math.round(document.querySelector('.planner .lg')?.getBoundingClientRect().height ?? 0)}; })()""")
    o.r.shot("web66-map")
    o.check("desktop map: no column of room buttons, the map half the width or more, Hint in view, legend counters 40px+",
            m["next"] == 0 and m["map"] >= 0.5 * m["w"] and m["hint"] and m["lg"] >= 40, f"{m}")
    # the keyboard still travels: an arrow lights a next room, Enter goes there
    p.key("ArrowUp")
    o.settle(0.3)
    lit = p.js("!!document.querySelector('#screen g.node.reach.kbd')")
    p.key("Enter")
    o.settle(2.0)
    v = o.view()
    o.check("desktop map: an arrow lights a next room and Enter travels there", lit and v["decision"] != "map", f"lit {lit}; decision {v['decision']}")
    # a deck pick: Smith on a deck of 10 different cards (two rows: the grid scrolls)
    o.setup(0, "WEBTEST2")
    o.to_map(0)
    o.debug({"room": "rest", "cards": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "BASH", "ANGER", "SHRUG_IT_OFF", "BLUDGEON", "CINDER", "BREAKTHROUGH", "MANGLE", "SETUP_STRIKE"]})
    smith = next((r for r in rows_of(o.view()) if plain(r["label"]).startswith("Smith")), None)
    if not smith:
        raise Skip("no smith")
    o.tap_bid(smith["id"])
    if o.view()["decision"] != "card-select":
        raise Skip("no smith grid")
    geo = """(() => { const R = (s) => { const e = document.querySelector(s); if (!e) return null; const r = e.getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round); };
        const cards = [...document.querySelectorAll('#screen > .list .cards > .card')].map((c) => c.getBoundingClientRect());
        return {back: R('#screen > .head > .chipbtn'), h1: R('#screen > .head > h1'), toggle: R('#screen > .head > .uptoggle'), top: R('#top'),
          grid: cards.length ? [Math.min(...cards.map((r) => r.left)), Math.max(...cards.map((r) => r.right))].map(Math.round) : null, esc: !!document.querySelector('#screen > .head > .chipbtn kbd'),
          prompt: document.querySelector('#screen > .head')?.innerText.replace(/\\s+/g, ' ')}; })()"""
    g = p.js(geo)
    o.r.shot("web66-smith")
    b, h, t, gr = g["back"], g["h1"], g["toggle"], g["grid"]
    ok = b and h and gr and abs(b[0] - gr[0]) <= 3 and b[1] < h[3] and b[3] > h[1] and b[3] - b[1] >= 44 and g["esc"] and (not t or abs(t[2] - gr[1]) <= 3) and "Esc cancels" not in (g["prompt"] or "")
    o.check("desktop pick: Back (with Esc) at the header's left on the title's line, aligned with the grid, the switch at its right", ok, f"{g}")
    p.wheel(o.r.w // 2, o.r.h // 2, 120, 4, 10)
    o.settle(0.6)
    g2 = p.js(geo)
    scrolled = p.js("scrollY")
    o.r.shot("web66-smith-scrolled")
    o.check("desktop pick: the header bar with Back stays in view while the grid scrolls", scrolled > 40 and g2["back"] and g2["back"][1] >= g2["top"][3] - 1 and g2["back"][3] <= o.r.h,
            f"scrolled {scrolled}; back {g['back']} -> {g2['back']}; top bar bottom {g2['top'][3]}")
    o.tap("#screen > .head > .chipbtn", 0.8)
    o.check("desktop pick: Back leaves the grid", o.view()["decision"] == "rest", o.view()["decision"])
