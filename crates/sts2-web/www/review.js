// The run review (WEB46): the 3 decisions of a finished run where the bot most disagrees with the
// player, each with both moves and the numbers the bot compared (a fight's win estimate, HP and
// search score; a run choice's policy odds), and the board at that moment. Worked out on demand
// in the bot's Web Worker, a slice at a time; saved in the browser as it goes, so it can be left
// and picked up again, even after a reload. Opened from the end screen and a History run.
import { call, warm } from "./worker.js";
import { button, el } from "./replay.js";

const S = window.sts2;
const STORE = "sts2-reviews-v1";
const KEPT = 12;
const SAVE_EVERY_MS = 2000;
// Below these the two moves are close: the card says so. A fight's score in thousandths of a
// win, a run choice's odds in thousandths.
const SMALL_FIGHT = 20;
const SMALL_CHOICE = 150;
// The bot's own pick under this share of the policy, or ahead in under this share of the sampled
// draws: the card calls it uncertain.
const UNSURE_SHARE = 500;
const UNSURE_DRAWS = 0.7;
// A swipe's least length, as a share of the card's width.
const SWIPE = 0.15;
const HEROES = ["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"];
const KIND = { ancient: "Neow", event: "Event", map: "Map", rewards: "Rewards", "card-select": "Card choice", bundles: "Bundles", rest: "Rest site", treasure: "Treasure", shop: "Shop" };
const ROOM = { boss: "Boss", elite: "Elite", fight: "Fight" };

let job = null; // the review in progress or on screen
let nextId = 1;
let screen = null; // the overlay, while it is open
let head = null; // the overlay's title and ✕
let board = null; // the bar, while a board is on screen

const keyOf = (r) => `${r.character}|${r.seed}|${r.ascension}|${r.actions.length}`;
// Thousandths as a percent: never "0%" or "100%" for a net's estimate.
const pct = (n) => (n >= 995 ? "99%+" : n < 5 ? "under 1%" : `${Math.round(n / 10)}%`);

function store() {
  try {
    const all = JSON.parse(localStorage.getItem(STORE) ?? "[]");
    return Array.isArray(all) ? all : [];
  } catch {
    return [];
  }
}
function saved(key) {
  return store().find((x) => x.key === key) ?? null;
}
function save() {
  if (!job) return;
  const { key, base, total, found, seen, result } = job;
  const all = store().filter((x) => x.key !== key);
  all.unshift({ key, at: base, total, found: result ? [] : found, seen, result });
  try {
    localStorage.setItem(STORE, JSON.stringify(all.slice(0, KEPT)));
  } catch {}
}

// ---- the work ------------------------------------------------------------------------------

// A review runs while it is on screen or nothing else needs the worker: not under a run in play
// or the bot's watch. The next drawing starts it again.
function free() {
  if (screen) return true;
  if (document.body.classList.contains("watching")) return false;
  const v = S.view();
  return !document.getElementById("start").hidden || !v || v.over;
}

function begin(run) {
  const key = keyOf(run);
  if (job?.key === key) return job;
  const was = saved(key);
  // `base`: where `found` is up to; the work picks up from there.
  job = { key, run, id: nextId++, base: was?.at ?? 0, at: was?.at ?? 0, total: was?.total ?? run.actions.length, found: was?.found ?? [], seen: was?.seen ?? 0, result: was?.result ?? null, started: false, busy: false, error: null, last: 0 };
  return job;
}

async function pump() {
  const me = job;
  if (!me || me.busy || me.result || me.error || !free()) return;
  me.busy = true;
  try {
    if (!me.started) {
      await call("reviewStart", { id: me.id, save: me.run, at: me.base, found: me.found, seen: me.seen });
      me.at = me.base;
      me.started = true;
    }
    const due = performance.now() - me.last > SAVE_EVERY_MS;
    const r = await call("reviewStep", { id: me.id, state: due });
    if (r.stale) me.started = false;
    else {
      me.at = r.at;
      me.total = r.total;
      if (r.state) {
        me.found = r.state.found;
        me.seen = r.state.seen;
        me.base = r.at;
        me.last = performance.now();
      }
      if (r.result) me.result = r.result;
      if ((r.state || r.result) && me === job) save();
    }
  } catch (e) {
    me.error = String(e.message ?? e);
    me.started = false;
  }
  me.busy = false;
  if (me !== job) return pump();
  paint();
  if (!me.result && !me.error) setTimeout(pump, 0);
}

// ---- the review on screen ------------------------------------------------------------------

function open(run) {
  begin(run);
  if (!screen) {
    screen = el("div");
    screen.id = "review";
    document.body.append(screen);
    if (!history.state?.review) history.pushState({ ...history.state, review: 1 }, "");
  }
  head = el("div", "rvhead", el("div", "rvtitle", el("h2", null, "Run review"), el("small", null, title())), button("wbtn rvclose", "✕", close));
  screen.dataset.drawn = "";
  paint();
  pump();
}
S.review = open;

function close() {
  if (board) closeBoard();
  screen?.remove();
  screen = null;
}

const title = () => {
  const r = job.run;
  const end = { victory: "Victory", defeat: "Defeat" }[r.end] ?? "Stopped";
  return `${end} · ${HEROES[r.character] ?? "?"} · Floor ${r.floor}`;
};

// The head stays and the bar is updated in place: a button rebuilt under a press loses the tap.
function paint() {
  paintRow();
  if (!screen || !job) return;
  const state = job.error ? "error" : job.result ? "done" : "busy";
  const drawn = screen.dataset.drawn;
  if (state === "busy" && drawn === "busy") return updateProgress();
  // The cards are drawn once: a redraw would lose the decision on screen.
  if (state === "done" && drawn === "done") return;
  screen.dataset.drawn = state;
  if (state === "busy") {
    screen.replaceChildren(head, progress());
    updateProgress();
  } else if (state === "error") {
    screen.replaceChildren(head, el("div", "rvbusy", el("p", null, "The review stopped."), el("small", null, job.error), button("wbtn rvretry", "Try again", () => {
      job.error = null;
      screen.dataset.drawn = "";
      paint();
      pump();
    })));
  } else showCards();
}

function progress() {
  return el("div", "rvbusy", el("p", "rvline"), el("div", "rvbar", el("i")), el("small", null, "Close this any time: it keeps its place."));
}

function updateProgress() {
  const share = job.total ? job.at / job.total : 0;
  screen.querySelector(".rvline").textContent = job.at || warm() ? `Comparing your ${job.total} moves with the bot's: ${Math.floor(share * 100)}%` : "Loading the bot…";
  screen.querySelector(".rvbar i").style.width = `${share * 100}%`;
}

function row(label, you, bot) {
  return el("tr", null, el("th", null, label), el("td", null, you), el("td", "isbot", bot));
}

const number = (x) => (Number.isInteger(x) ? `${x}` : x.toFixed(1));

function fightRows(p) {
  const rows = [];
  const { you, bot } = p.ends ?? {};
  if (you && bot) {
    rows.push(row("Win this fight", pct(you.win), pct(bot.win)));
    rows.push(row(`HP next turn (now ${p.hp})`, number(Math.round(you.hp_tenths / 10)), number(Math.round(bot.hp_tenths / 10))));
    if (you.potions || bot.potions) rows.push(row("Potions drunk", number(you.potions / p.worlds), number(bot.potions / p.worlds)));
  }
  rows.push(row("Score", number(Math.round(p.score.you / 10)), number(Math.round(p.score.bot / 10))));
  return rows;
}

// What the numbers say, and nothing they do not: the bot gives no reasons of its own.
function verdict(p) {
  if (p.kind === "combat") {
    const gap = p.score.bot - p.score.you;
    const draws = `in ${p.ahead} of ${p.worlds} sampled draws`;
    if (gap < SMALL_FIGHT) return `Small gap: ${(gap / 10).toFixed(1)} points. The bot's move leads ${draws}.`;
    if (p.ahead / p.worlds < UNSURE_DRAWS) return `Uncertain: the bot's move leads only ${draws}, by ${Math.round(gap / 10)} points.`;
    return `The bot's move leads ${draws}, by ${Math.round(gap / 10)} points.`;
  }
  const gap = p.share.bot - p.share.you;
  if (gap < SMALL_CHOICE) return `Small gap: ${pct(gap)} between them, of ${p.options} choices.`;
  if (p.share.bot < UNSURE_SHARE) return `Uncertain: the bot's own pick gets only ${pct(p.share.bot)}, of ${p.options} choices.`;
  return `The bot's run policy rates its pick above yours, of ${p.options} choices. It gives odds, not reasons.`;
}

function where(p, i, n) {
  const what = p.kind === "combat" ? `${ROOM[p.room] ?? "Fight"}, turn ${p.round}` : KIND[p.kind] ?? p.kind;
  return `${i + 1} of ${n} · Floor ${p.floor} · ${what}`;
}

function card(p, i, n) {
  const fight = p.kind === "combat";
  const table = el("table", "rvnums", el("thead", null, el("tr", null, el("th"), el("th", null, "You"), el("th", "isbot", "Bot"))), el("tbody", null, ...(fight ? fightRows(p) : [row("Policy's odds", pct(p.share.you), pct(p.share.bot))])));
  const see = button("primary rvsee", "See the board", () => showBoard(p));
  return el(
    "article",
    "rvcard",
    el("div", "rvwhere", where(p, i, n)),
    el("div", "rvmoves", el("div", "rvmove", el("i", null, "You"), el("b", null, p.you)), el("div", "rvmove isbot", el("i", null, "Bot"), el("b", null, p.bot))),
    table,
    el("p", "rvwhy", verdict(p)),
    fight ? el("small", "rvnote", "Score: its win estimate plus credit for HP kept, less potions drunk.") : null,
    see,
  );
}

function showCards() {
  const picks = job.result.picks;
  if (!picks.length) {
    screen.replaceChildren(head, el("div", "rvbusy", el("p", null, "No scored move differs from the bot's.")));
    return;
  }
  const track = el("div", "rvtrack", ...picks.map((p, i) => card(p, i, picks.length)));
  const small = picks.every((p) => (p.kind === "combat" ? p.score.bot - p.score.you < SMALL_FIGHT : p.share.bot - p.share.you < SMALL_CHOICE));
  const dots = picks.map((_, i) => button("rvdot", "", () => go(i)));
  dots.forEach((d, i) => d.setAttribute("aria-label", `Decision ${i + 1}`));
  const prev = button("wbtn rvprev", "‹", () => go(place - 1));
  const next = button("wbtn rvnext", "›", () => go(place + 1));
  let place = 0;
  const go = (i) => {
    place = Math.max(0, Math.min(i, picks.length - 1));
    [...track.children].forEach((c, k) => c.classList.toggle("on", k === place));
    dots.forEach((d, k) => d.classList.toggle("on", k === place));
    prev.disabled = place <= 0;
    next.disabled = place >= picks.length - 1;
  };
  // A swipe: a mostly sideways stroke over a good part of the card. Up and down scroll the card.
  let from = null;
  track.addEventListener("pointerdown", (e) => (from = [e.clientX, e.clientY]));
  track.addEventListener("pointercancel", () => (from = null));
  track.addEventListener("pointerup", (e) => {
    if (!from) return;
    const [dx, dy] = [e.clientX - from[0], e.clientY - from[1]];
    from = null;
    if (Math.abs(dx) > track.clientWidth * SWIPE && Math.abs(dx) > 2 * Math.abs(dy)) go(place - Math.sign(dx));
  });
  const nav = el("div", "rvnav", prev, el("div", "rvdots", ...dots), next);
  screen.replaceChildren(...[head, small ? el("p", "rvsmall", "Every gap here is small: the bot would play this run much the same.") : null, track, picks.length > 1 ? nav : null].filter(Boolean));
  go(0);
}

// ---- the board at that moment --------------------------------------------------------------

function light(move, cls) {
  const id = move?.taps?.[0];
  const sel = (p) => `${p} [data-bid="${id}"], ${p} [data-tap="${id}"]`;
  if (id != null) for (const e of document.querySelectorAll(`${sel("#screen")}, ${sel("#dock")}, ${sel("#sheet")}, #screen g.node[data-button="${id}"]`)) e.classList.add(cls);
  for (const aim of move?.aims ?? []) for (const e of document.querySelectorAll(`#screen .enemy[data-id="${aim}"]`)) e.classList.add(cls);
  if (move?.potion != null) document.querySelector(`#top [data-slot="${move.potion}"]`)?.classList.add(cls);
}

async function showBoard(p) {
  const r = await call("reviewBoard", { save: job.run, at: p.at }).catch(() => null);
  if (!r || !screen) return;
  const { view, you, bot } = r.board;
  board = el("div", "rvboard", el("div", "rvboardmoves", el("span", "rvyou", el("i", null, "You"), p.you), el("span", "rvbot", el("i", null, "Bot"), p.bot)), button("primary rvback", "‹ Back to the review", closeBoard));
  board.id = "rvboard";
  screen.hidden = true;
  document.body.classList.add("watching", "rvboarding");
  document.body.append(board);
  document.body.style.setProperty("--watchbar", `${board.getBoundingClientRect().height}px`);
  S.peek(view);
  light(you, "rvlityou");
  light(bot, "rvlitbot");
}

function closeBoard() {
  if (!board) return;
  board.remove();
  board = null;
  document.body.classList.remove("watching", "rvboarding");
  S.unpeek();
  if (screen) screen.hidden = false;
}

// ---- the entry on the end screen -----------------------------------------------------------

function rowWords(run) {
  const was = job?.key === keyOf(run) ? job : saved(keyOf(run));
  if (was?.result) return was.result.picks.length ? "Ready" : "Ready: no scored move differs";
  if (was?.at) return `Comparing: ${Math.floor((100 * was.at) / Math.max(was.total, 1))}%`;
  return "The 3 moves the bot disagrees with most";
}

let rowRun = null;
function paintRow() {
  const sub = document.querySelector("#screen .reviewrun .sub");
  if (sub && rowRun) sub.textContent = rowWords(rowRun);
}

function onDraw(kind, v) {
  if (kind === "screen" && !board && v?.over && v.save.actions?.length) {
    const main = document.getElementById("screen");
    const s = v.save;
    if (!main.querySelector(".reviewrun")) {
      rowRun = { character: s.character, seed: s.seed, ascension: s.ascension, modifiers: s.modifiers ?? [], actions: s.actions, end: v.decision, floor: s.floor };
      const run = rowRun;
      const go = button("row watchbot reviewrun", "", () => open(run));
      go.append(el("span", "main", el("span", "lab", "Review this run"), el("span", "sub", rowWords(run))));
      const anchor = main.querySelector(".watchbot");
      if (anchor) anchor.after(go);
      else main.prepend(go);
    }
  }
  if (!screen) pump();
}
S.hook(onDraw);
if (S.view()?.over) onDraw("screen", S.view());

// Esc and the arrows drive the review, never the run under it. Back closes the board, then the review.
document.addEventListener(
  "keydown",
  (e) => {
    if (!screen) return;
    e.stopImmediatePropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      if (board) closeBoard();
      else close();
    } else if (!board && (e.key === "ArrowRight" || e.key === "ArrowLeft")) {
      e.preventDefault();
      screen.querySelector(e.key === "ArrowRight" ? ".rvnext" : ".rvprev")?.click();
    }
  },
  { capture: true },
);
window.addEventListener(
  "popstate",
  (e) => {
    if (!screen) return;
    e.stopImmediatePropagation();
    if (board) {
      closeBoard();
      history.pushState({ ...history.state, review: 1 }, "");
    } else close();
  },
  { capture: true },
);
