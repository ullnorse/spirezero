// Ask the bot (WEB10, WEB27): a "Hint" button in the dock of a fight or a choice asks the honest
// bot (in a Web Worker, worker.js) for its move from what the player sees, lights the button(s) it
// would tap and says it in one line with its reason (a fight's win chance, a choice's confidence).
// With the "Bot hints" setting on, every decision is asked in the background and only lit (a thin
// gold outline, no text); the Hint button then adds the reason line.
import { call, warm } from "./worker.js";
import "./replay.js";
import "./review.js";

const S = window.sts2;
const CHOICES = new Set(["ancient", "event", "map", "rewards", "card-select", "bundles", "rest", "treasure", "shop"]);
const link = Object.assign(document.createElement("link"), { rel: "stylesheet", href: "bot.css" });
document.head.append(link);

// The advice shown: for which position (`at`), and the bot's answer once it is in.
let advice = null;
const keyOf = (v) => `${v.save.seed}|${v.save.actions?.length}`;
// The screen an advice's tap is lit on: a tap that changes it (a card aimed, a shop's section
// opened, a "Tap again") moves the light to the next tap.
const screenOf = (v) => JSON.stringify(v.screen);
const watching = () => document.body.classList.contains("watching");
const always = () => S.pref("hints") === "always";
// The last answer's time, for the headless proof's phone timing.
window.sts2bot = { ms: null, label: null };

function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null) e.append(k);
  return e;
}

function say(main, sub = null, busy = false) {
  let b = document.getElementById("botsay");
  if (!b) {
    b = el("div");
    b.id = "botsay";
    b.setAttribute("role", "status");
    b.addEventListener("click", () => {
      advice = null;
      unmark();
      b.hidden = true;
    });
    document.body.append(b);
  }
  b.hidden = false;
  b.classList.toggle("busy", busy);
  b.replaceChildren(...[el("span", "who", "Bot"), el("b", null, main), sub ? el("small", null, sub) : null].filter(Boolean));
  place();
}

function unmark() {
  for (const e of document.querySelectorAll(".botpick, .botaim")) e.classList.remove("botpick", "botaim", "botquiet");
}

// Lights the button of the advice's next tap (a card, a row, a room, End Turn, an aimed enemy),
// the creature it aims at and a potion to drink.
function mark(a, step, quiet) {
  unmark();
  const id = a.taps[step];
  const sel = (p) => `${p} [data-bid="${id}"], ${p} [data-tap="${id}"]`;
  if (id != null) for (const e of document.querySelectorAll(`${sel("#screen")}, ${sel("#dock")}, ${sel("#sheet")}, #screen g.node[data-button="${id}"]`)) e.classList.add("botpick", ...(quiet ? ["botquiet"] : []));
  // A card reward is drawn as its cards (tapping one takes it): the advice's first tap, opening the reward, lights
  // the card the label names.
  if (id != null && !document.querySelector(`#screen [data-bid="${id}"], #screen [data-tap="${id}"]`)) {
    const name = /→\s*(.+)$/.exec(a.label ?? "")?.[1]?.trim();
    if (name) for (const e of document.querySelectorAll("#screen .peekreward .card")) if (e.textContent.includes(name)) e.classList.add("botpick", ...(quiet ? ["botquiet"] : []));
  }
  for (const aim of a.aims) for (const e of document.querySelectorAll(`#screen .enemy[data-id="${aim}"]`)) e.classList.add("botaim", ...(quiet ? ["botquiet"] : []));
  if (a.potion != null) document.querySelector(`#top [data-slot="${a.potion}"]`)?.classList.add("botpick", ...(quiet ? ["botquiet"] : []));
  if (!quiet) {
    reveal();
    place();
  }
}

// WEB65: the lit target is brought on screen (clear of the top bar and the dock) when the player
// asked: a shop's row below the fold, a map room out of the map's view.
function reveal() {
  const t = document.querySelector("#screen .botpick, #sheet .botpick");
  if (!t) return;
  const r = t.getBoundingClientRect();
  const top = document.getElementById("top")?.getBoundingClientRect().bottom ?? 0;
  const dock = document.getElementById("dock");
  const bottom = dock && !dock.hidden ? dock.getBoundingClientRect().top : innerHeight;
  if (r.top < top || r.bottom > bottom) t.scrollIntoView({ block: "center", inline: "nearest" });
}

// The bubble goes where it covers no title, no enemy's intent and nothing lit: under the top bar,
// else just above the dock, else over the gap between the enemies and you.
function place() {
  const b = document.getElementById("botsay");
  if (!b || b.hidden) return;
  const clear = [...document.querySelectorAll("#screen .head h1, #screen .pictext h1, #screen .pictitle, #screen .enemy .intent, .botpick, .botaim, #sheet:not([hidden]) .panel")]
    .map((e) => e.getBoundingClientRect())
    .filter((r) => r.width && r.height);
  const top = document.getElementById("top")?.getBoundingClientRect().bottom ?? 0;
  const dock = document.getElementById("dock");
  const bottom = dock && !dock.hidden ? dock.getBoundingClientRect().top : innerHeight;
  const h = b.getBoundingClientRect().height;
  const field = document.querySelector("#screen .field")?.getBoundingClientRect();
  const spots = [top + 6, bottom - h - 6];
  if (field) spots.push(field.bottom + 2);
  const hits = (y) => clear.reduce((n, r) => n + Math.max(0, Math.min(r.bottom, y + h) - Math.max(r.top, y)), 0);
  const best = spots.reduce((a, y) => (hits(y) < hits(a) ? y : a), spots[0]);
  b.style.top = `${Math.round(best)}px`;
}

// `show`: the player asked (the line and the full light); else it is the setting's background ask
// (the light only, and nothing while the answer is not in).
async function ask(show = true) {
  const v = S.view();
  if (!v) return;
  const at = keyOf(v);
  if (advice?.at === at) {
    if (!show || advice.show) return;
    advice.show = true;
    if (!advice.data) return say(warm() ? "Thinking…" : "Loading the bot…", null, true);
    say(advice.data.label, advice.data.why);
    return mark(advice.data, advice.step);
  }
  advice = { at, busy: true, show };
  if (show) say(warm() ? "Thinking…" : "Loading the bot…", null, true);
  const { character, seed, ascension, actions } = v.save;
  try {
    const r = await call("advise", { save: { character, seed, ascension, actions } });
    if (advice?.at !== at) return;
    window.sts2bot = { ms: r.ms, label: r.advice?.label ?? null };
    if (!r.advice) return advice.show ? say("Nothing to do here") : null;
    // The taps from the page's own screen (a shop's section may be open), else the worker's.
    if (r.advice.index != null) Object.assign(r.advice, S.taps(r.advice.index) ?? {});
    window.sts2bot.label = r.advice.label;
    const shown = advice.show;
    advice = { at, data: r.advice, step: 0, screen: screenOf(S.view()), show: shown };
    if (shown) say(r.advice.label, r.advice.why);
    mark(r.advice, 0, !shown);
    if (shown) place();
  } catch (e) {
    const shown = advice?.show;
    if (advice?.at === at) advice = { at, failed: true, show: shown };
    if (shown) say("The bot could not answer", String(e.message ?? e));
  }
}

// The dock's Hint button: beside End Turn or the primary button, else on a row of its own. The
// dock's height is measured again, as it was drawn without it.
function askButton() {
  const b = el("button", "hintbtn", "Hint");
  b.id = "askbot";
  b.setAttribute("aria-label", "Ask the bot");
  b.addEventListener("click", () => ask(true));
  return b;
}

function placeButton() {
  const dock = document.getElementById("dock");
  if (!dock || dock.querySelector("#askbot")) return;
  const b = askButton();
  const bar = dock.querySelector(".fightbar");
  const prim = dock.querySelector(".primary.wide");
  if (bar) bar.insertBefore(b, bar.querySelector(".endturn"));
  else if (prim) {
    const row = el("div", "hintrow");
    prim.replaceWith(row);
    row.append(b, prim);
  } else dock.prepend(el("div", "hintrow", b));
  dock.hidden = false;
  document.body.style.setProperty("--dock", `${dock.getBoundingClientRect().height}px`);
}

function onDraw(kind, v) {
  if (kind === "start" || !v) return;
  if (watching()) return;
  const can = !v.over && (v.fight || CHOICES.has(v.decision));
  if (can) placeButton();
  if (advice && (advice.at !== keyOf(v) || !can)) {
    advice = null;
    unmark();
    const bubble = document.getElementById("botsay");
    if (bubble) bubble.hidden = true;
  }
  if (can && always() && !advice) ask(false);
  if (!advice?.data) return;
  const screen = screenOf(v);
  if (screen !== advice.screen) {
    advice.screen = screen;
    advice.step++;
  }
  if (advice.step < advice.data.taps.length || advice.data.potion != null) mark(advice.data, advice.step, !advice.show);
  else unmark();
}
S.hook(onDraw);
// This module loads after the page's first drawing: add to what is on screen now.
onDraw("screen", S.view());
