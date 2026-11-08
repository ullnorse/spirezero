// Headless proof (?proof=<scenario>&seed=S&char=N): plays the run by clicking the page's own
// buttons, as a thumb would, until the scenario's screen is up; then sets the title to
// `proof:<scenario>:<outcome>` for the screenshot. Not loaded in normal use.
const q = new URLSearchParams(location.search);
const scenario = q.get("proof");
const S = window.sts2;
const log = [];
const seen = new Set();

const $$ = (sel) => [...document.querySelectorAll(sel)];
const trail = [];
const click = (e) => {
  if (!e) return false;
  const sv = v()?.screen;
  trail.push(`${e.className.split(" ").slice(0, 2).join(".")}:${(e.textContent ?? "").slice(0, 10)} e${sv?.blocks.find((b) => b.You)?.You.energy} s${sv?.sheet ? 1 : 0} h${sv?.top?.hp} L${(sv?.blocks.find((b) => b.Last)?.Last ?? []).map((x) => x.text).join("").slice(0, 30)} n${S.view().save.actions.length}`);
  if (trail.length > 14) trail.shift();
  e.click();
  return true;
};
const v = () => S.view();
const blocks = (n, kind) => n.screen.blocks.filter((b) => b[kind]).map((b) => b[kind]);
const text = (e) => e?.textContent ?? "";

// A thumb's tap on an element: pointerdown with a touch pointer, then the click.
function touchTap(e) {
  e.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, pointerType: "touch", isPrimary: true }));
  e.dispatchEvent(new MouseEvent("click", { bubbles: true }));
}

// A thumb dragging `from` onto `to` (an element, or a function finding it once the drag has
// redrawn the screen; or back down, `back`); `hold` stops before letting go.
function pointerDrag(from, to, back = false, hold = false) {
  const r = from.getBoundingClientRect();
  const x0 = r.left + r.width / 2;
  const y0 = r.top + r.height / 2;
  const fire = (target, type, x, y) => target.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: y, pointerId: 1, isPrimary: true }));
  fire(from, "pointerdown", x0, y0);
  fire(window, "pointermove", x0, y0 - 30);
  let x = x0;
  let y = y0 + 4;
  if (!back) {
    const t = (typeof to === "function" ? to() : to).getBoundingClientRect();
    x = t.left + t.width / 2;
    y = t.top + t.height / 2;
  }
  fire(window, "pointermove", x, y);
  if (!hold) fire(window, "pointerup", x, y);
}

// A thumb pulling the open sheet's head `dy` px down (up when negative): `ms` of waiting before
// letting go makes it a slow pull (no fling), 0 a fast one.
async function swipeHead(dy, ms) {
  const head = document.querySelector("#sheet .sheethead");
  const r = head.getBoundingClientRect();
  const x = r.left + r.width / 3;
  const y = r.top + r.height / 2;
  const fire = (target, type, yy) => target.dispatchEvent(new PointerEvent(type, { bubbles: true, clientX: x, clientY: yy, pointerId: 2, isPrimary: true, pointerType: "touch" }));
  fire(head, "pointerdown", y);
  fire(window, "pointermove", y + dy / 2);
  fire(window, "pointermove", y + dy);
  await new Promise((r) => setTimeout(r, ms));
  fire(window, "pointerup", y + dy);
  await new Promise((r) => setTimeout(r, 450));
}

function fightStep() {
  const choice = $$("#sheet .cards .card:not(.armed):not(.unplayable):not(.picked)");
  if (document.querySelector("#sheet.pinned:not([hidden])")) {
    const done = document.querySelector("#dock .primary:not(:disabled)");
    if (choice.length && !document.querySelector("#sheet .cards .card.armed")) return click(choice[0]);
    return click(done) || click(choice[0]);
  }
  const aims = $$("#screen .enemy.aim");
  if (aims.length) {
    const hp = (e) => Number(text(e.querySelector(".num")).split("/")[0]);
    aims.sort((a, b) => hp(a) - hp(b));
    return click(aims[0]);
  }
  const hand = $$("#screen .hand .card:not(.unplayable)");
  const inc = v().screen.blocks.find((b) => b.Incoming)?.Incoming;
  const blockFirst = inc && inc.hp_loss > 4;
  const rank = (c) => {
    const k = c.classList.contains("k-attack") ? 0 : c.classList.contains("k-power") ? 1 : 2;
    const blocks = /Block/.test(text(c));
    if (blockFirst && blocks) return -1;
    return k;
  };
  hand.sort((a, b) => rank(a) - rank(b));
  if (hand.length) return click(hand[0]);
  return click(document.querySelector("#dock .endturn:not(:disabled)"));
}

function roomStep(goal) {
  const d = v().decision;
  if (d === "map") {
    const grid = v().screen.blocks.find((b) => b.Map)?.Map;
    // The next rooms: a tap on the map's node (a phone has no dock buttons), else the dock's.
    const node = (id) => {
      const c = grid?.cells.find((x) => x.button === id);
      return c && document.querySelector(`#screen g.node[data-at="${c.x},${c.y}"]`);
    };
    const docked = $$("#dock .nextroom");
    const rows = (v().screen.blocks.find((b) => b.List)?.List.rows ?? []).filter((r) => node(r.id) || docked.some((d) => d.dataset.bid == r.id));
    const rooms = rows.map((r) => node(r.id) ?? docked.find((d) => d.dataset.bid == r.id));
    // Head for the nearest room of the goal's kind; else keep off elites.
    const dist = (id) => {
      if (!goal || !grid) return 99;
      const start = grid.cells.find((c) => c.button === id);
      const re = new RegExp(goal, "i");
      let frontier = start ? [start] : [];
      for (let d = 0; d < 20 && frontier.length; d++) {
        if (frontier.some((c) => re.test(c.room))) return d;
        const next = [];
        for (const c of frontier)
          for (const e of grid.edges)
            if (e.from[0] === c.x && e.from[1] === c.y) {
              const n = grid.cells.find((x) => x.x === e.to[0] && x.y === e.to[1]);
              if (n) next.push(n);
            }
        frontier = next;
      }
      return 99;
    };
    const scored = rooms.map((r, i) => ({ r, d: dist(rows[i]?.id), elite: /Elite/.test(rows[i].label.map((s) => s.text).join("")) }));
    scored.sort((a, b) => a.d - b.d || a.elite - b.elite);
    const pick = scored[0]?.r;
    if (!pick) return false;
    if (pick instanceof SVGElement) {
      touchTap(pick);
      return true;
    }
    return click(pick);
  }
  const asking = document.querySelector("#screen .row.s-picked, #screen .card.asking, #dock .primary.s-picked");
  if (asking) return click(asking);
  if (d === "rewards") {
    const card = document.querySelector("#screen .cards:not(.peek) .card");
    if (card) return click(card);
    const rows = $$("#screen .row:not(:disabled):not(.s-dim)").filter((r) => !/Skip/.test(text(r)));
    return click(rows[0]) || click(document.querySelector("#dock .primary"));
  }
  if (d === "shop") return click(document.querySelector("#dock .primary")) || click($$("#screen .row").at(-1));
  const tiles = $$("#screen .cards .card:not(.unplayable):not(.picked)");
  const rows = $$("#screen .row:not(:disabled):not(.s-dim)");
  if (d === "card-select") return click(tiles[0] ?? rows[0]) && (click(document.querySelector("#dock .primary")) || true);
  return click(rows[0] ?? tiles[0]) || click(document.querySelector("#dock .primary"));
}

// Plays until `until()` holds; `goal` is the room kind the map prefers.
function play(until, goal = null, max = 1500) {
  for (let i = 0; i < max; i++) {
    const now = v();
    seen.add(now.decision);
    if (until(now)) return true;
    if (now.over) return false;
    const before = JSON.stringify(now.screen);
    const acted = now.fight ? fightStep() : roomStep(goal);
    if (q.has("trail")) {
      const after = v().screen;
      const diff = Object.keys(after).filter((k) => JSON.stringify(after[k]) !== JSON.stringify(now.screen[k]));
      trail.push(`sk:${v().sheet_kind} pots:${JSON.stringify(after.top.potions.map((p) => p.selected))} diff:${diff.join(",")} st:${JSON.stringify(after.status)?.slice(0, 60)} mode:${JSON.stringify(after.title)?.slice(0, 50)}`);
    }
    if (!acted || JSON.stringify(v().screen) === before) {
      // Unstuck: the main button, else the first row.
      click(document.querySelector("#dock .primary:not(:disabled)")) || click(document.querySelector("#screen .row"));
    }
  }
  return false;
}

const inFight = (n) => n.fight && !n.screen.sheet;
const SCENARIOS = {
  start() {
    localStorage.setItem("sts2-save-v1", JSON.stringify({ character: 4, seed: "PROOF", ascension: 0, actions: [0], floor: 7, hp: 48, max_hp: 66 }));
    S.showStart();
    return "start";
  },
  neow: () => play((n) => n.decision === "ancient"),
  fight() {
    play(inFight);
    let plays = 0;
    play(() => plays++ >= 2 || !v().fight);
    return v().fight;
  },
  target() {
    play((n) => inFight(n) && n.screen.blocks.filter((b) => b.Enemy).length > 1, "Monster");
    for (let i = 0; i < 30 && !document.querySelector(".enemy.aim"); i++) {
      const atk = document.querySelector("#screen .hand .card.k-attack:not(.unplayable)");
      if (!atk) break;
      click(atk);
    }
    return !!document.querySelector(".enemy.aim");
  },
  // Hold a hand card: its full text and the game's keywords, as a sheet.
  hold() {
    play(inFight);
    const card = $$("#screen .hand .card").find((c) => c.querySelector(".ctext .r-Gold, .ctext .r-Energy, .ctext .r-Buff, .ctext .r-Debuff")) ?? $$("#screen .hand .card")[0];
    card.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 5, clientY: 5 }));
    return new Promise((done) => setTimeout(() => done(!!document.querySelector("#sheet .bigcard")), 600));
  },
  // Play feel with animation on: an aimed card's ghost flies to the enemy, which flashes; the
  // energy orb squeezes. Outcome `fly:hit:spent`.
  feel() {
    play((n) => inFight(n) && blocks(n, "List").some((l) => l.rows.some((r) => r.card?.kind === "Attack" && r.state === "Enabled")));
    document.body.classList.remove("proof");
    click($$("#screen .hand .card.k-attack:not(.unplayable)")[0]);
    click(document.querySelector("#screen .enemy.aim"));
    const fly = !!document.querySelector(".flycard");
    return new Promise((done) => setTimeout(() => done(`${fly}:${!!document.querySelector(".enemy.hit")}:${!!document.querySelector(".energy.spent")}`), 80));
  },
  // Drag to play (outcome `<attack>:<skill>:<cancel>`): an attack dropped on an enemy, a skill
  // dropped over the hand, a card dragged back down stays. `dragging` holds an attack over an
  // enemy for the screenshot.
  drag() {
    play((n) => inFight(n) && blocks(n, "List").some((l) => l.rows.filter((r) => r.state === "Enabled").length >= 3));
    const cards = () => $$("#screen .hand .card:not(.unplayable)");
    const handSize = () => $$("#screen .hand .card").length;
    const n0 = handSize();
    const cancel = cards()[0];
    pointerDrag(cancel, cancel, true);
    const kept = handSize() === n0 && !v().screen.title;
    const atk = cards().find((c) => c.classList.contains("k-attack"));
    const foe = () => document.querySelector("#screen .enemy:not(.dead)");
    const hp0 = JSON.stringify(blocks(v(), "Enemy").map((e) => e.hp));
    if (atk) pointerDrag(atk, foe);
    const hit = JSON.stringify(blocks(v(), "Enemy").map((e) => e.hp)) !== hp0;
    const n1 = handSize();
    const skill = cards().find((c) => !c.classList.contains("k-attack"));
    if (skill) pointerDrag(skill, () => document.querySelector("#screen .field"));
    return `${atk ? hit : "none"}:${skill ? handSize() < n1 : "none"}:${kept}`;
  },
  dragging() {
    play((n) => inFight(n) && blocks(n, "Enemy").length > 1 && blocks(n, "List").some((l) => l.rows.some((r) => r.card?.kind === "Attack" && r.state === "Enabled")), "Monster");
    const atk = $$("#screen .hand .card.k-attack:not(.unplayable)")[0];
    pointerDrag(atk, () => $$("#screen .enemy:not(.dead)").at(-1), false, true);
    // The screenshot's window settles late: move again once it has.
    return new Promise((done) => setTimeout(() => {
      const t = $$("#screen .enemy:not(.dead)").at(-1).getBoundingClientRect();
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: t.left + t.width / 2, clientY: t.top + t.height / 2, pointerId: 1 }));
      done(!!document.querySelector(".enemy.drop") && !!document.querySelector(".dragcard"));
    }, 500));
  },
  // An attack held over an enemy: its exact damage on the enemy, the End Turn cost and the HP left
  // in the aim bar and on End Turn. Outcome `<enemy numbers>|<after line>|<end hint>`.
  preview() {
    play((n) => inFight(n) && blocks(n, "List").some((l) => l.rows.some((r) => r.card?.kind === "Attack" && r.state === "Enabled")), "Monster");
    const atk = $$("#screen .hand .card.k-attack:not(.unplayable)")[0];
    pointerDrag(atk, () => $$("#screen .enemy:not(.dead)").at(-1), false, true);
    return new Promise((done) => setTimeout(() => {
      const t = $$("#screen .enemy:not(.dead)").at(-1).getBoundingClientRect();
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: t.left + t.width / 2, clientY: t.top + t.height / 2, pointerId: 1 }));
      done([$$(".pvx").map(text).join("+"), text(document.querySelector(".after")), text(document.querySelector(".endhint"))].join("|"));
    }, 500));
  },
  // Practice: a card played and taken back leaves the hand and energy as they were; End Turn
  // removes the undo. Outcome `<undo offered>:<restored>:<gone after end turn>`.
  undo() {
    play(inFight);
    const hand = () => $$("#screen .hand .card").length;
    const key = () => JSON.stringify(v().screen);
    const start = key();
    const quiet = () => $$("#screen .hand .card:not(.unplayable)").find((c) => /Block|Strike|Defend/.test(text(c)));
    click(quiet());
    for (let i = 0; i < 3 && v().screen.title; i++) click(document.querySelector("#screen .enemy.aim"));
    const offered = v().undo;
    const played = hand();
    click(document.querySelector("#dock .undo"));
    const restored = key() === start;
    click(quiet());
    for (let i = 0; i < 3 && v().screen.title; i++) click(document.querySelector("#screen .enemy.aim"));
    click(document.querySelector("#dock .endturn"));
    return new Promise((done) => setTimeout(() => done(`${offered}:${restored && hand() !== played}:${!v().undo && !document.querySelector("#dock .undo:not(:disabled)")}`), 500));
  },
  // The settings sheet with large text and End Turn asking first.
  settings() {
    play(inFight);
    S.showLocal(S.settingsSheet());
    click($$("#sheet .seg button").find((b) => b.textContent === "Large"));
    click($$("#sheet .seg button").find((b) => /Ask/.test(b.textContent)));
    return document.documentElement.dataset.text;
  },
  // WEB31: swiping a detail sheet down. A short, slow pull springs back; a long one closes it.
  // Outcome `<kept after a short pull>:<closed after a long one>`.
  async swipe() {
    play(inFight);
    S.showLocal(S.helpSheet());
    const open = () => !document.querySelector("#sheet")?.hidden && !!document.querySelector("#sheet .panel");
    await swipeHead(40, 200);
    const kept = open();
    await swipeHead(240, 0);
    return `${kept}:${!open()}`;
  },
  // WEB31: a pinned choice sheet folds on a swipe down and unfolds on a swipe up.
  // Outcome `<folded>:<unfolded>:<still open>`.
  async swipefold() {
    play((n) => n.fight && n.screen.sheet?.Choice);
    const sheet = () => document.querySelector("#sheet.pinned");
    await swipeHead(120, 0);
    const folded = !!sheet()?.classList.contains("collapsed");
    await swipeHead(-120, 0);
    const unfolded = !!sheet() && !sheet().classList.contains("collapsed");
    return `${folded}:${unfolded}:${!!sheet()}`;
  },
  // WEB31: the platform features feature-detect cleanly: the settings rows, the toggles, the start
  // screen and no console errors. Outcome `wake:<api>:vibrate:<api>:install:<button>:errors:<n>`.
  async platform() {
    const errors = [];
    const err = console.error;
    console.error = (...a) => (errors.push(a.join(" ")), err(...a));
    window.addEventListener("error", (e) => errors.push(e.message));
    play(inFight);
    S.showLocal(S.settingsSheet());
    const rows = $$("#sheet h3").map((h) => h.textContent);
    for (const row of ["Keep screen on", "Vibration"]) {
      const h = $$("#sheet h3").find((x) => x.textContent === row);
      for (const label of ["Off", "On"]) click([...(h?.nextElementSibling?.querySelectorAll("button") ?? [])].find((b) => b.textContent === label));
    }
    const prefsOn = S.pref("keepAwake") !== false && S.pref("vibration") !== false;
    S.closeSheet();
    S.showStart();
    await new Promise((r) => setTimeout(r, 300));
    const install = !!document.querySelector("#start .foot .install");
    return `wake:${"wakeLock" in navigator}:vibrate:${!!navigator.vibrate}:rows:${rows.includes("Keep screen on")}/${rows.includes("Vibration")}:prefs:${prefsOn}:install:${install}:errors:${errors.length}`;
  },
  // End Turn with energy left asks first, then ends on the second tap.
  confirm() {
    play(inFight);
    S.showLocal(S.settingsSheet());
    click($$("#sheet .seg button").find((b) => /Ask/.test(b.textContent)));
    S.closeSheet();
    const end = document.querySelector("#dock .endturn");
    click(end);
    const asked = /tap again/.test(text(document.querySelector("#dock .endturn")));
    return asked ? "asked" : "no-ask";
  },
  // Tap an enemy while no card waits: its next move and powers explained.
  enemy() {
    play((n) => inFight(n) && blocks(n, "Enemy").some((e) => e.powers?.some((p) => p.shown)), "Monster");
    const e = $$("#screen .enemy").find((x) => x.querySelector(".pw")) ?? document.querySelector("#screen .enemy");
    click(e);
    return !!document.querySelector("#sheet .term"); // its powers explained (the card shows their amounts, its HP)
  },
  // End the first turn: the next turn starts with no banner over the field.
  turn() {
    play(inFight);
    S.pinBanner = true;
    click(document.querySelector("#dock .endturn:not(:disabled)"));
    document.body.classList.add("keepbanner");
    return !!v().fight && !document.querySelector("#banner.show");
  },
  // The phone's Back closes the deck sheet and keeps the run.
  back() {
    play(inFight);
    S.open("deck");
    history.back();
    return new Promise((done) => setTimeout(() => done(`${v().sheet_kind}:${!!v().fight}:${history.state?.run}`), 400));
  },
  tutorial: () => !!document.querySelector("#sheet .steps"),
  map: () => play((n) => n.decision === "map" && n.save.floor > 1),
  // Route planner: a thumb taps a room two rows up (a preview, not a trip); outcome
  // `<rooms lit>:<label>:<still on the map>`.
  route() {
    play((n) => n.decision === "map" && n.save.floor > 1);
    const far = $$("#screen g.node[data-at]:not(.reach):not(.visited)").filter((n) => n.classList.contains("ahead") && n.querySelector(".hit"));
    const pick = far.find((n) => /^\d,(\d+)$/.test(n.dataset.at) && Number(n.dataset.at.split(",")[1]) >= (v().screen.blocks.find((b) => b.Map).Map.you?.[1] ?? 0) + 3) ?? far[0];
    touchTap(pick);
    return `${$$("#screen g.node.inroute").length}:${text(document.querySelector(".routelabel"))}:${v().decision === "map"}`;
  },
  // One tap on a next room travels, with no dock buttons on a phone; outcome `<no dock rooms>:<travelled>`.
  routetap() {
    play((n) => n.decision === "map" && n.save.floor > 1);
    const docked = !!document.querySelector("#dock .nextroom");
    touchTap(document.querySelector("#screen g.node.reach"));
    return `${!docked}:${v().decision !== "map"}`;
  },
  rewards: () => play((n) => n.decision === "rewards" && !document.querySelector("#screen .cards:not(.peek)")),
  cardpick() {
    play((n) => n.decision === "rewards");
    for (let i = 0; i < 6 && !document.querySelector("#screen .cards:not(.peek)"); i++) {
      const card = $$("#screen .row").find((r) => /card/i.test(text(r)));
      if (!click(card)) break;
    }
    return !!document.querySelector("#screen .cards:not(.peek)");
  },
  shop: () => play((n) => n.decision === "shop", "Shop"),
  // The shop's cards and its relics (one screen since WEB30: the cards, then the relics scrolled to).
  shopcards() {
    if (!play((n) => n.decision === "shop", "Shop")) return false;
    return !!document.querySelector("#screen .shopsec .cards .card");
  },
  shoprelics() {
    if (!play((n) => n.decision === "shop", "Shop")) return false;
    document.querySelector("#screen .s-relics")?.scrollIntoView({ block: "start" });
    return !!document.querySelector("#screen .ware .info");
  },
  // Buys a card and a potion with one tap and its "Tap again" each: the gold goes down, both
  // leave the stock and the screen stays the shop. Outcome
  // `<card bought>:<potion bought>:<gold before>><gold after>:<still the shop>`.
  shopbuy() {
    if (!play((n) => n.decision === "shop", "Shop")) return "no shop";
    const gold0 = v().screen.top.gold;
    const buy = (sel) => {
      const e = $$(sel).find((x) => !x.classList.contains("unplayable") && !x.classList.contains("unaffordable"));
      if (!e) return null;
      const id = e.dataset.bid;
      const [n, stock] = [v().save.actions.length, $$(sel).length];
      click(e);
      const asked = !!document.querySelector(`#screen [data-bid="${id}"].asking`);
      click(document.querySelector(`#screen [data-bid="${id}"]`));
      return asked && v().save.actions.length === n + 1 && $$(sel).length === stock - 1;
    };
    const card = buy("#screen .s-cards .card, #screen .s-colorless .card");
    const potion = buy("#screen .s-potions .ware");
    return `${card}:${potion}:${gold0}>${v().screen.top.gold}:${v().decision === "shop" && !!document.querySelector("#screen .shopsec")}`;
  },
  // Act 2's map and its first fight.
  act2map: () => play((n) => n.decision === "map" && n.screen.blocks.some((b) => b.Map?.act === 2), "Rest", 20000),
  act2fight: () => play((n) => inFight(n) && n.screen.blocks.some(() => true) && n.save.floor > 18, "Rest", 20000),
  // Low-end phones: 5 enemies and 10 cards, one aimed (every enemy beckons), animation on.
  // Outcome `render=<median ms>:frames=<p50>/<p95>/<max ms>:long=<frames over 34 ms>`.
  async perf() {
    play((n) => inFight(n) && blocks(n, "List").some((l) => l.rows.some((r) => r.card)));
    document.body.classList.remove("proof");
    const base = JSON.parse(JSON.stringify(v()));
    const sc = base.screen;
    const enemyAt = sc.blocks.findIndex((b) => b.Enemy);
    const foe = sc.blocks[enemyAt].Enemy;
    const list = sc.blocks.find((b) => b.List).List;
    const rows = list.rows;
    sc.blocks = sc.blocks.filter((b) => !b.Enemy);
    const foes = Array.from({ length: 5 }, (_, i) => ({ Enemy: { ...foe, id: 900 + i, name: `${foe.name} ${i + 1}`, target: { ...rows[0], id: 990 + i, state: "Enabled" }, preview: { Certain: { hp_before: foe.hp, hp_after: Math.max(0, foe.hp - 6), block_before: 0, block_after: 0, dies: false } } } }));
    sc.blocks.splice(enemyAt, 0, ...foes);
    list.rows = Array.from({ length: 10 }, (_, i) => ({ ...rows[i % rows.length], id: 700 + i, state: i === 0 ? "Selected" : "Enabled" }));
    sc.title = [{ role: "Plain", bold: false, text: "Strike → pick a target" }];
    const times = [];
    for (let i = 0; i < 15; i++) {
      const t = performance.now();
      S.draw(JSON.parse(JSON.stringify(base)));
      times.push(performance.now() - t);
    }
    await new Promise((r) => setTimeout(r, 400));
    const gaps = [];
    // Headless Chrome's virtual time may stop the frames: 180 frames or 3.5 s, whichever is first.
    await new Promise((done) => {
      let last = performance.now();
      const tick = (now) => {
        gaps.push(now - last);
        last = now;
        if (gaps.length < 180) requestAnimationFrame(tick);
        else done();
      };
      requestAnimationFrame(tick);
      setTimeout(done, 3500);
    });
    if (!gaps.length) gaps.push(0);
    const q = (a, f) => a.slice().sort((x, y) => x - y)[Math.min(a.length - 1, Math.floor(a.length * f))].toFixed(1);
    return `render=${q(times, 0.5)}:frames=${q(gaps, 0.5)}/${q(gaps, 0.95)}/${Math.max(...gaps).toFixed(0)}:long=${gaps.filter((g) => g > 34).length}of${gaps.length}`;
  },
  rest: () => play((n) => n.decision === "rest", "Rest"),
  event: () => play((n) => n.decision === "event", "Unknown"),
  treasure: () => play((n) => n.decision === "treasure", "Treasure"),
  menu() {
    play(inFight);
    click(document.querySelector("#top .iconbtn:last-child"));
    return v().sheet_kind === "Menu";
  },
  deck() {
    play((n) => n.decision === "map" && n.save.floor > 1);
    click(document.querySelector("#top .iconbtn"));
    return v().sheet_kind === "Deck";
  },
  // WEB12: hold a deck card (touch) for the upgrade pair in its sheet; right-click it (desktop)
  // for the popover. Outcome `<pair in sheet>` / `<pair in popover>`.
  upgrade() {
    play((n) => n.decision === "map" && n.save.floor > 1);
    click(document.querySelector("#top .iconbtn"));
    const card = $$("#sheet .cards .card")[0];
    card.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, clientX: 5, clientY: 5 }));
    return new Promise((done) => setTimeout(() => done(`${!!document.querySelector("#sheet .uppair")}:${$$("#sheet .uppair .hl.new").length}`), 600));
  },
  upgradehover() {
    play((n) => n.decision === "map" && n.save.floor > 1);
    click(document.querySelector("#top .iconbtn"));
    $$("#sheet .cards .card")[0].dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: 5, clientY: 5 }));
    return `${!!document.querySelector(".uppop .uppair")}:${$$(".uppop .hl.new").length}`;
  },
  // WEB12: the History screen after a finished run, and one run's detail.
  async history() {
    play((n) => n.over, null, 20000);
    S.showLocal((await import("./history.js")).historySheet());
    return `${$$("#sheet .hrun").length}`;
  },
  async historyrun() {
    play((n) => n.over, null, 20000);
    S.showLocal((await import("./history.js")).historySheet());
    click($$("#sheet .hrun")[0]);
    return `${$$("#sheet .hcard").length}`;
  },
  // WEB12: the start screen's Daily button starts the date's seed.
  async daily() {
    S.showStart();
    click(document.querySelector(".dailybtn"));
    const { dailySeed } = await import("./daily.js");
    return `${v().save.seed === dailySeed()}:${v().save.seed}`;
  },
  // The draw pile as card tiles, sorted by cost.
  pile() {
    play(inFight);
    click(document.querySelector("#dock .pile"));
    click($$("#sheet .sorts button").find((b) => b.textContent === "Cost"));
    return `${v().sheet_kind}:${$$("#sheet .cards .card").length}`;
  },
  // Tap the fight's folded relics, then one: its text in a sheet.
  relic() {
    play(inFight);
    click(document.querySelector("#top .relic"));
    click(document.querySelector("#sheet .list .row"));
    return !!document.querySelector("#sheet .textsheet");
  },
  // A reward's relic or potion read before it is taken.
  offer() {
    play((n) => n.decision === "rewards" && !!document.querySelector("#screen .row .info"), "Elite", 4000);
    click(document.querySelector("#screen .row .info"));
    return !!document.querySelector("#sheet .textsheet");
  },
  mapsheet() {
    play(inFight);
    S.open("map");
    return v().sheet_kind === "Map";
  },
  potion() {
    const ok = play((n) => n.screen.top?.potions.some((p) => !p.empty) && n.fight, "Monster");
    click(document.querySelector("#top .pot:not(.empty)"));
    return ok;
  },
  help() {
    play(inFight);
    S.showLocal(S.helpSheet());
    return true;
  },
  choice: () => play((n) => n.fight && n.screen.sheet?.Choice),
  end() {
    // A hopeless player: ends every turn at once.
    for (let i = 0; i < 3000 && !v().over; i++) {
      if (v().fight) click(document.querySelector("#dock .endturn:not(:disabled)")) || fightStep();
      else roomStep("Elite");
    }
    return v().over;
  },
  // Elite and boss fights open with their own banner.
  elite() {
    S.pinBanner = true;
    document.body.classList.add("keepbanner");
    return play((n) => inFight(n) && n.room === "Elite", "Elite", 20000);
  },
  boss() {
    S.pinBanner = true;
    document.body.classList.add("keepbanner");
    return play((n) => inFight(n) && n.room === "Boss", "Rest", 20000);
  },
  // A fight won: the "Victory" beat over the rewards.
  won() {
    play(inFight);
    S.pinBanner = true;
    document.body.classList.add("keepbanner");
    return play((n) => !n.fight);
  },
  // The run's end as it happens: the full-screen Defeat beat.
  dead() {
    S.pinMoment = true;
    document.body.classList.add("keepmoment");
    for (let i = 0; i < 3000 && !v().over; i++) {
      if (v().fight) click(document.querySelector("#dock .endturn:not(:disabled)")) || fightStep();
      else roomStep("Elite");
    }
    return !!document.querySelector(".moment");
  },
  // Save on the map, leave to the start screen, Continue: the same screen comes back.
  continue() {
    play((n) => n.decision === "map" && n.save.floor > 2);
    const was = JSON.stringify(v().screen);
    S.showStart();
    click(document.querySelector("#start .continue"));
    return JSON.stringify(v().screen) === was ? "same-screen" : "differs";
  },
  // A whole act: fights, the map, rewards, shops, events, rest; done when the run ends.
  run() {
    const ok = play((n) => n.over, null, 20000);
    return `${ok}:${[...seen].join(",")}:floor${v().save.floor}`;
  },
};

async function go() {
  const character = Number(q.get("char") ?? 4);
  const seed = q.get("seed") ?? "PROOF";
  if (scenario !== "start") S.start(character, seed, 0);
  let out;
  try {
    // WEB10's bot scenarios live in proof-bot.js.
    const extra = scenario.startsWith("bot") ? (await import("./proof-bot.js")).scenarios({ play, click, v }) : {};
    out = await (SCENARIOS[scenario] ?? extra[scenario] ?? (() => "unknown scenario"))();
  } catch (e) {
    out = `error ${e}`;
    console.error(e);
  }
  log.push(out);
  if (!document.querySelector(".uppop")) window.scrollTo(0, 0);
  document.title = `proof:${scenario}:${out}:${v()?.decision}:${S.text()}`;
  if (q.has("trail")) document.title += ` TRAIL ${trail.join(" | ")}`;
  // Real-time runs (`perf`) have no DOM dump: the server logs the title.
  if (q.has("report")) fetch(`/proof-done?${encodeURIComponent(document.title)}`).catch(() => {});
}
go();
