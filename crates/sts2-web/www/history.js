// Run history and stats, kept in localStorage (`sts2-history-v1`, newest last): every finished
// run with its character, ascension, seed, result, floor, act, killer, final deck, relics and
// path, plus what a replay needs (seed, character, ascension and the save's action log). The
// History screen on the start page reads it.
import { dailyOf, utcDate } from "./daily.js";
import { shareRun, shareWords } from "./share.js";

const KEY = "sts2-history-v1";
const MAX = 300;

let ctx = null;
export const initHistory = (c) => (ctx = c);

const read = () => {
  try {
    const h = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(h) ? h : [];
  } catch {
    return [];
  }
};
const write = (h) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(h.slice(-MAX)));
  } catch {
    // Storage full: drop the action logs of the older half and try once more.
    const cut = h.slice(-MAX).map((r, i, a) => (i < a.length / 2 ? { ...r, actions: undefined } : r));
    try {
      localStorage.setItem(KEY, JSON.stringify(cut));
    } catch {}
  }
};
export const runs = () => read();

// "Slain by Bygone Effigy: Tackle 4×3." → "Bygone Effigy"; "Died in Shop." → "Shop".
export function killerName(text) {
  if (!text) return null;
  const who = /^Slain by (.+?)\.?$/.exec(text)?.[1];
  if (who?.startsWith("no enemy")) return "No enemy (card, relic or curse)";
  if (who) return who.split(":")[0].trim();
  return /^Died in (.+?)\.?$/.exec(text)?.[1] ?? text;
}

// Called after every view; records the run once when it has just ended (a win or a loss).
let recorded = false;
export function recordRun(view) {
  if (!view?.over) {
    recorded = false;
    return;
  }
  const s = view.summary;
  if (recorded || !s || (s.result !== "win" && s.result !== "loss")) return;
  recorded = true;
  const at = Date.now();
  const h = read();
  h.push({
    id: `${at.toString(36)}${Math.random().toString(36).slice(2, 6)}`,
    at,
    date: utcDate(new Date(at)),
    daily: dailyOf(view.save.seed),
    modifierKeys: view.save.modifiers ?? [],
    modifiers: (view.run?.modifiers ?? []).map((m) => m.title),
    character: view.save.character,
    ascension: view.save.ascension,
    seed: view.save.seed,
    result: s.result,
    floor: s.floor,
    act: s.act,
    hp: s.hp,
    max_hp: s.max_hp,
    gold: s.gold,
    killer: s.result === "loss" ? killerName(s.killer) : null,
    deck: s.deck,
    relics: s.relics,
    relic_chips: (s.relic_chips ?? []).map(({ name, detail }) => ({ name, detail })),
    path: s.path,
    actions: view.save.actions,
  });
  write(h);
}

// ---- stats --------------------------------------------------------------------------------

const pct = (w, n) => (n ? `${Math.round((100 * w) / n)}%` : "—");

export function stats(h = read()) {
  const wins = h.filter((r) => r.result === "win").length;
  const by = (key) => {
    const m = new Map();
    for (const r of h) {
      const g = m.get(r[key]) ?? { key: r[key], runs: 0, wins: 0 };
      g.runs++;
      if (r.result === "win") g.wins++;
      m.set(r[key], g);
    }
    return [...m.values()];
  };
  const killers = new Map();
  for (const r of h) if (r.killer) killers.set(r.killer, (killers.get(r.killer) ?? 0) + 1);
  let best = 0;
  let run = 0;
  for (const r of [...h].sort((a, b) => a.at - b.at)) {
    run = r.result === "win" ? run + 1 : 0;
    best = Math.max(best, run);
  }
  return {
    runs: h.length,
    wins,
    rate: pct(wins, h.length),
    bestStreak: best,
    byCharacter: by("character").sort((a, b) => b.runs - a.runs),
    byAscension: by("ascension").sort((a, b) => a.key - b.key),
    killers: [...killers].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0])).slice(0, 5),
  };
}

// The first recorded result of the daily on `date`, or null.
export const dailyResult = (date = utcDate()) => read().find((r) => r.daily === date) ?? null;

// ---- the replay hook ----------------------------------------------------------------------
// "Watch the bot play this seed": another module builds the replay viewer. It answers either by
// setting `window.sts2.watchSeed = ({seed, character, ascension, actions, source}) => …`, or by
// listening for the cancelable `sts2:watch-seed` event on `document` and calling
// `preventDefault()`. With neither the page says it is not there yet.
export const WATCH_EVENT = "sts2:watch-seed";
export function watchSeed(run) {
  const detail = { seed: run.seed, character: run.character, ascension: run.ascension, actions: run.actions ?? null, source: "history" };
  if (typeof window.sts2?.watchSeed === "function") return void window.sts2.watchSeed(detail);
  const ev = new CustomEvent(WATCH_EVENT, { detail, cancelable: true });
  document.dispatchEvent(ev);
  if (!ev.defaultPrevented) ctx.say("The replay viewer is not available yet.");
}

// ---- screens ------------------------------------------------------------------------------

const when = (r) => new Date(r.at).toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" });

function tile(big, small) {
  return ctx.el("div", "hstat", ctx.el("b", null, big), ctx.el("small", null, small));
}

const line = (label, sub, right) =>
  ctx.el("div", "hline", ctx.el("span", "main", ctx.el("span", "lab", label), sub ? ctx.el("span", "sub", sub) : null), right != null ? ctx.el("span", "right", right) : null);

// The stats tiles, today's daily and the hero, ascension and killer breakdowns (History and the
// compendium's Statistics share them).
export function statsBlocks(h = read()) {
  const st = stats(h);
  const out = [ctx.el("div", "hstats", tile(`${st.runs}`, st.runs === 1 ? "run" : "runs"), tile(`${st.wins}`, st.wins === 1 ? "win" : "wins"), tile(st.rate, "win rate"), tile(`${st.bestStreak}`, "best streak"))];
  const today = dailyResult();
  if (today) out.push(ctx.el("h3", null, "Today's daily"), line(`${today.result === "win" ? "Won" : "Lost"} · ${ctx.character(today.character).name}`, `Floor ${today.floor} · seed ${today.seed}`));
  out.push(ctx.el("h3", null, "By hero"), ...st.byCharacter.map((g) => line(ctx.character(g.key).name, `${g.wins} of ${g.runs} won`, pct(g.wins, g.runs))));
  out.push(ctx.el("h3", null, "By ascension"), ...st.byAscension.map((g) => line(`Ascension ${g.key}`, `${g.wins} of ${g.runs} won`, pct(g.wins, g.runs))));
  if (st.killers.length) out.push(ctx.el("h3", null, "Top killers"), ...st.killers.map(([k, n]) => line(k, null, `×${n}`)));
  return out;
}

export function historySheet() {
  return {
    title: "History",
    body() {
      const h = read();
      if (!h.length) return [ctx.el("p", "note", "No finished runs yet. Every run you win or lose is kept here, on this device.")];
      const out = statsBlocks(h);
      out.push(ctx.el("h3", null, "Runs"));
      const list = ctx.el("div", "list");
      for (const r of [...h].reverse()) {
        const win = r.result === "win";
        const b = ctx.el(
          "button",
          `row hrun ${win ? "won" : "lost"}`,
          ctx.el("span", "main", ctx.el("span", "lab", `${win ? "Victory" : "Defeat"} · ${ctx.character(r.character).name}${r.ascension ? ` A${r.ascension}` : ""}${r.daily ? " · Daily" : r.modifiers?.length ? " · Custom" : ""}`), ctx.el("span", "sub", `Floor ${r.floor} · Act ${r.act}${r.killer ? ` · ${r.killer}` : ""} · ${when(r)}`)),
          ctx.el("span", "chev", "›"),
        );
        list.append(ctx.tap(b, () => ctx.showLocal(runSheet(r.id))));
      }
      out.push(list);
      let sure = false;
      const clear = ctx.el("button", "chipbtn danger", "Clear history");
      ctx.tap(clear, () => {
        if (!sure) {
          sure = true;
          clear.textContent = "Tap again to clear";
          return;
        }
        localStorage.removeItem(KEY);
        ctx.showLocal(historySheet());
      });
      out.push(ctx.el("div", "hfoot", clear));
      return out;
    },
  };
}

function grouped(deck) {
  const m = new Map();
  for (const c of deck) m.set(c.name, (m.get(c.name) ?? 0) + 1);
  return [...m].sort((a, b) => a[0].localeCompare(b[0]));
}

const KIND = { card: "Took", skip: "Skipped", gold: "Took", potion: "Took", relic: "Took", ancient: "Chose", buy: "Bought", enchant: "Enchanted", event: "Event", max_hp: "Max HP", potion_used: "Used", potion_discarded: "Dropped", rest: "Rest", transform: "Transformed" };

// One floor's choices as lines: the card taken and the ones passed over, relics, buys, rests.
function floorChoices(p) {
  const out = [];
  for (const c of p.choices ?? []) {
    const what = c.kind === "skip" ? "all cards" : c.text;
    out.push(ctx.el("div", `hchoice hc-${c.kind}`, ctx.el("b", null, `${KIND[c.kind] ?? c.kind} `), what));
    if (c.skipped?.length) out.push(ctx.el("div", "hchoice passed", ctx.el("small", null, `Passed over: ${c.skipped.join(", ")}`)));
  }
  return out;
}

export function runSheet(id) {
  const r = read().find((x) => x.id === id);
  const self = () => runSheet(id);
  // A card or relic opens its text; back returns here.
  const lookAt = (title, body) => ({ title, body: () => [ctx.tap(ctx.el("button", "chipbtn back", "‹ Run"), () => ctx.showLocal(self())), ...body()] });
  const cardOpen = (face) => lookAt(face.name, () => ctx.cardSheet({ id: 0, state: "Enabled", label: [], detail: [], card: face }).body());
  return {
    title: r ? `${r.result === "win" ? "Victory" : "Defeat"} · ${ctx.character(r.character).name}` : "Run",
    body() {
      if (!r) return [ctx.el("p", "note", "That run is gone.")];
      const save = ctx.el("p", "note", `${when(r)} · Ascension ${r.ascension} · seed ${r.seed}${r.daily ? ` · Daily ${r.daily}` : ""}${r.modifiers?.length ? ` · Modifiers: ${r.modifiers.join(", ")}` : ""}`);
      const fin = ctx.el("p", "note", `Act ${r.act} · Floor ${r.floor} · ♥ ${r.hp}/${r.max_hp} · ${r.gold} gold${r.killer ? ` · killed by ${r.killer}` : ""}`);
      const share = ctx.tap(ctx.el("button", "chipbtn", "Share seed"), async () => ctx.say(shareWords[await shareRun({ seed: r.seed, char: r.character, asc: r.ascension })]));
      const again = ctx.tap(ctx.el("button", "chipbtn", "Play this seed"), () => {
        ctx.closeSheet();
        ctx.startRun(r.character, r.seed, r.ascension, r.modifierKeys ?? []);
      });
      // The hook for the replay viewer (see `watchSeed`).
      const watch = ctx.tap(ctx.el("button", "chipbtn watch-seed", "Watch the bot play this seed"), () => watchSeed(r));
      watch.dataset.watchSeed = r.seed;
      const review = r.actions?.length && window.sts2?.review ? ctx.tap(ctx.el("button", "chipbtn review-run", "Review this run"), () => window.sts2.review({ character: r.character, seed: r.seed, ascension: r.ascension, modifiers: r.modifierKeys ?? [], actions: r.actions, end: r.result === "win" ? "victory" : "defeat", floor: r.floor })) : null;
      const back = ctx.tap(ctx.el("button", "chipbtn", "‹ History"), () => ctx.showLocal(historySheet()));
      const deck = r.deck ?? [];
      const faces = new Map();
      for (const c of deck) {
        const key = c.name;
        const g = faces.get(key) ?? { c, n: 0 };
        g.n++;
        faces.set(key, g);
      }
      const chip = (g) => {
        const b = ctx.el("button", `hcard${/\+\d*$/.test(g.c.name) ? " up" : ""}`, g.n > 1 ? `${g.c.name} ×${g.n}` : g.c.name);
        return g.c.face ? ctx.tap(b, () => ctx.showLocal(cardOpen(g.c.face))) : b;
      };
      const relics = (r.relic_chips ?? []).length ? r.relic_chips : (r.relics ?? []).map((name) => ({ name }));
      const relicChip = (x) => {
        const b = ctx.el("button", "hcard relic", x.name);
        return x.detail ? ctx.tap(b, () => ctx.showLocal(lookAt(x.name, () => [ctx.textSheet(x.detail)]))) : b;
      };
      return [
        ctx.el("div", "hacts", back, share, again),
        save,
        fin,
        ctx.el("div", "hacts", watch, review),
        ctx.el("h3", null, `Deck · ${deck.length} cards`),
        ctx.el("div", "hdeck", [...faces.values()].sort((a, b) => a.c.name.localeCompare(b.c.name)).map(chip)),
        ctx.el("h3", null, `Relics · ${r.relics?.length ?? 0}`),
        ctx.el("div", "hdeck", relics.map(relicChip)),
        ctx.el("h3", null, "Floor by floor"),
        ctx.el("div", "hpath", (r.path ?? []).map((p) => ctx.el("div", "hfloor", ctx.el("div", "hstep", ctx.el("span", "fl", `${p.act}-${p.floor}`), ctx.el("span", "what", p.what), ctx.el("span", "hp", `♥ ${p.hp}/${p.max_hp}`)), ...floorChoices(p)))),
      ];
    },
  };
}
