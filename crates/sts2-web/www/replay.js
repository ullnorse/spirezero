// Watch the bot (WEB10): the honest bot plays a finished run's seed and hero step by step in the
// Web Worker, drawn by the page's own renderer with the controls in a bar (play/pause, step,
// speed, exit), and a list of where the player's run differed from it: the first split, each run
// choice the bot would have made otherwise, and how both runs ended. Offered on the run's end
// screen and, for the last finished runs, on the start screen (`sts2.watch(run)` for others).
import { call, warm } from "./worker.js";

const S = window.sts2;
const RUNS = "sts2-bot-runs";
const HEROES = ["Ironclad", "Silent", "Defect", "Regent", "Necrobinder"];
const SPEEDS = [[1, 900], [2, 420], [4, 160], [0.5, 1800]];
// Frames the worker plays ahead of the screen.
const AHEAD = 24;
const KIND = { combat: "Fight", ancient: "Neow", event: "Event", map: "Map", rewards: "Rewards", "card-select": "Card choice", bundles: "Bundles", rest: "Rest site", treasure: "Treasure", shop: "Shop" };

let w = null; // the watch on screen

export function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null) e.append(k);
  return e;
}
export function button(cls, label, fn) {
  const b = el("button", cls, label);
  b.addEventListener("click", fn);
  return b;
}

const runs = () => JSON.parse(localStorage.getItem(RUNS) ?? "[]");
const endWord = (e) => ({ victory: "Victory", defeat: "Defeat" })[e] ?? "Stopped";
const outcomeLine = (o) => (o ? `${endWord(o.end)}, floor ${o.floor}${o.end === "victory" ? ` · ♥ ${o.hp}/${o.max_hp}` : ""}` : "…");

// A finished run kept for watching (the last eight, newest first).
function keep(v) {
  const s = v.save;
  const list = runs().filter((r) => !(r.seed === s.seed && r.character === s.character && r.ascension === s.ascension && r.actions.length === s.actions.length));
  list.unshift({ character: s.character, seed: s.seed, ascension: s.ascension, actions: s.actions, end: v.decision, floor: s.floor, at: Date.now() });
  localStorage.setItem(RUNS, JSON.stringify(list.slice(0, 8)));
}

// ---- watching -----------------------------------------------------------------------------

export function watch(run) {
  stop();
  const start = document.getElementById("start");
  w = { run, id: Date.now(), queue: [], shown: null, playing: true, speed: 0, done: false, asking: false, split: null, bot: null, differ: null, timer: 0 };
  start.hidden = true;
  document.body.classList.add("watching");
  const b = bar();
  document.body.append(b);
  document.body.style.setProperty("--watchbar", `${b.getBoundingClientRect().height}px`);
  caption(warm() ? "Starting…" : "Loading the bot…");
  const me = w;
  call("watch", { id: w.id, save: run })
    .then((r) => {
      if (w !== me) return;
      show(r.frame);
      pump();
      tick();
    })
    .catch(fail);
  call("differ", { save: run })
    .then((r) => {
      if (w !== me) return;
      w.differ = r;
      if (document.getElementById("watchlist")) list();
    })
    .catch(() => {});
}
S.watch = watch;
// History's "Watch the bot play this seed" (`history.js` `watchSeed`).
S.watchSeed = (d) => watch({ character: d.character, seed: d.seed, ascension: d.ascension, actions: d.actions ?? [] });

function fail(e) {
  if (w) caption("The bot stopped", String(e.message ?? e));
}

// Asks the worker for the next frame while fewer than AHEAD wait.
function pump() {
  if (!w || w.done || w.asking || w.queue.length >= AHEAD) return;
  w.asking = true;
  const me = w;
  call("step", { id: w.id })
    .then((r) => {
      if (w !== me || r.stale) return;
      w.asking = false;
      w.queue.push(r.frame);
      if (r.frame.split) w.split = r.frame.split;
      if (r.frame.done) {
        w.done = true;
        w.bot = r.frame.outcome;
      }
      if (document.getElementById("watchlist")) list();
      if (!w.shown || (w.playing && w.queue.length === 1)) status();
      pump();
    })
    .catch(fail);
}

function tick() {
  if (!w) return;
  clearTimeout(w.timer);
  if (w.playing && w.queue.length) show(w.queue.shift());
  w.timer = setTimeout(tick, SPEEDS[w.speed][1]);
  pump();
}

function show(frame) {
  w.shown = frame;
  S.draw(frame.view);
  status();
}

function status() {
  const f = w.shown;
  if (!f) return;
  const m = f.move;
  const behind = w.playing && !w.queue.length && !w.done;
  const end = f.done ? `Bot: ${outcomeLine(f.outcome)}` : null;
  caption(end ?? (m ? m.label : "The bot's run"), end ? null : [m?.why, behind ? "thinking…" : null].filter(Boolean).join(" · "));
  document.querySelector("#watchbar .play").textContent = w.playing ? "❚❚" : "▶";
}

function caption(main, sub = null) {
  const c = document.querySelector("#watchbar .cap");
  if (c) c.replaceChildren(el("b", null, main), sub ? el("small", null, sub) : null);
}

function bar() {
  const b = el("div");
  b.id = "watchbar";
  b.append(
    el("div", "cap"),
    el(
      "div",
      "ctl",
      button("wbtn exit", "✕", stop),
      button("wbtn play", "❚❚", () => {
        w.playing = !w.playing;
        status();
        tick();
      }),
      button("wbtn wstep", "Step", () => {
        w.playing = false;
        if (w.queue.length) show(w.queue.shift());
        else status();
        pump();
      }),
      button("wbtn speed", "1×", (e) => {
        w.speed = (w.speed + 1) % SPEEDS.length;
        e.currentTarget.textContent = `${SPEEDS[w.speed][0] === 0.5 ? "½" : SPEEDS[w.speed][0]}×`;
        tick();
      }),
      button("wbtn diff", "Where you differed", toggleList),
    ),
  );
  return b;
}

export function stop() {
  if (!w) return;
  clearTimeout(w.timer);
  w = null;
  call("stop").catch(() => {});
  document.getElementById("watchbar")?.remove();
  document.getElementById("watchlist")?.remove();
  document.body.classList.remove("watching");
  S.resume();
}

// ---- where you differed --------------------------------------------------------------------

function toggleList() {
  const open = document.getElementById("watchlist");
  if (open) return open.remove();
  list();
}

function choiceRow(d) {
  return el(
    "li",
    null,
    el("span", "where", `Floor ${d.floor} · ${KIND[d.kind] ?? d.kind}`),
    el("span", "you", el("i", null, "You "), d.you.label),
    el("span", "bot", el("i", null, "Bot "), d.bot.label, d.bot.why ? el("small", null, ` · ${d.bot.why}`) : null),
  );
}

function list() {
  let p = document.getElementById("watchlist");
  if (!p) {
    p = el("div");
    p.id = "watchlist";
    document.body.append(p);
  }
  const you = w.differ?.outcome ?? { end: w.run.end, floor: w.run.floor };
  const kids = [
    el("div", "lhead", el("h2", null, "Where you differed"), button("wbtn", "✕", () => p.remove())),
    el("div", "ends", el("span", null, el("i", null, "You "), outcomeLine(you)), el("span", null, el("i", null, "Bot "), w.bot ? outcomeLine(w.bot) : `playing, floor ${w.shown?.view.save.floor ?? 0}`)),
  ];
  if (w.split) kids.push(el("h3", null, "First split"), el("ul", "diffs", choiceRow(w.split)));
  else kids.push(el("p", "note", w.done ? "The bot played your run move for move." : "No split yet: the bot has played as you so far."));
  // The first split, when it is a run choice, is the list's first row too: once is enough.
  const same = (d) => w.split && d.floor === w.split.floor && d.you.label === w.split.you.label;
  const d = w.differ?.differ.filter((x) => !same(x));
  kids.push(el("h3", null, "Your choices the bot would make otherwise"));
  if (!d) kids.push(el("p", "note", "Reading your run…"));
  else if (!d.length) kids.push(el("p", "note", "None: every choice outside the fights was the bot's."));
  else kids.push(el("ul", "diffs", ...d.map(choiceRow)));
  p.replaceChildren(el("div", "lpanel", ...kids));
}

// ---- the entries: the end screen, the start screen -----------------------------------------

function onDraw(kind, v) {
  if (w) {
    // Something redrew the player's view under the watch (Back opened the menu): the frame again.
    if (kind === "screen" && w.shown && v !== w.shown.view) S.draw(w.shown.view);
    return;
  }
  if (kind === "start") return startEntry();
  if (kind !== "screen" || !v?.over || !v.save.actions?.length) return;
  keep(v);
  const main = document.getElementById("screen");
  if (main.querySelector(".watchbot")) return;
  const s = v.save;
  const go = button("row watchbot", "", () => watch({ character: s.character, seed: s.seed, ascension: s.ascension, actions: s.actions, end: v.decision, floor: s.floor }));
  go.append(el("span", "main", el("span", "lab", "Watch the bot play this seed"), el("span", "sub", "Its run, step by step, and where yours differed")));
  const anchor = main.querySelector(".list");
  if (anchor) anchor.before(go);
  else main.append(go);
}
S.hook(onDraw);
// This module loads after the page's first drawing: a run that has already ended is kept, and the
// start screen, if it is up, gets its entries.
if (S.view()?.over) onDraw("screen", S.view());
if (!document.getElementById("start").hidden) onDraw("start", S.view());

function startEntry() {
  const root = document.querySelector("#start .startin");
  const list = runs().slice(0, 3);
  if (!root || !list.length || root.querySelector(".watchruns")) return;
  const box = el("div", "watchruns", el("h3", null, "Watch the bot play your last runs"));
  for (const r of list) {
    const b = button("row watchbot", "", () => watch(r));
    b.append(el("span", "main", el("span", "lab", `${HEROES[r.character] ?? "?"} · ${r.seed}`), el("span", "sub", `${endWord(r.end)}, floor ${r.floor}${r.ascension ? ` · A${r.ascension}` : ""}`)));
    box.append(b);
  }
  const foot = root.querySelector(".foot");
  if (foot) foot.before(box);
  else root.append(box);
}

// The keyboard drives the watch, never the run under it: Space plays or pauses, → steps, Esc exits.
document.addEventListener(
  "keydown",
  (e) => {
    if (!w) return;
    e.stopImmediatePropagation();
    e.preventDefault();
    if (e.key === "Escape") stop();
    else if (e.key === " ") document.querySelector("#watchbar .play")?.click();
    else if (e.key === "ArrowRight") document.querySelector("#watchbar .wstep")?.click();
  },
  { capture: true },
);
window.addEventListener("popstate", () => w && stop());
