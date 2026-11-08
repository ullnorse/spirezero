"""WEB32 real-input test. One Chrome (CDP port $CDP_PORT) loads the real page from $URL (no ?proof=) and plays
runs with real touch/mouse/wheel/key input at a phone (390x664) and a desktop (1440x900) size. After every action it
compares the DOM with the engine's view (window.sts2.view()) and asserts the layout holds still.
Output: $OUT/results.txt (one line per check), screenshots in $OUT. Exit 1 on any failure.
Usage: realtest.py [phone|desktop]... (both by default)"""
import json, os, re, sys, time, traceback

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
WEBKIT = os.environ.get("BROWSER") == "webkit"
import outcomes
if WEBKIT:
    from wkdrv import Page
else:
    from drv import Page

PORT = int(os.environ.get("CDP_PORT", 9467))
URL = os.environ.get("URL", "http://127.0.0.1:8765/")
OUT = os.environ.get("OUT", "target/web32")
BUDGET = float(os.environ.get("BUDGET", 420))  # seconds per viewport
SEEDS = ["WEBTEST1", "WEBTEST2", "WEBTEST3", "WEBTEST4", "WEBTEST5", "WEBTEST6", "WEBTEST7", "WEBTEST8"]
SIZES = {"phone": (390, 664, True), "desktop": (1440, 900, False), "small": (320, 568, True)}
results = []
VIEWPORT = [None]


def log(status, name, detail=""):
    line = f"{status} {name} {detail}".rstrip()
    results.append(line)
    print(line, flush=True)
    with open(os.path.join(OUT, f"results-{VIEWPORT[0]}.txt"), "a") as f:
        f.write(line + "\n")


def plain(r):
    return "".join(s["text"] for s in (r or []))


def state(b):
    s = b.get("state")
    return s if isinstance(s, str) else next(iter(s))


def blocks(v, kind):
    return [b[kind] for b in v["screen"]["blocks"] if kind in b]


def rows_of(v):
    return [r for l in blocks(v, "List") for r in l["rows"]]


# ---- DOM facts, read in one eval ------------------------------------------------------------------
FACTS = r"""(() => {
  const R = (e) => { if (!e) return null; const r = e.getBoundingClientRect(); return {x: r.left, y: r.top, w: r.width, h: r.height}; };
  const q = (s, r = document) => r.querySelector(s);
  const vis = (e) => { if (!e) return false; const r = e.getBoundingClientRect(); if (r.width < 2 || r.height < 2) return false;
    return r.left >= -1 && r.top >= -1 && r.right <= innerWidth + 1 && r.bottom <= innerHeight + 1; };
  const onTop = (e) => { const r = e.getBoundingClientRect(); const t = document.elementFromPoint(r.left + r.width / 2, r.top + r.height / 2); return !!t && (e === t || e.contains(t)); };
  const f = {w: innerWidth, h: innerHeight, sheetOpen: !!(q('#sheet') && !q('#sheet').hidden), startOpen: !!(q('#start') && !q('#start').hidden)};
  const top = q('#top');
  f.top = {hp: q('.stat.hp b', top)?.textContent, max: q('.stat.hp small', top)?.textContent, gold: q('.stat.gold b', top)?.textContent, floor: q('.stat.floor b', top)?.textContent};
  f.enemies = [...document.querySelectorAll('#screen .enemy:not(.gone)')].map((e) => ({id: e.dataset.id, name: q('.ename', e)?.textContent, hp: q('.hpbar .num', e)?.textContent,
    block: q('.block b', e)?.textContent ?? '0', pw: e.querySelectorAll('.pw').length, ik: e.querySelectorAll('.intent .ik').length, dead: e.classList.contains('dead'),
    rect: R(e), vis: vis(e), top: onTop(e)}));
  const me = q('#screen .you .me');
  f.you = me ? {hp: q('.hpbar .num', me)?.textContent, block: q('.block b', me)?.textContent ?? '0', pw: me.querySelectorAll('.pw').length, rect: R(q('#screen .you')), vis: vis(me), top: onTop(me)} : null;
  f.pets = [...document.querySelectorAll('#screen .pet')].map((e) => ({name: q('.yname', e)?.textContent, hp: q('.hpbar .num', e)?.textContent, block: q('.block b', e)?.textContent ?? '0', dead: e.classList.contains('dead'), vis: vis(e), rect: R(e)}));
  f.hand = [...document.querySelectorAll('#screen .hand .card')].map((e) => ({text: e.textContent, vis: vis(e), rect: R(e), top: onTop(e), cls: e.className}));
  f.handRect = R(q('#screen .hand'));
  f.energy = q('#dock .energy')?.textContent;
  f.piles = [...document.querySelectorAll('#dock .pile b')].map((e) => e.textContent);
  f.end = R(q('#dock .primary'));
  f.dock = R(q('#dock'));
  f.topRect = R(q('#top'));
  f.aim = !!q('#screen .aimbar');
  f.field = R(q('#screen .field'));
  return f;
})()"""


_CTL = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "controls.js")).read()
CONTROL_AT = "(() => {" + _CTL + " return window.__ctlAt(%s, %s); })()"
CONTROLS = "(() => {" + _CTL + " return window.__ctlAll(); })()"
# Why a control is not pressed by the test, by key: the first matching pattern wins (filled in by the list the test prints).
UNTOUCHED_WHY = [
    (r"hintbtn", "why not: the bot's hint overlay (bot.js, own checks); it changes no game state"),
    (r"relic\.pic|pot\.pic", "why not: a top-bar relic or potion out of a fight only opens its sheet; pressed in a fight (potion_use) and by the hold/sheet checks"),
    (r"iconbtn\.close", "pressed in menu_tiles; other sheets close the same way"),
    (r"iconbtn ", "why not: the same top-bar button is pressed where it matters (deck_counts, map sheet, menu_tiles)"),
    (r"\| lg", "why not: the map legend only explains room kinds"),
    (r"\| (me|pet)$", "why not: opens the creature's info sheet; same control as the enemy sheet that is pressed"),
    (r"nextroom", "why not: the map-less route's button, seen once; it sends the same Tap as a map node (map_pick)"),
    (r"primary\.wide Done", "why not: only in up-to-N grids (min < count) that no seed or debug loadout reaches; the fight's Done is pressed (fight_choices)"),
    (r"artrow|me\.glow", "why not: a row in an info sheet only opens that relic's or potion's text (hold/sheet checks)"),
    (r"sheet:Act.*\| (node|routego)", "why not: the map sheet's node and Travel send the same Tap as the map screen's node (map_pick)"),
    (r"row Leave", "why not: disabled until the site is used; Proceed after it is pressed (after_fight)"),
    (r"defeat \|", "why not: the end screen leaves the run (start-screen checks cover what follows)"),
    (r"chipbtn Share seed", "why not: hands off to the browser's share/clipboard, outside the page"),
    (r"tile\.quit", "why not: Save & quit leaves the run; reload_continue covers the save"),
    (r"sheet:.*\| (button|on)$", "why not: a display setting or pile sort (no run state)"),
    (r"sheet:.*\| (card|bigcard)", "why not: a card in a viewing sheet only opens its detail (hold check)"),
    (r"\| (span|row)$", "why not: the inner label of a control that is pressed on that screen"),
    (r"shop \| card", "why not: the same shop tile control, pressed for another card kind (shop_buy)"),
]


def cmp_stable(a, b):
    """Positions of enemies (present in both), the player row, the field and the hand container must not move."""
    bad = []

    def same(r1, r2, tol=1.5):
        return r1 and r2 and all(abs(r1[k] - r2[k]) <= tol for k in "xywh")
    ids = {e["id"]: e for e in a["enemies"] if not e["dead"]}
    for e in b["enemies"]:
        o = ids.get(e["id"])
        if o and not e["dead"] and not same(o["rect"], e["rect"]):
            bad.append(f"enemy {e['name']} {fmt(o['rect'])}->{fmt(e['rect'])}")
    if a["you"] and b["you"] and not same(a["you"]["rect"], b["you"]["rect"]):
        bad.append(f"player row {fmt(a['you']['rect'])}->{fmt(b['you']['rect'])}")
    if a["handRect"] and b["handRect"] and not same(a["handRect"], b["handRect"]):
        bad.append(f"hand {fmt(a['handRect'])}->{fmt(b['handRect'])}")
    if a["field"] and b["field"] and not same(a["field"], b["field"]):
        bad.append(f"field {fmt(a['field'])}->{fmt(b['field'])}")
    return bad


def fmt(r):
    return f"[{r['x']:.0f},{r['y']:.0f} {r['w']:.0f}x{r['h']:.0f}]"


def cmp_view(v, f):
    """What the DOM shows against what the engine says. Returns a list of mismatches."""
    bad = []
    s = v["screen"]
    t = s.get("top")
    if t and not f["startOpen"]:
        if f["top"]["hp"] != str(t["hp"]) or f["top"]["max"] != f"/{t['max_hp']}":
            bad.append(f"top hp dom {f['top']['hp']}{f['top']['max']} engine {t['hp']}/{t['max_hp']}")
        if f["top"]["gold"] != str(t["gold"]):
            bad.append(f"gold dom {f['top']['gold']} engine {t['gold']}")
        if f["top"]["floor"] != str(t["floor"]):
            bad.append(f"floor dom {f['top']['floor']} engine {t['floor']}")
    if not v["fight"]:
        return bad
    en = blocks(v, "Enemy")
    if len(en) != len(f["enemies"]):
        bad.append(f"enemy count dom {len(f['enemies'])} engine {len(en)}")
    # WEB53: no enemy may sit above the top bar's bottom edge, or under the hand or the dock (a crowded fight on a phone
    # once clipped the first row under the bar).
    tb = f.get("topRect")
    for d in f["enemies"]:
        r = d["rect"]
        if tb and r["y"] < tb["y"] + tb["h"] - 1:
            bad.append(f"enemy {d['name']} top {r['y']:.0f} is under the top bar (bottom {tb['y'] + tb['h']:.0f})")
        for what, o in (("hand", f.get("handRect")), ("dock", f.get("dock"))):
            if o and r["y"] + r["h"] > o["y"] + 1.5 and r["y"] < o["y"] + o["h"] and r["x"] < o["x"] + o["w"] and r["x"] + r["w"] > o["x"]:
                bad.append(f"enemy {d['name']} {fmt(r)} is covered by the {what} {fmt(o)}")
    for e in en:
        d = next((x for x in f["enemies"] if x["id"] == str(e["id"])), None)
        if not d:
            bad.append(f"enemy {e['name']} id {e['id']} missing in dom")
            continue
        if d["name"] != e["name"]:
            bad.append(f"enemy name dom {d['name']} engine {e['name']}")
        if d["hp"] != f"{max(0, e['hp'])}/{e['max_hp']}":
            bad.append(f"{e['name']} hp dom {d['hp']} engine {e['hp']}/{e['max_hp']}")
        if d["block"] != str(e.get("block") or 0):
            bad.append(f"{e['name']} block dom {d['block']} engine {e.get('block')}")
        if d["pw"] != len([p for p in (e.get("powers") or []) if p["shown"]]):
            bad.append(f"{e['name']} powers dom {d['pw']} engine {len([p for p in (e.get('powers') or []) if p['shown']])}")
        if e["alive"]:
            want = len([sp for sp in (e.get("intent") or []) if sp["text"].strip()])
            if d["ik"] != want:
                bad.append(f"{e['name']} intents dom {d['ik']} engine {want}")
            if not f["sheetOpen"] and not (d["vis"] and d["top"]):
                bad.append(f"{e['name']} alive but not visible/uncovered {fmt(d['rect'])} vis={d['vis']} top={d['top']}")
        elif not d["dead"]:
            bad.append(f"{e['name']} dead in engine, not in dom")
    you = blocks(v, "You")
    if you and f["you"]:
        y = you[0]
        if f["you"]["hp"] != f"{y['hp']}/{y['max_hp']}":
            bad.append(f"you hp dom {f['you']['hp']} engine {y['hp']}/{y['max_hp']}")
        if f["you"]["block"] != str(y.get("block") or 0):
            bad.append(f"you block dom {f['you']['block']} engine {y.get('block')}")
        if f["you"]["pw"] != len([p for p in (y.get("powers") or []) if p["shown"]]):
            bad.append(f"you powers dom {f['you']['pw']} engine {len([p for p in (y.get('powers') or []) if p['shown']])}")
        if not f["sheetOpen"] and not (f["you"]["vis"] and f["you"]["top"]):
            bad.append("player not visible")
        en_dom = (f["energy"] or "").split("/")[0].strip()
        if en_dom != str(y["energy"]):
            bad.append(f"energy dom {f['energy']} engine {y['energy']}")
        want = [str(y["draw"]), str(y["discard"])] + ([str(y["exhaust"])] if y.get("exhaust") else [])
        if f["piles"][: len(want)] != want:
            bad.append(f"piles dom {f['piles']} engine {want}")
    pets = blocks(v, "Pet")
    if len(pets) != len(f["pets"]):
        bad.append(f"pet count dom {len(f['pets'])} engine {len(pets)}")
    for p, d in zip(pets, f["pets"]):
        if d["hp"] != f"{p['hp']}/{p['max_hp']}":
            bad.append(f"pet {p['name']} hp dom {d['hp']} engine {p['hp']}/{p['max_hp']}")
        if p["alive"] and not f["sheetOpen"] and not d["vis"]:
            bad.append(f"pet {p['name']} alive but not visible")
    hand = [r for r in rows_of(v) if r.get("card")]
    if len(hand) != len(f["hand"]):
        bad.append(f"hand size dom {len(f['hand'])} engine {len(hand)}")
    for r, d in zip(hand, f["hand"]):
        if r["card"]["name"] not in d["text"]:
            bad.append(f"hand card dom '{d['text'][:20]}' engine '{r['card']['name']}'")
        if not f["sheetOpen"] and not d["vis"]:
            bad.append(f"hand card {r['card']['name']} outside the screen {fmt(d['rect'])}")
    return bad


class Run:
    def __init__(self, name):
        self.name = name
        w, h, touch = SIZES[name]
        self.p = Page(PORT, w, h, touch)
        self.w, self.h, self.touch = w, h, touch
        self.cov = set()
        self.t0 = time.time()
        self.shots = 0
        self.touchpads = 0
        self.fails = 0
        self.pressed, self.controls = set(), {}
        real_down = self.p.down

        def down():
            try:
                k = self.p.js(CONTROL_AT % (self.p.mx, self.p.my))
                if k:
                    self.pressed.add(k)
                    if os.environ.get("TRACE_PRESS"):
                        print("press", k, (self.p.mx, self.p.my), flush=True)
            except Exception as e:
                print("press tracking:", e, flush=True)
            real_down()
        self.p.down = down

    # -- plumbing ---------------------------------------------------------------------------------
    def view(self):
        return self.p.js("JSON.stringify(window.sts2 && sts2.view())") and json.loads(self.p.js("JSON.stringify(sts2.view())"))

    def facts(self):
        return self.p.js(FACTS)

    def stable(self, limit=2.5):
        """Facts once the layout has stopped moving (entrance, deal and hit animations are over)."""
        key = lambda f: json.dumps([[e["rect"] for e in f["enemies"]], f["you"] and f["you"]["rect"], f["handRect"], [c["rect"] for c in f["hand"]], f["field"]])
        end = time.time() + limit
        prev = self.facts()
        while time.time() < end:
            time.sleep(0.15)
            cur = self.facts()
            if key(cur) == key(prev):
                return cur
            prev = cur
        return prev

    def shot(self, label):
        path = os.path.join(OUT, re.sub(r"[^\w.-]+", "_", f"{self.name}-{label}") + ".png")
        self.p.shot(path)
        if self.touch and not getattr(self, "in_sideways", False) and not label.endswith("-fail"):
            self.in_sideways = True
            try:
                self.sideways(label)
            finally:
                self.in_sideways = False
        return path

    # The page must not pan sideways on a phone (iOS Safari does so wherever anything is wider than the screen, even
    # under overflow-x: hidden). Run after every screenshot of a screen or sheet.
    WIDE = r"""(() => {
      const de = document.scrollingElement, out = [];
      if (de.scrollWidth > innerWidth + 1) out.push('document ' + de.scrollWidth + '>' + innerWidth);
      for (const e of document.querySelectorAll('body *')) {
        if (e.closest('[hidden]')) continue;
        const cs = getComputedStyle(e);
        if (!/(auto|scroll)/.test(cs.overflowX) || e.scrollWidth <= e.clientWidth + 1) continue;
        if (e.classList.contains('shelf')) continue;  // the tray's chip row scrolls on purpose
        out.push((e.id ? '#' + e.id : e.tagName.toLowerCase()) + '.' + e.className.toString().slice(0, 30) + ' ' + e.scrollWidth + '>' + e.clientWidth);
      }
      return out.slice(0, 6);
    })()"""
    PAN = r"""(() => [scrollX, visualViewport.pageLeft, visualViewport.offsetLeft,
      ...[...document.querySelectorAll('body *')].map((e) => e.scrollLeft).filter(Boolean),
      Math.round((document.querySelector('.map, #screen')?.getBoundingClientRect().left ?? 0) * 10) / 10])()"""

    def sideways(self, label):
        p = self.p
        wide = p.js(self.WIDE) or []
        self.check(f"nothing wider than the screen ({label})", not wide, "; ".join(wide))
        if "map" in label:
            before = p.js(self.PAN)
            y = 330
            p.drag(300, y, 60, y, steps=10)
            p.drag(60, y + 40, 330, y + 40, steps=10)
            after = p.js(self.PAN)
            self.check(f"horizontal swipe moves nothing ({label})", before == after, f"{before} -> {after}")
            # a kind tapped under the map scrolls to its rooms, up or down only, and the bar keeps its height
            bar = "(document.querySelector('.planner .routebar')?.getBoundingClientRect().height)"
            h0 = p.js(bar)
            if self.click_sel(".planner .lg:nth-child(2)"):
                p.settle(0.9)
                self.check(f"a kind tapped: no sideways pan, bar keeps its height ({label})", p.js(self.PAN)[0:1] == before[0:1] and p.js(bar) == h0 and not any(p.js("[...document.querySelectorAll('.map *')].map((e) => e.scrollLeft)")), f"{h0} -> {p.js(bar)}")
                self.click_sel(".planner .lg:nth-child(2)")
                p.settle(0.5)

    def scan(self):
        """Every tappable control on the screen now, for the UNTOUCHED list."""
        try:
            for k in self.p.js(CONTROLS) or []:
                self.controls[k] = True
        except Exception:
            pass

    def untouched(self):
        """Controls seen on a screen and never pressed by real input: UNTOUCHED lines, each with the check to add or the reason."""
        with open(os.path.join(OUT, f"pressed-{self.name}.txt"), "w") as f:
            f.write("\n".join(sorted(self.pressed)) + "\n")
        for k in sorted(set(self.controls) - self.pressed):
            why = next((w for pat, w in UNTOUCHED_WHY if re.search(pat, k)), "add a check")
            log("UNTOUCHED", f"{self.name}:{k}", f"-> {why}")

    def check(self, name, ok, detail=""):
        self.scan()
        if ok:
            log("PASS", f"{self.name}:{name}", detail)
        else:
            self.fails += 1
            log("FAIL", f"{self.name}:{name}", f"{detail} [{self.shot(name.replace(' ', '_')[:40] + '-fail')}]")
        return ok

    def seen(self, what):
        if what not in self.cov:
            self.cov.add(what)
            log("PASS", f"{self.name}:covered {what}", f"t={time.time() - self.t0:.0f}s")

    def left(self):
        return BUDGET - (time.time() - self.t0)

    def rect(self, sel):
        return self.p.js(f"(() => {{ const e = document.querySelector({json.dumps(sel)}); if (!e) return null; if (!e.closest('#dock, #top')) e.scrollIntoView({{block: 'center'}}); const r = e.getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2, r.width, r.height]; }})()")

    def click_sel(self, sel):
        r = self.rect(sel)
        if not r:
            return False
        self.p.click(r[0], r[1])
        return True

    def cmp_rects(self):
        """The WEB53 rectangle mismatches of the screen now (an enemy under the top bar, the hand or the dock)."""
        return [b for b in cmp_view(self.view(), self.stable()) if "top bar" in b or "covered by" in b]

    def verify(self, label):
        """DOM against the engine; logs the first few mismatches as one failure."""
        v = self.view()
        if not v:
            return v
        bad = cmp_view(v, self.stable())
        if bad:
            self.check(f"dom=engine after {label}", False, "; ".join(bad[:4]))
        return v

    # -- start / continue / reload ----------------------------------------------------------------
    def fresh(self, seed, chip=None):
        p = self.p
        p.open(URL)
        p.js("localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full'})); 0")
        p.open(URL)
        p.settle(1.5)
        if self.cov.isdisjoint({"menu music"}):
            # a real tap on the start screen (it unlocks audio), then a pause for the menu's music to start
            self.click_sel("#start .brand")
            p.settle(3.5)
            self.seen("menu music")
            self.early_tracks = p.js("__sound.tracks") or []  # a reload mid-run starts the page's log afresh
        if chip is not None:
            self.click_sel(f"#start .heroes .chip:nth-child({chip})")
        if not self.click_sel("#start input"):
            return self.check("start: seed field", False, "no input")
        p.type_text(seed)
        if not self.click_sel("#start .primary"):
            return self.check("start: Start button", False, "missing")
        p.settle(1.0)
        ok = p.js("!!window.sts2 && !!sts2.view() && document.getElementById('start').hidden")
        self.check("start a run", ok, f"seed {seed}")
        return ok

    def reload_continue(self):
        before = self.snap()
        self.p.open(URL)
        self.p.settle(2.0)
        ok = self.click_sel("#start button.continue")
        self.p.settle(1.0)
        after = self.snap()
        self.check("reload mid-run and continue", ok and before == after, f"before {before} after {after}")
        if ok and before == after and "bug report" not in self.cov:
            self.bug_report_check(before)

    def bug_report_check(self, before):
        """WEB42: Copy bug report (plain http has no clipboard; a phone gets the share sheet), then replay its address."""
        p = self.p
        self.seen("bug report")
        for mode in ("share", "copy") if self.touch else ("copy",):
            p.js("sts2.showLocal(sts2.settingsSheet()); 0")
            p.settle(0.4)
            p.js("window.__shared = null; navigator.share = " + ("async (d) => { window.__shared = d; }" if mode == "share" else "undefined") + "; 0")
            if not self.click_sel("#sheet .bugbox button"):
                return self.check("bug report button", False, "missing")
            p.settle(0.5)
            text = p.js("sts2.lastReport") or ""
            got = p.js("window.__shared?.text") if mode == "share" else (p.js("document.querySelector('#toast')?.textContent") or "")
            self.check(f"bug report {mode}", ("Bug report" in got) if mode == "copy" else got == text, got[:40])
            self.shot(f"bugreport-{mode}")
            self.close_sheet()
        m = re.search(r"replay (\S+)", text)
        self.check("bug report text", bool(m) and all(w in text for w in ("build ", "screen ", "decision ", "seed ", "errors ", "moves ")), text[:60])
        if not m:
            return
        p.open(m.group(1))
        p.settle(2.5)
        after = self.snap()
        self.shot("bugreport-replay")
        self.check("bug report replays to the same place", after == before, f"before {before} after {after}")
        p.open(URL)
        p.settle(2.0)
        self.click_sel("#start button.continue")
        p.settle(1.0)

    def snap(self):
        v = self.view()
        if not v:
            return None
        t = v["screen"]["top"] or {}
        s = {"d": v["decision"], "hp": t.get("hp"), "gold": t.get("gold"), "floor": t.get("floor")}
        if v["fight"]:
            s["hand"] = [r["card"]["name"] for r in rows_of(v) if r.get("card")]
            s["en"] = [(e["name"], e["hp"]) for e in blocks(v, "Enemy")]
        return s

    # -- fight ------------------------------------------------------------------------------------
    def hand_rows(self, v):
        return [r for r in rows_of(v) if r.get("card")]

    def play_one(self, v, mode):
        """Plays the first playable card by `mode` (tap, drag). Returns the new view, or None if nothing was playable."""
        p = self.p
        playable = [r for r in self.hand_rows(v) if state(r) == "Enabled"]
        if not playable:
            return None
        r = playable[0]
        name = r["card"]["name"]
        sel = f'#screen .hand .card[data-bid="{r["id"]}"]'
        before = self.stable()
        pos = self.rect(sel)
        if not pos:
            self.check("hand card has a DOM node", False, name)
            return None
        sig = (before["energy"], tuple(before["piles"]), len(before["hand"]))
        if mode == "drag":
            tgt = next((e for e in before["enemies"] if not e["dead"]), None)
            r2 = tgt["rect"] if tgt else {"x": self.w / 2, "y": 80, "w": 0, "h": 0}
            tx, ty = r2["x"] + r2["w"] / 2, r2["y"] + r2["h"] / 2
            p.drag(pos[0], pos[1], tx, ty, release=False)
            mid = self.facts()
            if not mid["aim"] and v["screen"].get("title") is None:
                pass
            self.check("drag: lit target while dragging", p.js("document.body.classList.contains('dragging')") or p.js("!!document.querySelector('.dragcard')"), name)
            p.up()
            p.settle(0.4)
            self.seen("drag a card")
        else:
            p.click(pos[0], pos[1])
            v2 = self.view()
            after_arm = self.stable(1.5)
            if v2["screen"].get("title"):  # armed: aim at a lit creature
                self.seen("tap card then target")
                bad = cmp_stable(before, after_arm)
                if bad:
                    self.check("layout holds while a card is armed", False, f"{name}: " + "; ".join(bad[:3]))
                aim = self.rect("#screen .enemy.aim, #screen .pet.aim")
                if aim:
                    p.click(aim[0], aim[1])
                else:
                    p.key("Escape")
            else:
                self.seen("tap card without target")
        v3 = self.view()
        f3 = self.stable()
        if not v3 or not v3["fight"]:
            return v3
        alive = lambda f: sorted(e["id"] for e in f["enemies"] if not e["dead"])
        bad = cmp_stable(before, f3) if alive(before) == alive(f3) else []
        if bad:
            self.check("layout holds after playing a card", False, f"{name}: " + "; ".join(bad[:3]))
        self.verify(f"playing {name} ({mode})")
        return v3

    def extras_in_fight(self, v):
        """Once per run: hover, hold, enemy sheet, pile sheets, deck sheet."""
        p = self.p
        f = self.facts()
        if not self.touch and "hover" not in self.cov:
            for e in f["enemies"]:
                if not e["dead"]:
                    r = e["rect"]
                    p.hover(r["x"] + r["w"] / 2, r["y"] + r["h"] / 2)
                    break
            for c in f["hand"][:3]:
                r = c["rect"]
                p.hover(r["x"] + r["w"] / 2, r["y"] + r["h"] / 2)
                bad = cmp_stable(f, self.facts())
                if bad:
                    self.check("layout holds on hover", False, "; ".join(bad[:3]))
            p.hover(self.w / 2, 5)
            self.seen("hover")
        if "hold" not in self.cov and f["hand"]:
            r = f["hand"][0]["rect"]
            p.mx, p.my = r["x"] + r["w"] / 2, r["y"] + r["h"] / 2
            if not self.touch:
                p.move(p.mx, p.my, 2)
            p.down(); time.sleep(0.7); p.up(); p.settle(0.4)
            opened = self.p.js("!!document.querySelector('#sheet:not([hidden])')")
            self.check("hold a card opens its sheet", opened)
            self.shot("hold-card")
            self.close_sheet()
            self.seen("hold")
        if "creature sheet" not in self.cov:
            sel = "#screen .enemy:not(.dead)"
            # WEB57: a sheet only when it adds to the card (the move in more words, an explanation), else nothing opens.
            more = self.p.js("""(() => { const id = document.querySelector('#screen .enemy:not(.dead)')?.dataset.id;
                const e = sts2.view().screen.blocks.flatMap((b) => b.Enemy ? [b.Enemy] : []).find((x) => `${x.id}` === id);
                return !!e && sts2.creatureSheet(e) !== null; })()""")
            if self.click_sel(sel):
                opened = self.p.js("!!document.querySelector('#sheet:not([hidden])')")
                self.check("tap an enemy opens its sheet when it has more to say, else nothing", opened == more, f"sheet {opened}, more {more}")
                self.close_sheet()
                self.seen("creature sheet")
        for pile, sheet in (("draw", 0), ("discard", 1)):
            if f"{pile} pile" in self.cov:
                continue
            v = self.view()
            you = blocks(v, "You")[0]
            n = you[pile]
            if not self.click_sel(f"#dock .pile:nth-of-type({sheet + 1})"):
                continue
            p.settle(0.4)
            total = p.js("[...document.querySelectorAll('#sheet .card')].reduce((a, c) => a + (parseInt((c.querySelector('.count')||{}).textContent?.replace('×','')) || 1), 0)")
            self.check(f"{pile} pile view lists {n} cards", total == n, f"dom {total}")
            self.shot(f"{pile}-pile")
            self.wheel_test(f"{pile} pile sheet")
            self.close_sheet()
            self.seen(f"{pile} pile")

    def fight_turn(self, v, turn):
        p = self.p
        if turn == 0:
            self.seen("fight")
            self.verify("fight start")
            self.extras_in_fight(v)
            v = self.view()
        plays = 0
        while v["fight"] and plays < 14:
            mode = "drag" if (plays + turn) % 4 == 0 else "tap"
            nv = self.play_one(v, mode)
            if nv is None:
                break
            v = nv
            plays += 1
            if v["fight"] and v["screen"].get("sheet"):
                return v  # a card choice (discard, upgrade...) opened: the main loop answers it
            if self.left() < 0:
                return v
        if not v["fight"]:
            return v
        if not self.click_sel("#dock .fightbar .primary"):
            self.check("End Turn button exists", False)
            return v
        self.seen("end turn")
        p.settle(0.5)
        v = self.view()
        self.verify("End Turn")
        return v

    # -- sheets and scrolling ---------------------------------------------------------------------
    def close_sheet(self):
        self.p.key("Escape")
        self.p.settle(0.4)
        if self.p.js("!!document.querySelector('#sheet:not([hidden])')"):
            self.click_sel("#sheet .close, #sheet .x, #sheet button[aria-label='Close']")
            self.p.settle(0.4)

    SCROLLERS = r"""(() => {
      const out = [];
      const cands = [document.scrollingElement, ...document.querySelectorAll('#screen, #sheet, #sheet *, #start, #start *, main *')];
      for (const e of new Set(cands)) {
        if (!e) continue;
        const isDoc = e === document.scrollingElement;
        const cs = getComputedStyle(e);
        if (!isDoc && !/(auto|scroll)/.test(cs.overflowY)) continue;
        if (e.scrollHeight - e.clientHeight < 8) continue;
        const r = isDoc ? {left: 0, top: 0, right: innerWidth, bottom: innerHeight} : e.getBoundingClientRect();
        if (r.right - r.left < 4 || r.bottom - r.top < 4) continue;
        if (!isDoc && e.closest('[hidden]')) continue;
        const x = Math.max(0, r.left) + (Math.min(innerWidth, r.right) - Math.max(0, r.left)) / 2;
        const y = Math.max(0, r.top) + (Math.min(innerHeight, r.bottom) - Math.max(0, r.top)) / 2;
        out.push({doc: isDoc, id: e.id || e.className.toString().slice(0, 30), x, y, range: e.scrollHeight - e.clientHeight});
      }
      return out;
    })()"""

    def scroll_pos(self, c):
        if c["doc"]:
            return self.p.js("[document.scrollingElement.scrollTop, document.scrollingElement.scrollHeight - document.scrollingElement.clientHeight]")
        return self.p.js(f"(() => {{ const e = [...document.querySelectorAll('#sheet *, #screen, #sheet, #start *, main *')].find((e) => (e.id || e.className.toString().slice(0, 30)) === {json.dumps(c['id'])} && e.scrollHeight > e.clientHeight); return e ? [e.scrollTop, e.scrollHeight - e.clientHeight] : [0, 0]; }})()")

    def wheel_test(self, label):
        """Every scrollable area reaches its end with touchpad-sized wheel deltas (4 px at 60 Hz) and mouse notches (100 px)."""
        p = self.p
        for c in p.js(self.SCROLLERS) or []:
            where = f"{label}/{c['id'] or 'page'}"
            kinds = ["notch"]
            if self.touchpads < 3:  # the 4 px / 60 Hz crawl is slow: on the first three scrollers only
                self.touchpads += 1
                kinds = ["touchpad", "notch"]
            for kind in kinds:
                p.js("document.scrollingElement.scrollTop = 0; document.querySelectorAll('#sheet *, #screen, #sheet, main *').forEach((e) => { if (e.scrollTop) e.scrollTop = 0; }); 0")
                p.settle(0.2)
                pos0, rng = self.scroll_pos(c)
                if self.touch and not WEBKIT:  # WebKit on Linux has no touch swipe: its scrollers get the wheel
                    half = 110
                    for _ in range(40):
                        p.drag(c["x"], c["y"] + half, c["x"], c["y"] - half, steps=10)
                        p.settle(0.25)
                        pos, rng = self.scroll_pos(c)
                        if pos >= rng - 2:
                            break
                    self.check(f"scroll reaches the end ({where}, swipe)", pos >= rng - 2, f"{pos:.0f}/{rng:.0f}")
                    break
                delta, hz = (4, 60) if kind == "touchpad" else (100, 10)
                n = 40 if kind == "touchpad" else 6
                p.wheel(c["x"], c["y"], delta, n, hz)
                pos1, _ = self.scroll_pos(c)
                moved_ok = pos1 >= min(rng, n * delta * 0.5)
                self.check(f"{kind} wheel moves ({where})", moved_ok, f"{pos1:.0f}px after {n} events of {delta}px")
                for _ in range(int(rng / delta) + 20):
                    p.call("Input.dispatchMouseEvent", type="mouseWheel", x=c["x"], y=c["y"], deltaX=0, deltaY=delta)
                    if _ % 50 == 0:
                        pos, rng = self.scroll_pos(c)
                        if pos >= rng - 2:
                            break
                p.settle(0.4)
                pos, rng = self.scroll_pos(c)
                self.check(f"{kind} wheel reaches the end ({where})", pos >= rng - 2, f"{pos:.0f}/{rng:.0f}")
            # the last thing on the page must not hide under the dock
            if c["doc"] and not self.p.js("document.getElementById('sheet') && !document.getElementById('sheet').hidden"):
                ok = p.js("""(() => { const k = [...document.querySelectorAll('#screen > *')].filter((e) => e.getBoundingClientRect().height > 2).pop();
                  const d = document.getElementById('dock'); if (!k) return true; const lim = d.hidden ? innerHeight : d.getBoundingClientRect().top; return k.getBoundingClientRect().bottom <= lim + 1; })()""")
                self.check(f"last content clears the dock ({where})", ok)
            self.shot("scroll-" + label.replace(" ", "_"))

    def sheets_tour(self):
        """Deck, map, menu/settings opened by the top bar's real buttons."""
        p = self.p
        for name, sel in (("deck", "#top .iconbtn:nth-of-type(1)"), ("map", "#top .iconbtn:nth-of-type(2)"), ("menu", "#top .iconbtn:nth-of-type(3)")):
            if f"{name} sheet" in self.cov:
                continue
            if not self.click_sel(sel):
                continue
            p.settle(0.5)
            opened = p.js("!!document.querySelector('#sheet:not([hidden])')")
            self.check(f"{name} sheet opens", opened)
            if opened:
                self.shot(f"{name}-sheet")
                if name == "deck":
                    n = p.js("[...document.querySelectorAll('#sheet .card')].reduce((a, c) => a + (parseInt((c.querySelector('.count')||{}).textContent?.replace('×','')) || 1), 0)")
                    v = self.view()
                    self.check("deck sheet card count", n > 0, f"{n} cards")
                self.wheel_test(f"{name} sheet")
                if name == "menu":
                    self.settings_tour()
            self.close_sheet()
            self.check(f"{name} sheet closes", not p.js("!!document.querySelector('#sheet:not([hidden])')"))
            self.seen(f"{name} sheet")

    def settings_tour(self):
        p = self.p
        tiles = p.js("[...document.querySelectorAll('#sheet .tile')].map((t) => t.textContent.trim())") or []
        if "settings" in self.cov:
            return
        idx = next((i for i, t in enumerate(tiles) if "etting" in t or "ption" in t), None)
        if idx is None:
            self.check("menu has a settings tile", False, str(tiles))
            return
        self.click_sel(f"#sheet .tile:nth-child({idx + 1})")
        p.settle(0.5)
        self.shot("settings")
        self.check("settings sheet opens", p.js("!!document.querySelector('#sheet:not([hidden])')"), str(tiles))
        self.wheel_test("settings")
        self.seen("settings")

    # -- rooms ------------------------------------------------------------------------------------
    WANT = ["Shop", "Rest", "Treasure", "Unknown", "Event", "Monster", "Elite"]

    def choose_row(self, v, rows):
        d = v["decision"]
        label = lambda r: plain(r["label"])
        ok = [r for r in rows if state(r) in ("Enabled", "Selected")]
        if not ok:
            return None
        if d == "map":
            log("INFO", f"{self.name}:map choices floor {v['save']['floor']}", str([plain(r["label"]) for r in ok]))
            def rank(r):
                t = label(r)
                for i, w in enumerate(self.WANT):
                    if w.lower() in t.lower():
                        miss = {"Shop": "shop", "Rest": "rest", "Treasure": "treasure", "Unknown": "event", "Event": "event"}.get(w)
                        return (0 if miss and miss not in self.cov else 1, 0 if w != "Elite" else 5, i)
                return (2, 0, 9)
            return sorted(ok, key=rank)[0]
        if d == "shop":
            buy = [r for r in ok if "leave" not in label(r).lower() and "proceed" not in label(r).lower() and "back" not in label(r).lower()]
            if "bought" not in self.cov and buy:
                self.seen("bought")
                return buy[0]
            # "Leave the shop" is the screen's primary (the dock), not a list row: with nothing to buy, fall through to it
            return next((r for r in ok if re_any(label(r), ("leave", "proceed", "back"))), None)
        if d == "rest":
            return next((r for r in ok if "rest" in label(r).lower() or "heal" in label(r).lower()), ok[0])
        skip = [r for r in ok if not re_any(label(r), ("skip", "leave", "proceed"))]
        return (skip or ok)[0]

    def deck_counts(self):
        """Deck sheet opened by its real button: card name -> copies."""
        self.click_sel("#top .iconbtn:nth-of-type(1)")
        self.p.settle(0.5)
        got = self.p.js("""Object.fromEntries([...document.querySelectorAll('#sheet .cards .card')].map((c) =>
            [c.querySelector('.g-title, .cname').textContent.trim(), Number(c.querySelector('.count')?.textContent.replace('×', '') ?? 1)]))""")
        self.close_sheet()
        self.p.settle(0.3)
        return got

    def reward_as_tapped(self):
        """The last tile of a card reward, by real input: that card is the one the deck gains."""
        p = self.p
        names = p.js("[...document.querySelectorAll('.peekreward .card')].map((c) => c.querySelector('.g-title, .cname').textContent.trim())")
        if not names or len(set(names)) < 2 or names[-1] == names[0]:
            return False
        if p.js("!!document.querySelector('#screen .list .row:not(:disabled)')"):
            # Claiming a gold, potion or relic row must not move the cards.
            where = lambda: p.js("(() => { scrollTo(0, 0); const r = document.querySelector('.peekreward .card:last-child').getBoundingClientRect(); return [Math.round(r.left), Math.round(r.top)]; })()")
            at = where()
            if self.click_sel("#screen .list .row:not(:disabled)"):
                p.settle(0.6)
                if p.js("!!document.querySelector('.peekreward .card')"):
                    self.check("claim leaves the reward cards in place", where() == at, f"{at} -> {where()}")
        before = self.deck_counts()
        if not self.click_sel(".peekreward .card:last-child"):
            return False
        p.settle(0.6)
        after = self.deck_counts()
        want = names[-1]
        self.check("reward card as tapped", after.get(want, 0) == before.get(want, 0) + 1, f"tapped {want} of {names}; deck before {before} after {after}")
        self.seen("reward card as tapped")
        return True

    def room_step(self, v):
        p = self.p
        d = v["decision"]
        s = v["screen"]
        self.seen(f"decision:{d}")
        if p.js("!!document.querySelector('#sheet:not([hidden]) .pickgo')"):
            # WEB65: a deck pick's preview: Confirm
            self.seen("deck pick preview")
            self.click_sel("#sheet .pickgo")
            p.settle(0.5)
            return
        if v.get("sheet") or p.js("!!document.querySelector('#sheet:not([hidden])')"):
            self.close_sheet()
            return
        rows = rows_of(v)
        key = (d, v["save"]["floor"])
        if key not in self.toured:
            self.toured.add(key)
            self.verify(f"entering {d}")
            if d not in ("map",) or "map-shot" not in self.cov:
                self.shot(f"room-{d}")
                if d == "map":
                    self.seen("map-shot")
            if d not in self.scrolled:
                self.scrolled.add(d)
                self.wheel_test(f"{d} screen")
        if d == "rewards" and "reward card as tapped" not in self.cov and self.reward_as_tapped():
            return
        if d == "rewards":
            # Rewards are taken as the player does: the gold, potion and relic rows, then a card by its tile (no row repeats the cards).
            rest = [r for r in rows if not plain(r["label"]).startswith("Card reward")]
            row = self.choose_row(v, rest)
            if row is not None and self.click_sel(f'#screen [data-bid="{row["id"]}"]'):
                p.settle(0.3)
                return
            n = p.js("document.querySelectorAll('.peekreward .card').length") or 0
            if n:
                self.tile_pick = getattr(self, "tile_pick", 0) + 1
                if self.click_sel(f".peekreward .card:nth-child({self.tile_pick % n + 1})"):
                    p.settle(0.6)
                    return
        row = self.choose_row(v, rows)
        if row is not None:
            sel = f'#screen [data-bid="{row["id"]}"], #dock [data-bid="{row["id"]}"], #screen g.node[data-button="{row["id"]}"]'
            if self.click_sel(sel):
                p.settle(0.3)
                return
        prim = s.get("primary")
        if prim and state(prim) != "Disabled":
            if self.click_sel(f'#dock [data-bid="{prim["id"]}"], #screen [data-bid="{prim["id"]}"]'):
                return
        if self.click_sel("#screen .head .chipbtn"):
            return
        p.key("Escape")
        self.stuck += 1

    def run_once(self, seed, chip):
        self.p.renew()  # a fresh tab per run: one tab for the whole test outgrows Chrome's memory cap
        if not self.fresh(seed, chip):
            return
        p = self.p
        self.toured, self.scrolled, self.stuck = set(), set(), 0
        turn, steps, last = 0, 0, None
        reloaded = "reload mid-run and continue" in self.cov
        while self.left() > 0 and steps < 900:
            steps += 1
            v = self.view()
            if v is None:
                self.check("page alive", False, "no sts2.view")
                return
            if v["over"] or v["decision"] in ("defeat", "victory"):
                self.seen(f"end:{v['decision']}")
                self.shot(f"end-{v['decision']}")
                return
            sig = json.dumps([v["decision"], v["save"]["floor"], self.snap()])
            self.stuck = self.stuck + 1 if sig == last else 0
            last = sig
            if self.stuck > 6:
                dbg = self.p.js("JSON.stringify({sheet: document.querySelector('#sheet')?.hidden, sheetText: document.querySelector('#sheet')?.innerText.slice(0,200), banner: document.querySelector('#banner')?.className, kind: sts2.view().sheet_kind, rows: [...document.querySelectorAll('#screen [data-bid], #dock [data-bid]')].map(e=>e.dataset.bid+':'+e.className.toString().slice(0,20)).slice(0,10), nodes: document.querySelectorAll('g.node.reach').length, blocks: sts2.view().screen.blocks.map(b=>Object.keys(b)[0]), cover: (()=>{const n=document.querySelector('g.node.reach'); if(!n) return null; const r=n.getBoundingClientRect(); const t=document.elementFromPoint(r.left+r.width/2, r.top+r.height/2); return [Math.round(r.left),Math.round(r.top),Math.round(r.width), t&&(t.tagName+'.'+t.className.toString().slice(0,30)+'#'+t.id)];})(), text: document.querySelector('#screen')?.innerText.slice(0,200)})")
                self.check("run advances", False, f"stuck on {v['decision']} floor {v['save']['floor']} rows={[(r['id'], plain(r['label'])[:14], state(r)) for r in rows_of(v)][:6]} dom={dbg}")
                return
            if v["fight"] and not (v["screen"].get("sheet")):
                if not reloaded and turn == 1:
                    reloaded = True
                    self.reload_continue()
                    self.seen("reload mid-run and continue")
                    continue
                v = self.fight_turn(v, turn)
                turn += 1
                if v and not v["fight"]:
                    turn = 0
                    self.seen("fight won")
                    self.fight_cues()
                    self.sheets_tour()
            elif v["fight"]:
                self.seen("fight card choice")
                sheet = next(iter(v["screen"]["sheet"].values()))
                rows = sheet.get("rows") or sheet.get("buttons") or []
                prim = v["screen"].get("primary")
                chosen = [r for r in rows if state(r) == "Picked"]
                ok = [r for r in rows if state(r) not in ("Disabled",)]
                tried = (self.click_sel(f'#sheet [data-bid="{prim["id"]}"], #dock [data-bid="{prim["id"]}"]') if chosen and prim else False)
                if not tried:
                    pick = (ok or rows or [None])[0]
                    if not pick or not self.click_sel(f'#sheet [data-bid="{pick["id"]}"]'):
                        self.p.key("Escape")
                        self.stuck += 1
            else:
                turn = 0
                self.room_step(v)

    def fight_cues(self):
        """The cues played since the last fight ended, one line per fight (sound-<name>.txt)."""
        n = self.p.js("__sound.log.length")
        cues = self.p.js(f"__sound.log.slice({getattr(self, 'cue_at', 0)})")
        self.cue_at = n
        self.fight_no = getattr(self, "fight_no", 0) + 1
        with open(os.path.join(OUT, f"sound-{self.name}.txt"), "a") as f:
            f.write(f"fight {self.fight_no}: " + " ".join(cues) + "\n")

    def sound_checks(self):
        st = self.p.js("JSON.stringify(__sound)")
        st = json.loads(st) if st else {"log": [], "missing": [], "tracks": []}
        played = {c.split(":", 1)[0] for c in st["log"]}
        with open(os.path.join(OUT, f"sound-{self.name}.txt"), "a") as f:
            f.write(f"bytes {st['bytes']} before first sound {st.get('bytesBeforeFirst')}; tracks {st['tracks']}; missing {st['missing']}\n")
        self.check("sound: no missing keys", not st["missing"], str(st["missing"][:4]))
        # A quick pass may not reach a win, so the cues are checked only by the whole test.
        for cue in [] if os.environ.get("QUICK") else ["click", "hover", "play", "hit", "block", "victory", "draw", "turn"]:
            self.check(f"sound: {cue} cue played", cue in played, "")
        tracks = " ".join(getattr(self, "early_tracks", []) + st["tracks"])
        self.check("sound: menu and fight music", "menu" in tracks and "_enemy" in tracks, tracks[:80])

    def finish(self):
        self.untouched()
        need = ["fight", "end turn", "drag a card", "decision:map", "decision:rewards", "reward card as tapped", "decision:shop", "decision:rest", "decision:event",
                "reload mid-run and continue", "deck sheet", "menu sheet", "settings", "draw pile", "discard pile", "hold"]
        need += ["tap card without target", "tap card then target"]
        if not self.touch:
            need.append("hover")
        # A deploy's quick pass plays for a minute and can't reach every screen: workers run the whole test.
        for n in [] if os.environ.get("QUICK") or os.environ.get("ONLY_OUTCOMES") else need:
            if n not in self.cov:
                self.check(f"coverage {n}", False, "never reached")
        errs = self.p.errors()
        self.check("no console errors", not errs, "; ".join(errs[:3]))
        return self.fails


def re_any(text, words):
    t = text.lower()
    return any(w in t for w in words)


def main():
    os.makedirs(OUT, exist_ok=True)
    names = sys.argv[1:] or ["phone"]
    VIEWPORT[0] = names[0]
    open(os.path.join(OUT, f"results-{names[0]}.txt"), "w").close()
    fails = 0
    t0 = time.time()
    for n in names:
        r = Run(n)
        try:
            for i, seed in enumerate(SEEDS):
                if r.left() <= 0 or os.environ.get("ONLY_OUTCOMES"):
                    break
                r.run_once(seed, [4, 1, 2, 3][i % 4] if i else None)
                if r.cov >= {"decision:shop", "decision:rest", "decision:event", "decision:rewards", "fight"} and "reload mid-run and continue" in r.cov and "settings" in r.cov and "hold" in r.cov:
                    break
        except Exception:
            r.check("test crashed", False, traceback.format_exc()[-400:])
        if not os.environ.get("ONLY_OUTCOMES"):
            r.sound_checks()  # the page's sound log is the main run's: the outcome scenarios reload the page
        try:
            outcomes.run(r)
        except Exception:
            r.check("outcome checks crashed", False, traceback.format_exc()[-400:])
        fails += r.finish()
    log("INFO", "runtime", f"{time.time() - t0:.0f}s, {fails} failures")
    sys.exit(1 if fails else 0)


main()
