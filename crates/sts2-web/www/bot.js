// Bot hints (WEB10, WEB77): with the setting on (Settings, or the top bar's toggle) every decision is
// asked of the honest bot (in a Web Worker, worker.js) and its pick glows on the board: the card and
// its target, a map room, an event option, a reward, a shop ware, a rest choice. No text.
// The combat net (WEB82): with "Net plays fights" on, every fight decision is the CB3 net's (worker.js,
// `netAdvise`), lit like a hint, then played after a short pause; "Net: one move" plays one. The player
// plays everything outside fights. Shown only where the weights (net/cb3.bin) are served.
import { call } from "./worker.js";
import { icon } from "./icons.js";
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
window.sts2bot = { ms: null, label: null, ask: () => ask(), net: { ms: null, label: null, moves: 0 } };

function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null) e.append(k);
  return e;
}

function unmark() {
  for (const e of document.querySelectorAll(".botpick, .botaim")) e.classList.remove("botpick", "botaim");
}

// Lights the button of the advice's next tap (a card, a row, a room, End Turn, an aimed enemy),
// the creature it aims at and a potion to drink.
function mark(a, step) {
  unmark();
  const id = a.taps[step];
  const sel = (p) => `${p} [data-bid="${id}"], ${p} [data-tap="${id}"]`;
  if (id != null) for (const e of document.querySelectorAll(`${sel("#screen")}, ${sel("#dock")}, ${sel("#sheet")}, #screen g.node[data-button="${id}"]`)) e.classList.add("botpick");
  // A card reward is drawn as its cards (tapping one takes it): the advice's first tap, opening the reward, lights
  // the card the label names.
  if (id != null && !document.querySelector(`#screen [data-bid="${id}"], #screen [data-tap="${id}"]`)) {
    const name = /→\s*(.+)$/.exec(a.label ?? "")?.[1]?.trim();
    if (name) for (const e of document.querySelectorAll("#screen .peekreward .card")) if (e.textContent.includes(name)) e.classList.add("botpick");
  }
  for (const aim of a.aims) for (const e of document.querySelectorAll(`#screen .enemy[data-id="${aim}"]`)) e.classList.add("botaim");
  if (a.potion != null) document.querySelector(`#top [data-slot="${a.potion}"]`)?.classList.add("botpick");
}

// The background ask: the bot's pick is lit on the board (no text) while hints are on.
async function ask() {
  const v = S.view();
  if (!v) return;
  const at = keyOf(v);
  if (advice?.at === at) return;
  advice = { at, busy: true };
  const { character, seed, ascension, actions } = v.save;
  try {
    const r = await call("advise", { save: { character, seed, ascension, actions } });
    if (advice?.at !== at) return;
    Object.assign(window.sts2bot, { ms: r.ms, label: r.advice?.label ?? null });
    if (!r.advice) return;
    // The taps from the page's own screen (a shop's section may be open), else the worker's.
    if (r.advice.index != null) Object.assign(r.advice, S.taps(r.advice.index) ?? {});
    window.sts2bot.label = r.advice.label;
    advice = { at, data: r.advice, step: 0, screen: screenOf(S.view()) };
    mark(r.advice, 0);
  } catch {
    if (advice?.at === at) advice = { at, failed: true };
  }
}

// The top bar's hint toggle (the same setting as Settings): lit while hints are on.
function placeToggle() {
  const stats = document.querySelector("#top .stats");
  if (!stats || stats.querySelector("#askbot")) return;
  const b = el("button", `iconbtn askbot${always() ? " on" : ""}`, icon("bulb"));
  b.id = "askbot";
  b.setAttribute("aria-label", "Bot hints");
  b.setAttribute("aria-pressed", String(always()));
  b.addEventListener("click", () => {
    S.setPref("hints", always() ? "manual" : "always");
    b.classList.toggle("on", always());
    b.setAttribute("aria-pressed", String(always()));
    if (always()) onDraw("screen", S.view());
    else clear();
  });
  stats.append(b);
}

function clear() {
  advice = null;
  unmark();
}

function onDraw(kind, v) {
  if (kind === "start" || !v) return;
  if (watching()) return;
  const can = !v.over && (v.fight || CHOICES.has(v.decision));
  if (!v.over) {
    placeToggle();
    placeNet();
  }
  syncNet(v);
  // The net's move is lit until it is played (and the advisor waits while it is asked).
  if (netMove) {
    if (netMove.data) mark(netMove.data, 0);
    return;
  }
  if (advice && (advice.at !== keyOf(v) || !can || !always())) clear();
  if (can && always() && !advice) ask();
  if (!advice?.data) return;
  const screen = screenOf(v);
  if (screen !== advice.screen) {
    advice.screen = screen;
    advice.step++;
  }
  if (advice.step < advice.data.taps.length || advice.data.potion != null) mark(advice.data, advice.step);
  else unmark();
}
// ---- the combat net ------------------------------------------------------------------------

// The pause before a lit move is played, per speed (Settings).
const NET_PAUSE_MS = { slow: 1500, normal: 600, fast: 250 };
const netPause = () => NET_PAUSE_MS[S.pref("netSpeed")] ?? NET_PAUSE_MS.normal;
const netAuto = () => S.pref("netFights") === true;
// Settings → Net potions: by default the net keeps its potions for elites and bosses (the worker's flag).
const netSavesPotions = () => S.pref("netPotions") !== "any";
// Whether the weights are served (checked once); the controls are placed only then.
let netServed = null;
fetch("net/cb3.bin", { method: "HEAD" })
  .then((r) => (netServed = r.ok))
  .catch(() => (netServed = false))
  .then(() => onDraw("screen", S.view()));
// The move asked or lit: `{at, data}` (data once the answer is in); `gen` drops answers no longer wanted.
let netMove = null;
let netAsking = false;
let netGen = 0;
let netTimer = null;

// Where the net may move: the player's own run, in a fight, nothing over it.
function netCan(v) {
  if (!v || v.over || !v.fight || watching()) return false;
  // A start screen fading out (`leaving`, hidden only after the run's first drawing) is gone already.
  const start = document.getElementById("start");
  if (start && !start.hidden && !start.classList.contains("leaving")) return false;
  if (document.querySelector("#sheet:not([hidden]):not(.pinned)")) return false;
  const review = document.getElementById("review");
  return !review || review.hidden;
}

// Drops the asked or lit move.
function netDrop() {
  netGen++;
  clearTimeout(netTimer);
  netTimer = null;
  if (netMove) unmark();
  netMove = null;
}

// Asks the net's move here, lights it, and plays it after the pause; `auto` moves stop when the toggle
// goes off. A move whose position changed in the meantime is dropped.
async function netStep(auto) {
  const v = S.view();
  if (netAsking || netMove || !netCan(v)) return;
  const at = keyOf(v);
  const gen = ++netGen;
  netAsking = true;
  netMove = { at, auto };
  const { character, seed, ascension, modifiers, actions } = v.save;
  let r = null;
  try {
    r = await call("netAdvise", { save: { character, seed, ascension, modifiers, actions }, savePotions: netSavesPotions() });
  } catch {
    r = null;
  } finally {
    netAsking = false;
  }
  const still = () => gen === netGen && netCan(S.view()) && keyOf(S.view()) === at && (!auto || netAuto());
  if (!still() || r?.advice?.index == null) {
    if (gen === netGen) netDrop();
    return;
  }
  Object.assign(r.advice, S.taps(r.advice.index) ?? {});
  Object.assign(window.sts2bot.net, { ms: r.ms, label: r.advice.label });
  netMove = { at, auto, data: r.advice };
  mark(r.advice, 0);
  netTimer = setTimeout(() => {
    netTimer = null;
    if (!still()) return netDrop();
    netMove = null;
    unmark();
    window.sts2bot.net.moves++;
    S.play(r.advice.index);
    // The next move (auto) is asked by the drawing this play makes.
  }, netPause());
}

// After every drawing: a move whose position is gone is dropped; with the toggle on, the next one is asked.
function syncNet(v) {
  document.getElementById("netone")?.toggleAttribute("disabled", !netCan(v));
  if (netMove && (netMove.at !== keyOf(v) || !netCan(v) || (netMove.auto && !netAuto()))) netDrop();
  if (netAuto() && netServed && netCan(v)) netStep(true);
}

// The top bar's net buttons, "Net plays fights" (lit while on) and "Net: one move": with the hint toggle on a
// wide screen; on a phone at the end of the potions and relics row (the stats row is full).
function placeNet() {
  const tray = document.querySelector("#top .tray");
  if (!netServed || !tray || tray.querySelector("#netbot")) return;
  const auto = el("button", `iconbtn netbot${netAuto() ? " on" : ""}`, icon("net"));
  auto.id = "netbot";
  auto.title = "Net plays fights";
  auto.setAttribute("aria-label", "Net plays fights");
  auto.setAttribute("aria-pressed", String(netAuto()));
  auto.addEventListener("click", () => {
    S.setPref("netFights", !netAuto());
    auto.classList.toggle("on", netAuto());
    auto.setAttribute("aria-pressed", String(netAuto()));
    if (!netAuto()) netDrop();
    onDraw("screen", S.view());
  });
  const one = el("button", "iconbtn netone", icon("step"));
  one.id = "netone";
  one.title = "Net: one move";
  one.setAttribute("aria-label", "Net: one move");
  one.addEventListener("click", () => netStep(false));
  tray.append(auto, one);
}

S.hook(onDraw);
// This module loads after the page's first drawing: add to what is on screen now.
onDraw("screen", S.view());
