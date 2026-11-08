// The phone page: draws sts2-view's screens (the JSON `Web.view()` returns) and sends taps back.
// Nothing here knows a game rule: every button is the model's, every number the engine's.
import { report as bugReport, deliver as bugDeliver, decodeRun } from "./bugreport.js";
import init, { Web, load_text, text_tables, modifier_list, random_modifiers, daily_run, library_counts, library_part, library_glossary } from "./pkg/sts2_web.js";
import { icon, iconSvg, GLYPH_ICON } from "./icons.js";
import { routePlanner } from "./map.js";
import * as fx from "./combatfx.js";
import * as ffx from "./fightfx.js";
import { play as sound, setSound, setTrack, unlock as unlockSound } from "./sound.js";
import { initHover, attachTip, hideTip } from "./hover.js";
import { initExtras, startExtras, shareThisRun, recordRun, attachPreview, upgradeSection, takeSharedLink } from "./extras.js";
import { pairBlock } from "./preview.js";

const SAVE = "sts2-save-v1";
const PREFS = "sts2-prefs";
// The characters in the game's select order; `id` is the engine's index (`Character::ALL`:
// Ironclad, Silent, Defect, Regent, Necrobinder), the number a run and a save are started with.
const CHARACTERS = [
  { id: 0, name: "Ironclad" },
  { id: 1, name: "Silent" },
  { id: 3, name: "Regent" },
  { id: 4, name: "Necrobinder" },
  { id: 2, name: "Defect" },
];
const character = (id) => CHARACTERS.find((c) => c.id === id) ?? CHARACTERS[0];

// The player's own game art (`cargo xtask art-import`, linked in by serve.sh as `art/`; never
// committed): `kind/ID` → its file, from `art/index.tsv`. Without it every picture falls back to
// the drawing it replaces.
// serve.py names the art's own address (`/a/<art id>/`), which outlives deploys, so a phone downloads
// each picture once per art import, not once per build.
const ART = document.querySelector('meta[name="art"]')?.content ?? "art/";
const artFiles = new Map();
const artSizes = new Map();
async function loadArt() {
  const r = await fetch(`${ART}index.tsv`).catch(() => null);
  if (!r?.ok) return;
  for (const line of (await r.text()).split("\n")) {
    const [kind, id, file, w, h] = line.split("\t");
    if (file) artFiles.set(`${kind}/${id}`, file);
    if (file) artSizes.set(`${kind}/${id}`, [Number(w), Number(h)]);
  }
  // The game's button planks for the page's own buttons (teal; red while it asks to confirm).
  const plank = artFiles.get("ui/popup_confirm_button");
  const red = artFiles.get("ui/popup_cancel_button");
  if (plank) {
    document.body.classList.add("gameart");
    document.documentElement.style.setProperty("--plank", `url("${ART}${plank}")`);
    if (red) document.documentElement.style.setProperty("--plank-red", `url("${ART}${red}")`);
  }
}
// Fetches the fight pictures in the background once (enemies, intents, cards, powers, then relics
// and potions), two at a time, so a fight's art is already here when it starts. The worker and the
// browser keep them (`immutable`), so later visits fetch nothing. Skipped when the phone saves data.
function warmArt() {
  if (navigator.connection?.saveData || new URLSearchParams(location.search).has("proof")) return;
  const order = ["monster", "intent", "card", "power", "character", "relic", "potion"];
  const kind = (k) => order.indexOf(k.split("/")[0]);
  const queue = [...new Set([...artFiles].filter(([k]) => kind(k) >= 0).sort(([a], [b]) => kind(a) - kind(b)).map(([, f]) => f))];
  const next = () => {
    const f = queue.shift();
    if (f) fetch(`${ART}${f}`, { priority: "low" }).catch(() => {}).finally(() => setTimeout(next, 0));
  };
  const go = () => { next(); next(); };
  if (window.requestIdleCallback) requestIdleCallback(go, { timeout: 3000 });
  else setTimeout(go, 1500);
}
// A picture of `kind`/`id` (`card`, `monster`, `relic`, `potion`, `power`, `character`), or null.
function artImg(kind, id, cls = "art") {
  const file = id && artFiles.get(`${kind}/${id}`);
  if (!file) return null;
  const img = el("img", cls);
  img.src = `${ART}${file}`;
  img.alt = "";
  img.decoding = "async";
  img.draggable = false;
  return img;
}
// A character's portrait, else its initial.
const portrait = (c, cls = "mono") => artImg("character", c.name.toUpperCase(), `${cls} portrait`) ?? el("span", cls, c.name[0]);

const $ = (id) => document.getElementById(id);
// Motion off by the settings, or by the phone's when the settings say "match".
const reduced = () => prefs.motion === "reduced" || (prefs.motion !== "full" && matchMedia("(prefers-reduced-motion: reduce)").matches);
let game = null;
let view = null;
let before = null; // the view before the last input, for the hit animations
let local = null; // a sheet of the page's own: a potion, help, settings
let peeked = null; // what the page showed before `peek` drew another moment
let choiceHidden = false;
let textTables = 0;
// Other modules' additions to a drawn page (bot.js, odds.js): each gets the kind ("screen",
// "sheet", "start") and the view.
const hooks = [];
const runHooks = (kind) => hooks.forEach((f) => f(kind, view));
let prefs = { theme: "dark", sound: true, volume: "low", motion: "auto", text: "normal", confirmEnd: false, hints: "manual", keepAwake: true, vibration: true, ...JSON.parse(localStorage.getItem(PREFS) ?? "{}") };
const savePrefs = () => localStorage.setItem(PREFS, JSON.stringify(prefs));
// A haptic tick or pattern (platform.js: the Vibration setting); iOS has no `vibrate`.
const buzz = (p) => prefs.vibration && navigator.vibrate?.(p);

// ---- small DOM helpers ------------------------------------------------------------------

function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids.flat()) if (k != null && k !== false) e.append(k);
  return e;
}

function tap(e, fn) {
  e.addEventListener("click", (ev) => {
    ev.stopPropagation();
    fn(ev);
  });
  return e;
}

// Rich is [{role, bold, text}].
function rich(r, cls = null) {
  const frag = el("span", cls);
  for (const s of r ?? []) frag.append(el("span", `r-${s.role}${s.bold ? " b" : ""}`, s.text));
  return frag;
}
const plain = (r) => (r ?? []).map((s) => s.text).join("");
const stateOf = (b) => (typeof b.state === "string" ? b.state : Object.keys(b.state)[0]);
const dimReason = (b) => (typeof b.state === "object" ? b.state.Dim : null);
const blocksOf = (s, kind) => s.blocks.filter((b) => b[kind] !== undefined).map((b) => b[kind]);
// The rows' second-tap questions name keys; on a phone the answer is a tap.
const forTouch = (t) => t.replace(/\? (Enter again|press \S+)$/, "? Tap again");

// ---- the run ------------------------------------------------------------------------------

function apply(json) {
  before = view;
  view = JSON.parse(json);
  persist();
  const leave = view.leave;
  render();
  if (leave) onLeave(leave);
}

function send(input) {
  if (!game) return;
  buzz(6);
  apply(game.input(JSON.stringify(input)));
}

function open(name) {
  if (!game) return;
  local = null;
  apply(game.open(name));
}

let keepSave = true; // false while a replayed bug report is on screen
function persist() {
  if (!view || !keepSave) return;
  recordRun(view);
  if (view.over) localStorage.removeItem(SAVE);
  else localStorage.setItem(SAVE, JSON.stringify({ ...view.save, at: Date.now() }));
}

function onLeave(leave) {
  if (leave === "select") showStart();
  else if (leave === "new-seed") startRun(view.save.character, randomSeed(), view.save.ascension);
  else if (leave === "settings") showLocal(settingsSheet());
  else if (leave === "help") showLocal(helpSheet());
}

function randomSeed() {
  const abc = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
  let s = "";
  for (let i = 0; i < 10; i++) s += abc[Math.floor(Math.random() * abc.length)];
  return s;
}

// `modifiers` are the Custom Run's keys; `daily` marks the run as that day's.
function startRun(character, seed, ascension, modifiers = [], daily = false) {
  try {
    game = new Web(character, seed, ascension, JSON.stringify(modifiers));
    fightSlots = null;
    keepSave = true;
    game.set_practice(!daily && !!startOpts.practice);
    if (daily) game.set_daily(true);
    game.set_flat_shop?.(true);
  } catch (e) {
    showStart(String(e));
    return;
  }
  local = null;
  before = null;
  view = null;
  hideStart();
  guardBack();
  apply(game.view());
  if (!prefs.tutored) showLocal(tutorialSheet());
}

function guardBack() {
  if (!history.state?.run) history.pushState({ run: 1 }, "");
}

function continueRun() {
  const save = JSON.parse(localStorage.getItem(SAVE) ?? "null");
  if (!save) return;
  resumeSave(save, false);
}

// A pasted bug report's code (`/?replay=`): the run played to the same place, kept out of the
// player's own save.
function replayCode(code) {
  const save = decodeRun(code);
  if (!save) return showStart("That replay code is not valid.");
  resumeSave(save, true);
}

function resumeSave(save, ephemeral) {
  try {
    game = Web.replay(save.character, save.seed, save.ascension, JSON.stringify(save.modifiers ?? []), JSON.stringify(save.actions));
    fightSlots = null;
    game.set_practice(!!save.practice);
    game.set_flat_shop?.(true);
  } catch (e) {
    showStart(`The save could not be loaded: ${e}`);
    return;
  }
  keepSave = !ephemeral;
  local = null;
  before = null;
  view = null;
  hideStart();
  guardBack();
  apply(game.view());
}

function hideStart() {
  const root = $("start");
  if (reduced() || document.body.classList.contains("proof")) {
    root.hidden = true;
    return;
  }
  root.classList.add("leaving");
  setTimeout(() => {
    root.hidden = true;
    root.classList.remove("leaving");
  }, 260);
}

// ---- drawing ------------------------------------------------------------------------------

// Each input redraws the page from scratch. A new <img> may paint blank for a frame while it
// decodes, so the pictures blink; the redraw takes the last page's <img> of the same picture
// instead (the element keeps its decoded image).
function keepImages(roots) {
  const pool = new Map();
  for (const r of roots) for (const img of r.querySelectorAll("img")) {
    const key = img.getAttribute("src");
    if (!pool.has(key)) pool.set(key, []);
    pool.get(key).push(img);
  }
  return () => {
    for (const r of roots) for (const img of r.querySelectorAll("img")) {
      const old = pool.get(img.getAttribute("src"))?.shift();
      if (!old || old === img) continue;
      for (const { name } of [...old.attributes]) if (name !== "src") old.removeAttribute(name);
      for (const { name, value } of [...img.attributes]) if (name !== "src") old.setAttribute(name, value);
      img.replaceWith(old);
    }
  };
}

// WEB67: the fight's card tiles as they stood before this redraw (rect and a copy, by card id),
// so a card exhausted out of the hand or a choice burns where it was (fightfx.js).
let tilesBefore = new Map();
function keepTiles() {
  tilesBefore = new Map();
  if (!document.body.classList.contains("fight") || reduced() || document.body.classList.contains("proof")) return;
  // The hand last, so its tile wins over a choice's copy of the same card.
  for (const t of document.querySelectorAll("#sheet .card[data-card], #screen .card[data-card]:not(.hand .card), #screen .hand .card[data-card]")) {
    const rect = t.getBoundingClientRect();
    if (rect.width) tilesBefore.set(Number(t.dataset.card), { rect, ghost: t.cloneNode(true) });
  }
}

function render() {
  const s = view.screen;
  hideTip();
  keepTiles();
  const reuse = keepImages([$("top"), $("screen"), $("dock")]);
  document.body.dataset.decision = view.decision;
  document.body.classList.toggle("fight", view.fight);
  document.body.classList.toggle("over", view.over);
  document.body.classList.toggle("room", !view.fight && !view.over && view.decision !== "map");
  renderTop(s.top);
  const main = $("screen");
  // A fight's title is only the aim prompt: arming a card must not replay the screen's entrance.
  const key = `${view.decision}|${view.fight ? "" : plain(s.title)}|${view.save.floor}`;
  const fresh = main.dataset.key !== key;
  main.dataset.key = key;
  main.replaceChildren();
  if (view.fight) renderFight(s, main);
  else {
    fightSlots = null;
    renderRoom(s, main);
  }
  if (fresh && view.decision === "map") mapFresh = true;
  if (fresh) {
    main.classList.remove("enter", "fightin", "mapin");
    void main.offsetWidth;
    main.classList.add("enter", view.fight && !before?.fight ? "fightin" : view.decision === "map" ? "mapin" : "roomin");
    if (!view.fight && view.decision !== "map") window.scrollTo(0, 0);
  }
  renderDock(s);
  reuse();
  renderSheet();
  toast(s.status);
  animate();
  turnMoment();
  endMoment();
  if (!peeked) music();
  keepFocus();
  runHooks("screen");
}

// ---- the keyboard -------------------------------------------------------------------------
// The arrows move a focus ring between what can be pressed on the screen (the sheet's, while one
// is open), in the direction pressed; Enter or Space presses it. The numbers and `e` are the
// game's own keys (as the TUI's: a card, a target, a row, End Turn), Esc goes back or closes.

let kbd = null; // what the focus rests on, by its identity, while the keyboard drives

function pressables() {
  const scope = document.querySelector("#sheet:not([hidden])") ?? document;
  const all = [...scope.querySelectorAll("#start:not([hidden]) button, #top button, #screen button, #dock button, #sheet button, #screen g.node.reach")];
  return all.filter((e) => !e.disabled && e.getClientRects().length && e.getBoundingClientRect().width > 0);
}

const identity = (e) =>
  e.dataset.card ? `card:${e.dataset.card}` : e.dataset.id ? `id:${e.dataset.id}` : e.dataset.button ? `node:${e.dataset.button}` : `${e.className?.baseVal ?? e.className}|${e.textContent.trim().slice(0, 32)}`;

function focusOn(e) {
  if (!e) return;
  for (const x of document.querySelectorAll(".kbd")) x.classList.remove("kbd");
  e.classList.add("kbd");
  if (e.focus) e.focus({ preventScroll: true });
  e.scrollIntoView?.({ block: "nearest" });
  kbd = identity(e);
}

// Where the focus starts: the screen's own choices (the hand, a list, cards, the next rooms on the
// map or as buttons, a sheet's rows), never the top bar's buttons or the map's legend.
const firstChoice = (items) =>
  items.find((e) => e.closest(".hand, .list, .cards, .field, .nextroom, #sheet") || e.classList.contains("nextroom") || e.matches("g.node.reach")) ??
  items.find((e) => !e.closest("#top")) ??
  items[0];

// After a redraw the focus finds what it rested on again (else the screen's first choice).
function keepFocus() {
  if (!kbd) return;
  const items = pressables();
  focusOn(items.find((e) => identity(e) === kbd) ?? firstChoice(items));
}

// The nearest pressable from `from` in direction `dx`,`dy`: ahead of it, the gap along the
// direction counting less than the step across it.
function nearest(from, dx, dy, items) {
  const c = (e) => {
    const r = e.getBoundingClientRect();
    return [r.left + r.width / 2, r.top + r.height / 2];
  };
  const [x0, y0] = c(from);
  let best = null;
  let score = Infinity;
  for (const e of items) {
    if (e === from) continue;
    const [x, y] = c(e);
    const along = (x - x0) * dx + (y - y0) * dy;
    if (along <= 4) continue;
    const across = Math.abs((x - x0) * dy) + Math.abs((y - y0) * dx);
    const s = along + 2.5 * across;
    if (s < score) {
      score = s;
      best = e;
    }
  }
  return best;
}

const PANEL_KEYS = { d: "deck", v: "relics", i: "info", m: "map", l: "log", h: "history" };
const PILE_KEYS = { a: "draw", s: "discard", x: "exhaust" };

function onKey(ev) {
  if (ev.metaKey || ev.ctrlKey || ev.altKey || ev.target.matches?.("input, textarea")) return;
  // Before a run only the arrows and Enter (the start screen's buttons) mean anything.
  if (!game && !["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Enter", " "].includes(ev.key)) return;
  const dirs = { ArrowUp: [0, -1], ArrowDown: [0, 1], ArrowLeft: [-1, 0], ArrowRight: [1, 0] };
  if (dirs[ev.key]) {
    ev.preventDefault();
    const items = pressables();
    const at = items.find((e) => identity(e) === kbd);
    if (!at) return focusOn(firstChoice(items));
    focusOn(nearest(at, ...dirs[ev.key], items) ?? at);
    return;
  }
  if (ev.key === "Enter" || ev.key === " ") {
    const at = pressables().find((e) => identity(e) === kbd);
    if (!at) return;
    ev.preventDefault();
    if (at instanceof SVGElement) at.dispatchEvent(new MouseEvent("click", { bubbles: true }));
    else at.click();
    return;
  }
  if (ev.key === "Escape") {
    ev.preventDefault();
    if (local || view?.sheet) closeSheet();
    else send("Back");
    return;
  }
  // A laptop's keys for the views (the menu's own letters, one press away): deck, relics, potions and
  // powers, map, log, rooms visited; and in a fight the three piles. The same key closes the view.
  const panel = view && !view.over && !local && !/menu/i.test(view.sheet_kind ?? "") ? (PANEL_KEYS[ev.key.toLowerCase()] ?? (view.fight ? PILE_KEYS[ev.key.toLowerCase()] : null)) : null;
  if (panel && !ev.repeat) {
    ev.preventDefault();
    open(panel);
    return;
  }
  if (/^[0-9]$/.test(ev.key)) {
    ev.preventDefault();
    send({ Key: { Digit: Number(ev.key) } });
    return;
  }
  if (ev.key.length === 1 && /[a-z]/i.test(ev.key)) send({ Key: { Char: ev.key.toLowerCase() } });
}

function renderTop(t) {
  const top = $("top");
  const mods = view?.run?.modifiers ?? [];
  top.replaceChildren();
  if (!t) return;
  const hpPct = t.max_hp ? Math.max(0, Math.min(100, (100 * t.hp) / t.max_hp)) : 0;
  const stats = el(
    "div",
    "stats",
    el("span", "stat hp", uiIcon("top_bar_heart", "heart"), el("b", null, `${t.hp}`), el("small", null, `/${t.max_hp}`)),
    el("span", "stat gold", uiIcon("top_bar_gold", "coin"), el("b", null, `${t.gold}`)),
    el("span", "stat floor", uiIcon("top_bar_floor", "stairs"), el("b", null, `${t.floor}`)),
    t.stars != null ? el("span", "stat stars", uiIcon("star_icon", "star"), el("b", null, `${t.stars}`)) : null,
    el("span", "grow"),
    mods.length ? tap(el("button", "iconbtn modsbtn", `✦${mods.length}`), () => showLocal(modsSheet(mods))) : null,
    tap(el("button", "iconbtn deckbtn", uiIcon("top_bar_deck", "deck")), () => open("deck")),
    tap(el("button", "iconbtn", uiIcon("top_bar_map", "map")), () => open("map")),
    tap(el("button", "iconbtn", uiIcon("top_bar_settings", "menu")), () => open("menu")),
  );
  stats.querySelector(".hp").dataset.hp = t.hp;
  const full = t.potions.filter((p) => !p.empty).length;
  const crowded = full >= 3;
  const belt = el("div", "belt");
  t.potions.forEach((p, slot) => {
    if (p.empty) {
      belt.append(el("span", "pot empty", icon("potion")));
      return;
    }
    const name = crowded ? shortPotion(p.name) : p.name;
    const word = !/\s/.test(name);
    const pic = artImg("potion", p.art, "potic");
    const b = pic
      ? el("button", `pot pic${p.usable ? " usable" : ""}${p.selected ? " sel" : ""}`, pic)
      : el("button", `pot${p.usable ? " usable" : ""}${p.selected ? " sel" : ""}${name.length > 11 ? " long" : ""}${word ? " word" : ""}${word && name.length > 8 ? " squeeze" : ""}`, el("span", null, name));
    b.setAttribute("aria-label", p.name);
    b.dataset.slot = slot;
    tap(b, () => showLocal(potionSheet(slot)));
    attachTip(b, () => ({ title: p.name, nodes: [textSheet(p.detail)] }));
    belt.append(b);
  });
  const line = el("div", "hpline", el("i"));
  line.firstChild.style.width = `${hpPct}%`;
  // The relics under the belt: every one an icon (its counter, a tap reads it), wrapped onto more
  // rows and sized down as they pile up so none is hidden; one `◆ N` chip without the art.
  const relics = t.relics ?? [];
  if (relics.length && artFiles.size) {
    const shelf = el("div", `shelf${relics.length > 18 ? " tiny" : relics.length > 9 ? " small" : ""}`);
    relics.forEach((r, i) => {
      const pic = artImg("relic", r.art, "relic-ic");
      const look = r.status === "Active" ? " active" : r.status === "Disabled" ? " used" : "";
      const chip = el("button", `relic pic${r.wax ? " wax" : ""}${look}`, pic ?? el("i", null, "◆"), r.counter != null ? el("b", null, `${r.counter}`) : null);
      chip.setAttribute("aria-label", r.name);
      attachTip(chip, () => ({ title: r.name, nodes: [textSheet(r.detail, r.counter != null ? `Counter: ${r.counter}` : null, r.wax ? "Wax: it melts once used up." : null)] }));
      shelf.append(tap(chip, () => showLocal(relicSheet(i))));
    });
    top.append(stats, el("div", "tray", belt, shelf));
  } else {
    if (relics.length) belt.append(tap(el("button", "relic fold", el("i", null, "◆"), el("b", null, `${relics.length}`)), () => showLocal(relicsSheet())));
    top.append(stats, el("div", "tray", belt));
  }
  top.append(line);
}

// A row led by a picture: its name, a count, its text; a tap (if any) reads it.
function artRow(pic, name, right, lines, onTap, cls = "") {
  const main = el("span", "main", el("span", "lab", name), ...(lines ?? []).map((d) => rich(d, "sub")));
  const kids = [el("span", "lead-pic", pic ?? el("i", null, "◆")), main, right != null && right !== "" ? el("span", "right", `${right}`) : null];
  const b = el(onTap ? "button" : "div", `row artrow${cls}`, ...kids);
  return onTap ? tap(b, onTap) : b;
}

// Every relic with its art, counter and text; a tap reads one with its glossary.
const relicsList = () => el("div", "list", (view.screen.top.relics ?? []).map((r, i) => artRow(artImg("relic", r.art, "relic-ic"), r.name, r.counter, r.detail, relicExtra(r) ? () => showLocal(relicSheet(i, true)) : null, r.status === "Disabled" ? " s-dim" : "")));
function relicsSheet() {
  return { title: "Relics", body: () => [relicsList()] };
}

// The Info sheet: the belt's potions, and in a fight each creature's powers, all with their art and text.
function infoBody(lines = []) {
  const out = [el("h3", null, "Potions")];
  const pots = (view.screen.top.potions ?? []).map((p, slot) => ({ p, slot })).filter(({ p }) => !p.empty);
  if (!pots.length) out.push(el("p", "note", "No potions."));
  else out.push(el("div", "list", pots.map(({ p, slot }) => artRow(artImg("potion", p.art, "relic-ic"), p.name, "", p.detail, () => showLocal(potionSheet(slot, true))))));
  if (view.fight) {
    // The Defect's orbs and the Regent's blades: the engine's own lines, which sit before the creatures.
    const from = lines.findIndex((l) => /^(Orbs \(|Sovereign Blade)/.test(plain(l)));
    if (from >= 0) {
      const to = lines.findIndex((l, i) => i > from && /\bHP \d+\/\d+/.test(plain(l)));
      out.push(detailLines(lines.slice(from, to < 0 ? undefined : to)));
    }
    for (const kind of ["You", "Pet", "Enemy"]) {
      for (const cr of blocksOf(view.screen, kind)) {
        if (cr.alive === false) continue;
        out.push(el("h3", null, `${cr.name} · ${Math.max(0, cr.hp)}/${cr.max_hp} HP${cr.block ? ` · ${cr.block} Block` : ""}`));
        const shown = (cr.powers ?? []).filter((p) => p.shown);
        if (!shown.length) out.push(el("p", "note", "No powers."));
        else out.push(el("div", "list", shown.map((p) => artRow(artImg("power", p.art, "relic-ic"), p.name, p.amount || "", p.detail, null, p.debuff ? " debuff" : ""))));
      }
    }
  }
  return out;
}

// Rooms visited: a picture for the room, then where and how you stood.
const HISTORY_ROOM_ART = [[/^Fight/, "map_monster"], [/^Elite/, "map_elite"], [/^Boss/, "map_elite"], [/^Shop/, "map_shop"], [/^Rest/, "map_rest"], [/^Treasure/, "map_chest"], [/^Event/, "map_unknown"]];
function historyBody(lines) {
  const out = [];
  let list = null;
  let meta = null;
  for (const l of lines) {
    const t = plain(l).trim();
    if (!t) continue;
    if (/^Act \d/.test(t)) {
      out.push(el("h3", null, t));
      list = el("div", "list");
      out.push(list);
    } else if (/^Floor \d/.test(t)) meta = l;
    else if (list) {
      const art = /^Neow|^Ancient/.test(t) ? "ancient_node_neow" : (HISTORY_ROOM_ART.find(([re]) => re.test(t)) ?? [, "map_unknown"])[1];
      const a = uiArt(art);
      list.append(artRow(a ? Object.assign(el("img", "relic-ic"), { src: a.href, alt: "" }) : icon("unknown"), t, "", meta ? [meta] : [], null));
      meta = null;
    }
  }
  return out;
}

// `shown`: opened from a row that already shows the text and counter, so only the rest.
function relicSheet(i, shown = false) {
  return {
    title: view.screen.top.relics?.[i]?.name ?? "Relic",
    body() {
      const r = view.screen.top.relics?.[i];
      if (!r) return [];
      const wax = r.wax ? "Wax: it melts once used up." : null;
      if (shown) return [wax ? el("p", "note", wax) : null, glossary(r.detail ?? [])].filter(Boolean);
      return [textSheet(r.detail, r.counter != null ? `Counter: ${r.counter}` : null, wax)];
    },
  };
}
const relicExtra = (r) => !!r.wax || !!glossary(r.detail ?? []);

// A relic's or potion's text, its notes and the game's glossary for its lit words.
function textSheet(lines, ...notes) {
  const box = el("div", "textsheet", detailLines(lines?.length ? lines : [[{ role: "Muted", bold: false, text: "No text loaded." }]]));
  for (const n of notes) if (n) box.append(el("p", "note", n));
  const g = glossary(lines ?? []);
  if (g) box.append(g);
  return box;
}

// A reward or shop row's relic or potion, read before it is taken. When `shown` (opened from
// the screen) and the screen has its whole text, the sheet has only the game's explanations of its lit
// words, and none (null) without them.
const offerLines = (b) => {
  const [kind, id] = Object.entries(b.info)[0];
  try {
    return JSON.parse(game.describe(kind.toLowerCase(), id));
  } catch {
    return [];
  }
};
function offerSheet(b, shown = null) {
  const lines = offerLines(b);
  const title = plain(b.label).replace(/^Take the relic:\s*/, "").replace(/^[^\p{L}\p{N}]+/u, "");
  if (!showsAll(shown, lines)) return { title, body: () => [textSheet(lines)] };
  const g = glossary(lines);
  return g ? { title, body: () => [g] } : null;
}
// Whether the screen (opened from it: `from`) shows every line of `lines` whole, not cut off.
function showsAll(from, lines) {
  if (!from || !lines.length) return false;
  const said = plain(lines.flat()).replace(/\s+/g, "");
  const box = [...document.querySelectorAll("#screen .sub, #screen .waretext")].filter((t) => t.getClientRects().length && t.scrollHeight <= t.clientHeight + 1);
  return said.length > 0 && box.map((t) => t.textContent).join("").replace(/\s+/g, "").includes(said);
}
// A hold or `i` that reads an offer: nothing when the offer says nothing more.
const readOffer = (b, shown) => {
  const sheet = offerSheet(b, shown);
  if (sheet) showLocal(sheet);
};

// A crowded belt's potion name: the word that tells it apart ("Potion of Doom" -> "Doom").
function shortPotion(name) {
  const short = name.replace(/\b(Potion|Flask|Bottled?|Vial|Elixir|Tincture|Draught|Brew|Touch|Essence|Jar)( of)?\b/gi, "").replace(/\s+/g, " ").trim();
  return short || name;
}

// -- the fight --

// WEB65: nothing slides mid-fight. An enemy that dies keeps its slot (the engine drops it from the
// fight; its slot stays, faded), and a card played leaves its slot empty: the hand closes up only
// when a card comes in (a draw, a new turn). Both are forgotten when the fight ends.
let fightSlots = null; // { key, enemies: Map id → last row, order: [enemy id], hand: [card id | null] }

function enemySlots(enemies) {
  const key = `${view.save.seed ?? ""}|${view.save.act ?? ""}|${view.save.floor}`;
  if (fightSlots?.key !== key) fightSlots = { key, enemies: new Map(), order: [], hand: null };
  const seen = fightSlots.enemies;
  const live = new Set(enemies.map((e) => e.id));
  for (const e of enemies) seen.set(e.id, e);
  // The engine lists the enemies in the encounter's slot order (`SortEnemiesBySlotName`), left to
  // right as the game places them: a summon goes in before the first enemy that follows it there
  // (Fogmog's Eye with Teeth, slot "illusion", left of Fogmog). Dead slots stay where they were.
  let ids = fightSlots.order.filter((id) => seen.has(id));
  enemies.forEach((e, i) => {
    if (ids.includes(e.id)) return;
    const next = enemies.slice(i + 1).find((n) => ids.includes(n.id));
    ids.splice(next ? ids.indexOf(next.id) : ids.length, 0, e.id);
  });
  // A crowded fight (summons) gives up its oldest dead slots first.
  while (ids.length > 4) {
    const dead = ids.filter((id) => !live.has(id));
    if (!dead.length) break;
    const gone = Math.min(...dead);
    seen.delete(gone);
    ids = ids.filter((id) => id !== gone);
  }
  fightSlots.order = ids;
  return ids.map((id) => (live.has(id) ? seen.get(id) : { ...seen.get(id), gone: true, alive: false, hp: 0, block: 0, target: null, preview: null, escaped: false, stolen: null }));
}

function handSlots(hand) {
  const ids = hand.map((b) => b.card?.card);
  const was = fightSlots?.hand;
  const keep = was && ids.every((id) => id != null && was.includes(id));
  const slots = keep ? was.map((id) => (ids.includes(id) ? id : null)) : ids;
  // Trailing empty slots go: nothing after them can move.
  while (slots.length && slots[slots.length - 1] === null) slots.pop();
  if (fightSlots) fightSlots.hand = slots;
  const by = new Map(hand.map((b) => [b.card?.card, b]));
  return slots.map((id) => (id == null ? null : by.get(id)));
}

function renderFight(s, main) {
  const enemies = enemySlots(blocksOf(s, "Enemy"));
  const pets = blocksOf(s, "Pet");
  const you = blocksOf(s, "You")[0];
  const incoming = blocksOf(s, "Incoming")[0];
  const lastLine = blocksOf(s, "Last")[0];
  const orbs = blocksOf(s, "Orbs");
  const hand = handSlots(blocksOf(s, "List").flatMap((l) => l.rows));
  const targeting = enemies.some((e) => e.target) || pets.some((p) => p.target);

  const board = el("div", "board");
  let aimBar = null;
  const big = { Boss: " boss", Elite: " elite" }[view.room] ?? "";
  const field = el("section", `field n${Math.min(enemies.length, 4)}${big}`);
  main.classList.toggle("crowded", enemies.length >= 4 && hand.length > 6);
  fx.begin(s, enemies);
  for (const e of enemies) field.append(enemyCard(e, targeting, enemies.length <= 2));
  board.append(field);
  // Under the enemies: what the enemies' turn takes, or, while a card is aimed, what is aimed (in
  // the incoming line's place, so the hand keeps its rows).
  // A dragged card shows no bar: the lit field says where it goes.
  if (s.title && !drag) {
    const aim = el("div", "aimbar", rich(s.title), tap(el("button", "chipbtn", "Cancel"), () => send("Back")));
    fx.afterLine(aim);
    aimBar = aim;
    if (!phone()) board.append(aim);
  }
  else if (incoming) board.append(incomingLine(incoming));
  if (you) {
    const panel = youPanel(you, pets, orbs, targeting);
    // On a phone the aim bar lies over your HP row, in no row of its own.
    if (aimBar && phone()) panel.append(aimBar);
    board.append(panel);
  }
  // Kept (hidden) while a card is aimed: the row going would move the board.
  if (lastLine) board.append(el("div", s.title ? "last quiet" : "last", rich(lastLine)));
  const grid = el("section", `hand${hand.length > 6 ? " many" : ""}`);
  grid.style.setProperty("--n", Math.max(hand.length, 1));
  hand.forEach((b, i) => {
    if (!b) return grid.append(el("div", "handgap"));
    const t = cardTile(b, { armed: targeting && stateOf(b) === "Selected", hand: true });
    t.style.setProperty("--i", i);
    grid.append(t);
  });
  main.append(board, grid);
  peekable(grid);
}

// The card under the pointer shows whole, above its neighbours. A mouse picks it by the pointer's x against the
// cards' own (unraised) places, so a raised card never hides its neighbours; a finger raises the card it presses
// while it is down. The class `peek` does the rest (style.css, desktop.css). `--strip` is the width of
// a card that its overlapped neighbour leaves showing: the name wraps to it.
// A phone's hand tile shows its whole text: the font steps down (to 10px at least) until the text
// fits under the name; text that still does not fit is cut at a whole line (hold reads it all).
function fitHand(grid) {
  if (!PHONE.matches) return;
  for (const card of grid.querySelectorAll(".card")) {
    const t = card.querySelector(".ctext");
    if (!t) continue;
    t.style.removeProperty("--fit");
    card.classList.remove("cut");
    let size = Number.parseFloat(getComputedStyle(t).fontSize);
    while (t.scrollHeight > t.clientHeight + 1 && size > 10) {
      size = Math.max(10, size - 0.5);
      t.style.setProperty("--fit", `${size}px`);
    }
    if (t.scrollHeight > t.clientHeight + 1) {
      const cs = getComputedStyle(t);
      const line = Number.parseFloat(cs.lineHeight) || size * 1.12;
      const pad = (Number.parseFloat(cs.paddingTop) || 0) + (cs.boxSizing === "border-box" ? Number.parseFloat(cs.borderBottomWidth) || 0 : 0);
      const lines = Math.max(1, Math.floor((t.clientHeight - pad) / line));
      card.style.setProperty("--lines", lines);
      card.style.setProperty("--clip", `${pad + lines * line}px`);
      card.classList.add("cut");
    }
  }
}

function peekable(grid) {
  const cards = () => [...grid.children];
  const fit = () => {
    const c = cards();
    if (c.length > 1 && grid.isConnected) grid.style.setProperty("--strip", `${c[1].getBoundingClientRect().left - c[0].getBoundingClientRect().left}px`);
    if (grid.isConnected) fitHand(grid);
  };
  requestAnimationFrame(fit);
  document.fonts?.ready.then(fit);
  new ResizeObserver(fit).observe(grid);
  const set = (t) => {
    for (const c of grid.querySelectorAll(".peek")) if (c !== t) c.classList.remove("peek");
    t?.classList.add("peek");
  };
  grid.addEventListener("pointermove", (ev) => {
    if (ev.pointerType !== "mouse" || drag) return;
    let hit = null;
    for (const c of cards()) if (c.getBoundingClientRect().left <= ev.clientX) hit = c;
    set(hit && ev.clientX <= hit.getBoundingClientRect().right && !hit.disabled ? hit : null);
  }, { passive: true });
  grid.addEventListener("pointerleave", () => set(null));
  grid.addEventListener("pointerdown", (ev) => {
    if (ev.pointerType === "mouse") return;
    const t = ev.target.closest?.(".card");
    if (!t || t.disabled) return;
    set(t);
    const off = () => requestAnimationFrame(() => set(null));
    window.addEventListener("pointerup", off, { once: true });
    window.addEventListener("pointercancel", off, { once: true });
  }, { passive: true });
}

// A creature's HP: its Block as a shield badge before the bar (the hits it takes first), the bar
// with its HP in it, and what the selected card takes, a pale part of the bar and `−6` after it.
function hpBar(hp, max, block, preview, prevHp, powers) {
  const pct = (v) => `${max ? Math.max(0, Math.min(100, (100 * v) / max)) : 0}%`;
  // Poison and Doom as `NHealthBar.RefreshForeground` draws them: Poison is the part of the HP
  // that its next turn takes (the right end of the HP, all of it when lethal); Doom fills from
  // the left, up to the HP it kills at, and when it reaches the HP left after Poison it covers all of that.
  const poison = Math.max(0, (powers ?? []).find((p) => p.name === "Poison")?.next ?? 0);
  const doom = Math.max(0, (powers ?? []).find((p) => p.name === "Doom")?.amount ?? 0);
  const poisonLethal = poison > 0 && poison >= hp;
  const doomLethal = doom > 0 && doom >= hp - poison;
  const cut = poison > 0 && !poisonLethal ? poison : 0;
  const fill = el("i", "fill");
  fill.style.width = pct(Math.max(0, (prevHp ?? hp) - cut));
  if (prevHp != null && prevHp !== hp) fill.dataset.to = pct(Math.max(0, hp - cut));
  const bar = el("div", "hpbar", fill);
  if (poisonLethal || doomLethal) fill.classList.add("hidden");
  if (poison > 0 && hp > 0) {
    const seg = el("i", "poisonfg");
    seg.style.left = poisonLethal ? "0" : pct(hp - poison);
    seg.style.width = poisonLethal ? pct(hp) : pct(poison);
    bar.append(seg);
  }
  if (doom > 0 && hp > 0 && !poisonLethal) {
    const seg = el("i", "doomfg");
    seg.style.left = "0";
    seg.style.width = doomLethal ? pct(hp - poison) : pct(doom);
    seg.style.minWidth = "12px";
    bar.append(seg);
  }
  if (poisonLethal) bar.classList.add("poison-lethal");
  else if (doomLethal) bar.classList.add("doom-lethal");
  const lost = preview && preview.hp_after < hp ? hp - Math.max(0, preview.hp_after) : 0;
  if (lost) {
    const ghost = el("i", "ghost");
    ghost.style.left = pct(Math.max(0, preview.hp_after));
    ghost.style.width = `calc(${pct(hp)} - ${pct(Math.max(0, preview.hp_after))})`;
    bar.append(ghost);
  }
  bar.append(el("span", "num", `${hp}/${max}`));
  const blockArt = uiArt("block");
  const shield = block ? el("span", `block${blockArt ? " shieldart" : ""}`, blockArt ? Object.assign(el("img", null), { src: blockArt.href, alt: "" }) : icon("shield"), el("b", null, `${block}`)) : null;
  const hit = preview?.dies ? el("span", "hit lethal", "☠") : lost ? el("span", "hit", `−${lost}`) : null;
  return el("div", `hprow${block ? " blocked" : ""}`, shield, bar, hit);
}

function hitOf(p) {
  if (!p || typeof p === "string") return null;
  return p.Certain ?? p.BeforeDraw ?? null;
}

// A thief's stolen card as a chip beside its Swipe: the card's name in its type's colour, tap to read it.
function stolenChip(c) {
  const chip = el("span", "stolen", c.name);
  const open = () => showLocal(cardSheet({ id: 0, label: [], detail: [], card: c }));
  chip.addEventListener("click", (ev) => { ev.stopPropagation(); open(); });
  return chip;
}

function powerChips(list, stolen) {
  const shown = (list ?? []).filter((p) => p.shown);
  if (!shown.length && !stolen) return null;
  return el(
    "div",
    "powers",
    stolen ? stolenChip(stolen) : null,
    shown.map((p) => {
      const pic = artImg("power", p.art, "pw-ic");
      const chip = pic
        ? el("span", `pw pic${p.debuff ? " debuff" : ""}`, pic, p.amount ? el("b", null, `${p.amount}`) : null)
        : el("span", `pw${p.debuff ? " debuff" : ""}`, p.amount ? `${p.name} ${p.amount}` : p.name);
      attachTip(chip, () => {
        const g = glossary([], [p.name]);
        return { title: p.amount ? `${p.name} ${p.amount}` : p.name, nodes: [g] };
      });
      return chip;
    }),
  );
}

function prevEnemy(id) {
  return before?.fight ? blocksOf(before.screen, "Enemy").find((e) => e.id === id) : null;
}

function enemyCard(e, targeting, roomy) {
  const aim = e.target && stateOf(e.target) !== "Disabled";
  const root = el("button", `enemy${e.escaped ? " escaped" : e.alive ? "" : " dead"}${e.gone ? " gone" : ""}${aim ? " aim" : ""}${targeting && !aim ? " off" : ""}`);
  root.dataset.id = e.id;
  if (e.gone) root.disabled = true;
  if (aim) root.dataset.tap = e.target.id;
  const was = prevEnemy(e.id);
  // A card's damage shows on the bars only while one is being aimed or dragged: the engine's
  // preview otherwise follows the first card in the hand, which nobody chose.
  const hit = targeting || drag ? hitOf(e.preview) : null;
  // An enemy that left the fight alive shows the game's own Escape intent in its place.
  const flees = e.escaped ? [{ text: "» Flees" }] : null;
  const intent = intentChips(flees ?? e.intent, flees ?? (roomy && e.intent_long?.length ? e.intent_long : e.intent), flees ? null : view.intent_tips?.[e.id]);
  if (intent) root.append(intent);
  const pic = artImg("monster", e.art, "eart");
  if (pic) {
    root.classList.add("has-art");
    root.append(pic);
  }
  root.append(el("div", "ename", e.name));
  root.append(hpBar(Math.max(0, e.hp), e.max_hp, e.block, hit, was?.hp, e.powers));
  if (aim || !targeting || hit) fx.enemyNumbers(root, e);
  root.append(powerChips(e.powers, e.stolen) ?? el("div", "powers none")); // the row is there before the first power, so it does not grow the card
  if (was && was.hp > e.hp) root.dataset.loss = was.hp - Math.max(0, e.hp);
  if (was && (e.block ?? 0) > (was.block ?? 0)) root.dataset.gain = e.block - (was.block ?? 0);
  if (aim) {
    const armed = () => document.querySelector("#screen .hand .card.armed");
    root.addEventListener("pointerenter", (ev) => ev.pointerType === "mouse" && aimText(armed(), Number(armed()?.dataset.card), e.id));
    root.addEventListener("pointerleave", (ev) => ev.pointerType === "mouse" && aimText(armed(), Number(armed()?.dataset.card), null));
  }
  tap(root, () => {
    if (!aim) return readCreature(e);
    const armed = document.querySelector("#screen .hand .card.armed");
    if (armed) flyFrom(armed, null, root);
    send({ Tap: e.target.id });
  });
  return root;
}

// An intent's kind by the glyph sts2-view starts each of its spans with (`intent_glyph`), with
// the game's icon for it (`intent/ID`, none for an attack) or the page's drawing without the art.
const INTENT_KIND = {
  "⚔": ["attack", null, "sword"], "▲": ["buff", "BUFF", "star"], "▼": ["debuff", "DEBUFF", null], "✚": ["status", "STATUS_CARD", "deck"],
  "▣": ["defend", "DEFEND", "shield"], "»": ["escape", "ESCAPE", null], "♥": ["heal", "HEAL", "heart"], "?": ["unknown", "UNKNOWN", "unknown"],
  "✦": ["summon", "SUMMON", null], z: ["sleep", "SLEEP", null], "✶": ["stun", "STUN", null],
};

// The intents as chips, one per move, each in its kind's colour and icon: an attack's number big
// (`4`, `6×3` with the hits small), the rest their words (`Weak 2`, `1 Slimed`, `Asleep`).
function intentChips(short, long, tips) {
  const spans = (short ?? []).filter((sp) => sp.text.trim());
  if (!spans.length) return null;
  const row = el("div", "intent");
  // The chips show the short move: the tip says it in words only where they say more.
  const words = (r) => plain(r ?? []).replace(/^\S+\s*/, "").replace(/\s+/g, " ").trim();
  // The game's own tip (`Aggressive: This enemy intends to Attack for 11 damage.`) when its text is loaded.
  if (tips?.length) attachTip(row, () => ({ title: tips.length === 1 ? tips[0].title : "", nodes: [...tips.map((t) => (tips.length === 1 ? el("p", null, rich(t.text)) : el("p", null, el("b", null, `${t.title}: `), rich(t.text)))), glossary(tips.map((t) => t.text))] }));
  else attachTip(row, () => ({ title: "Next move", nodes: [words(long) !== words(short) ? el("p", null, rich(long)) : null, glossary([long ?? short])] }));
  for (const sp of spans) {
    const [, glyph, words = ""] = /^(\S+)\s*(.*)$/.exec(sp.text.trim()) ?? [];
    const [kind, art, drawn] = INTENT_KIND[glyph] ?? ["unknown", "UNKNOWN", "unknown"];
    const pic = (art && artImg("intent", art, "iic")) ?? (drawn ? icon(drawn, "iic") : el("span", "iic glyph", glyph));
    const chip = el("span", `ik ik-${kind}`, pic);
    const hits = kind === "attack" && /^(\d+)(?:×(\d+))?/.exec(words);
    if (hits) chip.append(el("b", "inum", hits[1]), ...(hits[2] ? [el("small", "ihits", `×${hits[2]}`)] : []));
    else if (words && words !== "Unknown") chip.append(el("span", "iwords", words));
    row.append(chip);
  }
  return row;
}

function incomingLine(i) {
  if (!i.total && !i.lethal) return el("div", "incoming calm", icon("shield"), "No damage incoming");
  const kids = [icon("sword")];
  if (i.lethal) kids.push(el("b", null, "Lethal"), ` −${i.hp_loss} HP`);
  else if (i.hp_loss) kids.push(el("b", null, `−${i.hp_loss} HP`), el("span", "dimmer", ` → ${i.hp_after}`));
  else kids.push(el("b", null, "All blocked"));
  const parts = [`${i.total} incoming`];
  if (i.blocked) parts.push(`${i.blocked} blocked`);
  if (i.absorbed) parts.push(`${i.absorbed} to ${blocksOf(view.screen, "Pet")[0]?.name ?? "your pet"}`);
  kids.push(el("span", "why", parts.join(" · ") + (i.certain ? "" : " · may vary")));
  return el("div", `incoming${i.lethal ? " lethal" : ""}${i.hp_loss ? "" : " safe"}${i.stale ? " stale" : ""}`, kids);
}

function youPanel(y, pets, orbs, targeting) {
  const panel = el("section", "you");
  const was = before?.fight ? blocksOf(before.screen, "You")[0] : null;
  // The portrait shows only where the fight is a stage (a laptop): you facing the enemies.
  const pic = artImg("character", y.name.toUpperCase(), "yart");
  const me = el("button", "me", ...(pic ? [pic] : []), el("div", "yname", y.name), hpBar(y.hp, y.max_hp, y.block, null, was?.hp, y.powers));
  me.dataset.id = y.id;
  if (was) {
    me.dataset.loss = Math.max(0, was.hp - y.hp) || "";
    me.dataset.gain = Math.max(0, (y.block ?? 0) - (was.block ?? 0)) || "";
  }
  fx.youNumbers(me);
  if (!targeting) tap(me, () => readCreature(y));
  me.append(powerChips(y.powers) ?? el("div", "powers none"));
  const res = (y.resources ?? []).filter((r) => !(y.orb_slots && /orb slot/.test(plain(r))));
  if (res.length) me.append(el("div", "res", res.map((r) => rich(r))));
  if (y.orb_slots) me.append(orbTokens(orbs, y.orb_slots));
  panel.append(me);
  for (const p of pets) {
    const aim = !!p.target;
    const ppic = artImg("monster", p.art || p.name.toUpperCase(), "yart");
    const pet = el("button", `pet${p.alive ? "" : " dead"}${aim ? " aim" : ""}`, ...(ppic ? [ppic] : []), el("div", "yname", p.name), hpBar(p.hp, p.max_hp, p.block, null, null, p.powers));
    pet.dataset.id = p.id;
    if (aim) pet.dataset.tap = p.target.id;
    pet.append(powerChips(p.powers) ?? el("div", "powers none"));
    tap(pet, () => {
      if (!aim) return readCreature(p);
      const armed = document.querySelector("#screen .hand .card.armed");
      if (armed) flyFrom(armed, null, pet);
      send({ Tap: p.target.id });
    });
    panel.append(pet);
  }
  if (orbs.length && !y.orb_slots) panel.append(orbTokens(orbs, orbs.length));
  return panel;
}

// The Defect's orbs: one token per slot, its passive big and its evoke small; empty slots dashed.
function orbTokens(orbs, slots) {
  const row = el("div", "orbs");
  for (let i = 0; i < Math.max(slots, orbs.length); i++) {
    const o = orbs[i];
    if (!o) {
      row.append(el("span", "orb empty"));
      continue;
    }
    const t = el("span", `orb o-${o.name.toLowerCase().replace(/\W+/g, "")}`, el("b", null, `${o.passive}`), el("small", null, `${o.evoke}`));
    t.title = `${o.name}: ${o.passive} each turn, ${o.evoke} when evoked`;
    row.append(t);
  }
  return row;
}

function kindClass(kind) {
  return `k-${(kind || "card").toLowerCase()}`;
}

// A card as a tile: cost, name, kind, text. `opts.onTap` overrides the tap; `opts.price` is the
// shop's price line.
function cardTile(b, opts = {}) {
  const st = stateOf(b);
  const c = b.card;
  const cls = ["card", kindClass(c?.kind)];
  if (c?.rarity) cls.push(`rar-${c.rarity.toLowerCase()}`);
  const words = Math.max(plain(c?.effect?.length ? c.effect : b.detail?.[0]).length, plain(c?.before?.effect).length);
  if (words > 95) cls.push("longer");
  else if (words > 62) cls.push("long");
  // A deck selection's pick is `Picked` with no question; a question is `Picked` with `confirm`.
  const chosen = (st === "Picked" && !b.confirm) || /\[picked\]/.test(plain(b.label));
  if (chosen) cls.push("picked");
  else if (st === "Dim" || st === "Disabled") cls.push("unplayable");
  if (opts.hand && c?.glow && !chosen) cls.push("glow");
  if (opts.armed) cls.push("armed");
  if (opts.hand && drag && drag.card === c?.card) cls.push("lifted");
  if (st === "Picked" && b.confirm) cls.push("asking");
  const name = c?.name ?? plain(b.label);
  if (/\+\d*$/.test(name)) cls.push("upgraded");
  // A long word in the name shrinks it rather than breaking mid-word.
  const longest = Math.max(...name.split(/\s+/).map((w) => w.length));
  if (longest > 11) cls.push("xlname");
  else if (longest > 8) cls.push("lname");
  const t = el("button", cls.join(" "));
  if (c?.card) t.dataset.card = c.card;
  if (!opts.onTap) t.dataset.bid = b.id;
  t.disabled = st === "Disabled";
  const cost = (c?.cost ?? "").replace("·s", " ★");
  // The game's own card where there is room for it; a phone's hand keeps its compact tiles.
  if (c && uiArt("card_frame_attack_red") && !(opts.hand && PHONE.matches)) {
    t.classList.add("gamecard");
    t.append(...gameCard(c, name, cost));
  } else {
    const pic = artImg("card", c?.art, "cart");
    if (pic) t.classList.add("arted");
    t.append(
      el("span", "chead", cost && cost !== "—" ? costGem(c?.frame, cost) : null, el("span", "cname", name)),
      ...(pic ? [pic] : []),
      el("span", "ctext", rich(c?.effect?.length ? c.effect : b.detail?.[0])),
      el("span", "ckind", c?.kind ?? ""),
    );
  }
  // Copies of one card in a selection, the Deck sheet's badge.
  const copies = /^×(\d+)$/.exec(plain(b.right ?? []).trim())?.[1];
  if (copies) t.append(el("span", "count", `×${copies}`));
  if (opts.price) t.append(el("span", "price", rich(opts.price)));
  if (chosen) t.append(el("span", "ask", "✓ picked"));
  if (st === "Picked" && b.confirm) t.append(el("span", "ask", forTouch(plain(b.confirm))));
  if (c?.before) bothFaces(t, c);
  hold(t, () => showLocal(cardSheet(b)));
  // An upgrade's tile shows the pair already: no popover over it.
  if (!c?.before) attachPreview(t, c);
  if (opts.hand && st !== "Disabled" && st !== "Dim") draggable(t, b);
  // A picked card of a multi-pick grid is tapped again to put it back.
  tap(t, () => {
    if (opts.onTap) return opts.onTap();
    if (!opts.hand && endsPick(b)) return showLocal(pickPreview(b));
    if (opts.hand) flyFrom(t, c?.card);
    send({ Tap: b.id });
  });
  return t;
}

// WEB65: a deck selection's last pick (removal, Smith, an event's upgrade or transform) is held
// for a look first, as `NDeckCardSelectScreen` / `NDeckUpgradeSelectScreen` preview it: the cards
// picked, an upgrade as now → upgraded, then Confirm sends it and Cancel (or the backdrop) lets go.
const endsPick = (b) => view?.decision === "card-select" && view.select?.preview && !!b.card && stateOf(b) !== "Picked" && view.select.picked + 1 >= view.select.count;

function pickPreview(b) {
  const picked = rowsOf(view.screen).filter((r) => r.card && stateOf(r) === "Picked");
  const shown = (r) => {
    const c = r.card;
    if (c.before) return pairBlock({ now: { ...c, ...c.before, before: null }, up: { ...c, before: null } });
    return cardTile({ ...r, state: "Enabled" }, { onTap: () => {} });
  };
  return {
    title: plain(view.screen.title) || "Confirm",
    body() {
      const cards = el("div", `pickview n${Math.min(picked.length + 1, 3)}`, ...[...picked, b].map(shown));
      const go = tap(el("button", "primary pickgo", "Confirm"), () => {
        local = null;
        renderSheet();
        send({ Tap: b.id });
      });
      const back = tap(el("button", "chipbtn pickback", "Cancel", el("kbd", null, "Esc")), closeSheet);
      return [cards, el("div", "pickbar", back, go)];
    },
  };
}
const rowsOf = (s) => blocksOf(s, "List").flatMap((l) => l.rows);

const PHONE = matchMedia("(max-width: 599px)");
// The wide layout (desktop.css): a laptop or desktop screen, mouse and keyboard.
const DESKTOP = matchMedia("(min-width: 900px) and (orientation: landscape)");

// An upgrade selection's tile: the upgraded card's name, cost and text with the card's now in
// the same place under them, one shown at a time by the grid's toggle; the tile keeps its size.
function bothFaces(t, c) {
  const now = c.before;
  const game = t.classList.contains("gamecard");
  const swap = (sel, nowKids, cls = "") => {
    const e = t.querySelector(sel);
    if (!e) return null;
    const back = el("span", "f-now", nowKids);
    e.replaceChildren(el("span", "faces", el("span", `f-up${cls}`, ...e.childNodes), back));
    return back;
  };
  const title = swap(".g-title > span, .cname", now.name);
  if (title && game) title.style.setProperty("--outline", OUTLINE_OF[c.kind?.toLowerCase() === "quest" ? "quest" : BANNER_OF[now.rarity] ?? "common"]);
  if (!/\+\d*$/.test(now.name)) t.classList.add("fromplain");
  const cost = (now.cost ?? "").replace("·s", " ★");
  swap(".g-cost b, .cost.gem b, .cost:not(.gem)", cost, now.cost === c.cost ? "" : " upcost");
  swap(".g-desc, .ctext", rich(game && now.text?.length ? now.text : now.effect));
}

// On (the default): an upgrade selection's tiles show the cards upgraded; off: as they are.
let showUpgrades = true;
function upgradeToggle(grid) {
  const box = el("button", `switch${showUpgrades ? " on" : ""}`, el("i"));
  box.setAttribute("role", "switch");
  box.setAttribute("aria-checked", `${showUpgrades}`);
  grid.classList.toggle("now", !showUpgrades);
  box.addEventListener("click", (ev) => {
    ev.preventDefault();
    showUpgrades = !showUpgrades;
    box.classList.toggle("on", showUpgrades);
    box.setAttribute("aria-checked", `${showUpgrades}`);
    grid.classList.toggle("now", !showUpgrades);
  });
  return el("label", "uptoggle", box, el("span", null, "Upgrades"));
}

// The game's card (`scenes/cards/card.tscn`): the portrait under the frame of its type in its
// pool's colour, the portrait border, title banner and type plaque in its rarity's tint, the
// description, and the cost on the pool's energy gem. The pieces are baked by `art-import`.
const BANNER_OF = { Starter: "common", Common: "common", Uncommon: "uncommon", Rare: "rare", Ancient: "rare", Special: "event", Status: "status", Curse: "curse" };
const OUTLINE_OF = { common: "#4D4B40", uncommon: "#005C75", rare: "#6B4B00", curse: "#550B9E", quest: "#7E3E15", status: "#4F522F", event: "#1B6131" };
const ENERGY_OF = { red: "ironclad", green: "silent", blue: "defect", orange: "regent", pink: "necrobinder" };
// The energy counters' number outlines (`scenes/combat/energy_counters/<character>_energy_counter.tscn`).
const GEM_LINE = { red: "#4D1310", green: "#004F04", blue: "#163E64", orange: "#784000", pink: "#702D6F" };
const ORB_LINE = { ironclad: "#4D1310", silent: "#004F04", defect: "#163E64", regent: "#784000", necrobinder: "#702D6F" };
// A compact tile's cost: the number on its pool's energy gem, else our coin.
function costGem(frame, cost) {
  const a = uiArt(`energy_${ENERGY_OF[frame] ?? "colorless"}`);
  if (!a) return el("span", "cost", cost);
  const gem = el("span", "cost gem", Object.assign(el("img", null), { src: a.href, alt: "" }), el("b", null, cost));
  gem.style.setProperty("--gemline", GEM_LINE[frame] ?? "#333");
  return gem;
}

function gameCard(c, name, cost) {
  const kind = (c.kind || "Skill").toLowerCase();
  const frameType = ["attack", "skill", "power", "quest"].includes(kind) ? kind : "skill";
  const borderType = ["attack", "skill", "power"].includes(kind) ? kind : "skill";
  const colour = c.frame || "colorless";
  const rarity = kind === "quest" ? "quest" : BANNER_OF[c.rarity] ?? "common";
  const piece = (id, cls) => {
    const a = uiArt(id);
    return a ? Object.assign(el("img", cls), { src: a.href, alt: "", draggable: false }) : el("span", cls);
  };
  const title = el("span", "g-title", el("span", null, name));
  title.style.setProperty("--outline", /\+\d*$/.test(name) ? "#1B6131" : OUTLINE_OF[rarity]);
  const parts = [
    artImg("card", c.art, "g-art") ?? el("span", "g-art"),
    piece(`card_frame_${frameType}_${colour}`, "g-frame"),
    piece(`card_portrait_border_${borderType}_${rarity}`, "g-border"),
    piece(`card_banner_${rarity}`, "g-banner"),
    title,
    piece(`card_plaque_${rarity}`, "g-plaque"),
    el("span", "g-type", c.kind ?? ""),
    el("span", "g-desc", rich(c.text?.length ? c.text : c.effect)),
  ];
  if (cost && cost !== "—") {
    const gem = el("span", "g-cost", piece(`energy_${ENERGY_OF[colour] ?? "colorless"}`, "g-gem"), el("b", null, cost));
    gem.style.setProperty("--gemline", GEM_LINE[colour] ?? "#333");
    parts.push(gem);
  }
  return parts;
}

// Press and hold: `fn` after 420 ms, and the click that ends the press is swallowed.
function hold(e, fn) {
  let timer = 0;
  let held = false;
  const stop = () => clearTimeout(timer);
  e.addEventListener("pointerdown", (ev) => {
    held = false;
    const x = ev.clientX;
    const y = ev.clientY;
    timer = setTimeout(() => {
      if (drag || !e.isConnected) return;
      held = true;
      buzz(12);
      fn();
    }, 420);
    const move = (m) => {
      if (Math.hypot(m.clientX - x, m.clientY - y) > 10) stop();
    };
    e.addEventListener("pointermove", move, { passive: true });
    e.addEventListener("pointerup", () => e.removeEventListener("pointermove", move), { once: true });
  });
  for (const ev of ["pointerup", "pointercancel", "pointerleave"]) e.addEventListener(ev, stop);
  e.addEventListener("contextmenu", (ev) => ev.preventDefault());
  e.addEventListener("click", (ev) => {
    if (held) {
      ev.stopImmediatePropagation();
      held = false;
    }
  }, true);
  return e;
}

// Drag to play: lift a hand card and drop it on a lit enemy (a card that needs a target) or
// anywhere above the hand (one that does not); a drop back on the hand puts it back. A tap still
// plays as before.
let drag = null;
let dragClick = false;

function draggable(t, b) {
  t.addEventListener("pointerdown", (ev) => {
    if (drag || ev.button > 0 || !view?.fight || view.screen.sheet) return;
    const x0 = ev.clientX;
    const y0 = ev.clientY;
    const move = (m) => {
      if (!drag) {
        if (Math.hypot(m.clientX - x0, m.clientY - y0) < 14) return;
        // Lifting redraws the hand and removes the pressed element. iOS may keep sending the touch's
        // pointer events to that removed element, where they never reach window, so hold them on the root.
        try {
          document.documentElement.setPointerCapture(m.pointerId);
        } catch {} // a pointer the browser no longer tracks: the window listeners still apply
        if (!liftCard(t, b, x0, y0)) return stop();
      }
      dragTo(m.clientX, m.clientY);
    };
    const up = (u) => {
      stop();
      if (drag) dropCard(u.clientX, u.clientY);
    };
    const cancel = () => {
      stop();
      if (drag) dropCard(-1, Infinity);
    };
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("pointercancel", cancel);
    };
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerup", up);
    window.addEventListener("pointercancel", cancel);
  });
}

// A card's numbers against the enemy being aimed at (`Web.card_face`, as the game shows a card with
// a target under it); `enemy` null: no target, the card as the hand shows it. Rewrites the text of
// the tile in place, so nothing moves.
function aimText(tile, card, enemy) {
  if (!tile || card == null || !game?.card_face) return;
  let face;
  try {
    face = JSON.parse(game.card_face(card, enemy == null ? -1 : Number(enemy)));
  } catch {
    return;
  }
  const slot = tile.querySelector(".g-desc, .ctext");
  if (!face || !slot) return;
  const holder = el("span", null, rich(slot.classList.contains("g-desc") && face.text?.length ? face.text : face.effect?.length ? face.effect : face.text));
  slot.replaceChildren(...holder.childNodes);
}

// The one living enemy a card that needs a target is thrown at on its own.
function soleEnemy() {
  const live = blocksOf(view.screen, "Enemy").filter((e) => e.alive);
  return live.length === 1 ? live[0].id : null;
}

function liftCard(t, b, x0, y0) {
  if (local) return false;
  const rect = t.getBoundingClientRect();
  const ghost = t.cloneNode(true);
  ghost.classList.remove("armed", "lifted", "peek");
  ghost.classList.add("dragcard");
  Object.assign(ghost.style, { left: `${rect.left}px`, top: `${rect.top}px`, width: `${rect.width}px`, height: `${rect.height}px` });
  // The card rides above the thumb, so neither hides what it is dropped on.
  drag = { ghost, card: b.card?.card, ox: rect.width / 2, oy: rect.height + 18, x0: rect.left, y0: rect.top, rect };
  send({ Arm: b.id });
  if (view.screen.status?.length || !view.screen.title) {
    drag = null;
    render();
    return false;
  }
  drag.aimed = [...document.querySelectorAll("#screen [data-tap]")];
  document.body.append(ghost);
  if (b.card?.aims && !drag.aimed.length) aimText(ghost, drag.card, soleEnemy());
  document.body.classList.add("dragging");
  buzz(8);
  return true;
}

// Where a drop at (x, y) lands: an aimed creature, the field, or back in the hand.
function dropAt(x, y) {
  const hand = document.querySelector("#screen .hand")?.getBoundingClientRect();
  const above = y < (hand?.top ?? 0) - 6;
  if (!drag.aimed.length) return above ? "field" : null;
  const hit = document.elementFromPoint(x, y)?.closest("[data-tap]");
  if (hit) return hit;
  // One target: anywhere above the hand aims it.
  return above && drag.aimed.length === 1 ? drag.aimed[0] : null;
}

function dragTo(x, y) {
  const g = drag.ghost;
  const at = dropAt(x, y);
  // Over a target the card shrinks toward the thumb.
  g.style.transformOrigin = "50% 100%";
  g.style.transform = `translate(${x - drag.ox - drag.x0}px, ${y - drag.oy - drag.y0}px) rotate(${Math.max(-6, Math.min(6, (x - drag.ox - drag.x0) / 30))}deg) scale(${at instanceof Element ? 0.62 : 1.04})`;
  for (const e of document.querySelectorAll("#screen .drop")) if (e !== at) e.classList.remove("drop");
  if (at instanceof Element) {
    if (!at.classList.contains("drop")) buzz(4); // the target locks
    at.classList.add("drop");
  }
  document.body.classList.toggle("dropfield", at === "field");
  fx.hover(at instanceof Element ? at.dataset.id : null);
  g.classList.toggle("cancel", !at);
  // Over an enemy the card's numbers read against it; off it, as in the hand.
  const key = at instanceof Element ? at.dataset.id : null;
  if (drag.aimed.length && key !== drag.shownFor) {
    drag.shownFor = key;
    aimText(g, drag.card, key);
  }
}

function dropCard(x, y) {
  const d = drag;
  const at = dropAt(x, y);
  drag = null;
  dragClick = true;
  setTimeout(() => (dragClick = false), 0);
  document.body.classList.remove("dragging", "dropfield");
  const g = d.ghost;
  if (!at) {
    g.classList.add("back");
    g.style.transform = "none";
    setTimeout(() => g.remove(), 200);
    send("Back");
    return;
  }
  const to = at === "field" ? null : at.getBoundingClientRect();
  pendingFly = { rect: g.getBoundingClientRect(), ghost: g.cloneNode(true), cardId: d.card, to };
  pendingFly.ghost.style.transform = "";
  pendingFly.ghost.classList.remove("dragcard", "cancel");
  g.remove();
  if (reduced() || document.body.classList.contains("proof")) pendingFly = null;
  send(at === "field" ? "Drop" : { Tap: Number(at.dataset.tap) });
}

// The click a drag's release fires is not a tap.
window.addEventListener("click", (ev) => {
  if (dragClick) {
    ev.stopPropagation();
    ev.preventDefault();
  }
}, true);

// -- the rooms --

// Lines that name a key (`Esc back to the rewards`): a phone gets a Back chip instead.
const KEY_HINT = /^(Esc|Enter|Press|Space)\b/;

// The room offers a way back (its text names Esc): a phone docks a Back button, a laptop shows a chip.
const escHinted = (s) => s.blocks.some((b) => b.Text?.some((l) => /^Esc\b|\bEsc cancels\b/.test(plain(l).trim())));

function renderRoom(s, main) {
  if (view.decision === "shop" && !s.blocks.some((b) => b.List?.rows.some(sectionRow))) return renderShop(s, main);
  const hinted = escHinted(s) && !phone();
  // The room's own picture (an event's, an ancient's, the treasure's chest) in a box of fixed
  // size, so nothing moves when it loads. Without the art there is no box: the page is as before.
  const picture = view.picture && artImg("event", view.picture, "picimg");
  const chest = view.picture === "CHEST";
  // A narrow screen's event: the picture is the background of the title and story (a card with art behind its text).
  const card = picture && !chest && !DESKTOP.matches && s.title && !s.aside && !hinted;
  // The chest on a phone keeps the title on the picture's foot (the game's own framing).
  const onPicture = picture && chest && phone() && s.title && !s.aside && !hinted;
  let host = main;
  if (card) {
    host = el("div", "picstory", picture, el("div", "picgap"), el("div", "pictext", el("h1", null, rich(inGameWords(s.title)))));
    main.append(host);
    host = host.lastChild;
  } else if (picture) main.append(el("div", `picture${chest ? " chest" : ""}`, picture, onPicture ? el("h1", "pictitle", rich(inGameWords(s.title))) : null));
  if (!onPicture && !card && (s.title || s.aside || hinted)) {
    const over = view.over;
    main.append(
      el(
        "header",
        `head${over ? ` end ${view.decision}` : ""}`,
        el("h1", null, rich(inGameWords(s.title))),
        s.aside && view.decision !== "shop" ? el("span", "aside", rich(s.aside)) : null,
        hinted ? tap(el("button", "chipbtn", icon("back"), "Back", el("kbd", null, "Esc")), () => send("Back")) : null,
      ),
    );
  }
  // A room with no story (rest, treasure, shop) gets its sign in the space over the choices.
  const art = ROOM_ART[view.decision];
  const cardRows = s.blocks.some((b) => b.List?.rows.some((r) => r.card));
  // With the game's art the rows carry the pictures; our sign stands in only without it.
  if (art && !cardRows && !uiArt("map_rest") && !s.blocks.some((b) => b.Text?.some((l) => plain(l).trim()))) main.append(el("div", `roomart ${art}`, icon(art)));
  let sign = cardRows ? null : STORY_ART[view.decision];
  for (const b of s.blocks) {
    if (b === "Rule") {
      main.append(el("hr"));
      continue;
    }
    let [kind, v] = Object.entries(b)[0];
    if (kind === "Text") {
      // WEB65: a card selection is its own screen (`NDeckCardSelectScreen`): the event's story
      // that led to it stays behind; only the prompt, its last paragraph, shows over the grid.
      if (view.decision === "card-select") {
        const blank = v.findLastIndex((l) => !plain(l).trim());
        v = v.slice(blank + 1);
      }
      // The Back button (the dock's on a phone, the header's chip with its Esc key on a laptop) says it once.
      const lines = v.map((l) => l.map((sp) => ({ ...sp, text: sp.text.replace(" · Esc cancels", "") }))).filter((l) => !KEY_HINT.test(plain(l).trim()) && !(artFiles.size && OUR_LINES.has(plain(l).trim())) && ![plain(s.title ?? []), plain(inGameWords(s.title ?? []))].some((t) => t.trim() && t.trim() === plain(l).trim()));
      if (!lines.some((l) => plain(l).trim())) continue;
      // On a phone the map fills the screen: a leftover quote under it would cover the rooms to tap.
      if (view.decision === "map" && phone()) continue;
      host.append(story(lines));
      if (sign) {
        host.append(el("div", `roomart small ${sign}`, icon(sign)));
        sign = null;
      }
    }
    else if (kind === "Map") {
      const head = main.querySelector(".head");
      if (head && v.act && !s.aside) head.append(el("span", "aside", `Act ${v.act}`));
      // The top bar's map icon and the map say "Map": the heading goes, the header keeps its height.
      head?.querySelector("h1")?.classList.add("quiet");
      main.append(mapView(v, false));
    }
    else if (kind === "List" && view.decision !== "map") {
      const rows = withRelics(v.rows);
      const extras = roomExtras(rows);
      // A reward whose cards are drawn as tiles has no row of its own: the tiles are the one way to take a card.
      const shown = view.decision === "rewards" ? rows.filter((r) => !extras.drawn?.has(r.id)) : rows;
      if (view.decision === "rewards") {
        if (shown.length) main.append(list(shown));
        main.append(...extras);
      } else {
        main.append(...extras);
        // WEB67: the chest's gold is gained as it opens (`DoTreasureRoomRewards`, a burst of
        // coins), not offered: it shows as what the chest gave, above the relic to take.
        // On the chest's picture (a box of fixed size) where there is one, so nothing shifts.
        if (view.decision === "treasure" && view.chest_gold) {
          const gold = el("p", "chestgold", uiIcon("top_bar_gold", "coin"), el("b", null, `+${view.chest_gold}`), " Gold");
          const pic = main.querySelector(".picture.chest");
          if (pic) pic.append(gold);
          else main.append(gold);
        }
        main.append(list(rows));
      }
    }
  }
  // A laptop's deck pick: Back, the title, the prompt and the Upgrades switch on one header bar over the grid.
  const head = DESKTOP.matches && view.decision === "card-select" && main.querySelector(":scope > .head");
  if (head) head.append(...main.querySelectorAll(":scope > .story, :scope > .prompt, :scope > .list > .uptoggle"));
}

// What fills a room's space over its choices (WEB30): a card reward's cards as tiles (a tap
// takes one), a rest site's HP before and after resting, a treasure's or ancient's relics large.
// A treasure's or ancient's rows with the relic each offers (`Web.room_relics`), read like a
// reward's: its picture, its `i`.
function withRelics(rows) {
  if (view.decision !== "treasure" && view.decision !== "ancient") return rows;
  const relics = new Map(JSON.parse(game?.room_relics?.() ?? "[]").map((r) => [r.id, r.relic]));
  return rows.map((r) => (relics.has(r.id) && !r.info ? { ...r, info: { Relic: relics.get(r.id) } } : r));
}

function roomExtras(rows) {
  const d = view.decision;
  if (d === "rewards") {
    const out = [];
    out.drawn = new Set();
    for (const r of rows) {
      if (r.state === "Disabled" || !/^Card reward/.test(plain(r.label).trim())) continue;
      const sec = rewardCards(r);
      if (sec) {
        out.push(sec);
        out.drawn.add(r.id);
      }
    }
    return out;
  }
  return [];
}

// A card reward's cards, from what opening it would show (`Web.peek`): a tile's tap opens the
// reward and takes that card, as two taps on the page would.
function rewardCards(r) {
  const peeked = JSON.parse(game?.peek?.(JSON.stringify({ Tap: r.id })) ?? "null");
  const rowsOf = (v) => v.screen.blocks.flatMap((b) => b.List?.rows ?? []);
  const cards = peeked ? rowsOf(peeked).filter((c) => c.card) : [];
  if (!cards.length) return null;
  const grid = el("div", "cards peek");
  // A reward's cards all have card id 0, so the tapped one is known only by its place.
  cards.forEach((c, i) =>
    grid.append(cardTile(c, {
      onTap: () => {
        send({ Tap: r.id });
        const now = rowsOf(view).filter((x) => x.card)[i];
        if (now && stateOf(now) !== "Disabled") send({ Tap: now.id });
      },
    })));
  // What opening the reward offers besides cards (a reroll, a relic's trade) stays one tap away; Skip is Proceed's.
  const others = rowsOf(peeked).filter((c) => !c.card && c.state !== "Disabled" && !/^Skip\b/.test(plain(c.label).trim()));
  const alts = others.map((o) => {
    const b = el("button", "row peekalt", el("span", "main", rich(o.label, "lab")));
    tap(b, () => {
      send({ Tap: r.id });
      const now = rowsOf(view).find((x) => !x.card && plain(x.label) === plain(o.label));
      if (now && stateOf(now) !== "Disabled") send({ Tap: now.id });
    });
    return b;
  });
  const extra = plain(r.label).match(/ \(.+\)$/)?.[0] ?? "";
  return el("section", "peekreward", el("p", "prompt", `${gameLines.addCard ?? "Add a card to your deck"}${extra}`), grid, ...alts);
}

// The rest site: your HP now and after resting, the heal lit on the bar.
// -- the shop: its whole stock on one screen (WEB30) --

// A first-level section row of the two-level shop (a client that did not ask for the flat one).
const sectionRow = (r) => !r.card && !r.info && /^\d+ (cards?|relics?|potions?)$/.test(plain(r.right).trim());
// The shop's items by id, as last drawn: a row asking "Tap again" comes without its card or
// relic, so it is drawn as the item it was.
let shopSeen = new Map();

function renderShop(s, main) {
  main.append(el("header", "head", el("h1", null, rich(inGameWords(s.title)))));
  for (const lines of blocksOf(s, "Text")) {
    const said = lines.filter((l) => plain(l).trim());
    if (said.length) main.append(story(said));
  }
  const seen = shopSeen;
  shopSeen = new Map();
  const groups = { cards: [], colorless: [], relics: [], potions: [], rest: [] };
  for (let r of blocksOf(s, "List").flatMap((l) => l.rows)) {
    const asking = stateOf(r) === "Picked" && r.confirm;
    if (asking && !r.card && !r.info && seen.has(r.id)) r = { ...seen.get(r.id), state: "Picked", confirm: r.confirm };
    shopSeen.set(r.id, r);
    const g = r.card ? (r.card.frame === "colorless" ? "colorless" : "cards") : r.info?.Relic ? "relics" : r.info?.Potion ? "potions" : "rest";
    groups[g].push(r);
  }
  const section = (cls, title, kids) => kids.length && main.append(el("section", `shopsec ${cls}`, el("h2", null, title), ...kids));
  const tiles = (rows) => {
    if (!rows.length) return [];
    const grid = el("div", "cards");
    for (const r of rows) {
      const t = cardTile(r, { price: r.right });
      if (/SALE/.test(plain(r.right))) t.classList.add("sale");
      grid.append(t);
    }
    return [grid];
  };
  const wares = (rows, kind) => (rows.length ? [el("div", "wares", ...rows.map((r) => ware(r, kind)))] : []);
  section("s-cards", "Cards", tiles(groups.cards));
  section("s-colorless", "Colorless", tiles(groups.colorless));
  section("s-relics", "Relics", wares(groups.relics, "relic"));
  section("s-potions", "Potions", wares(groups.potions, "potion"));
  if (groups.rest.length) main.append(el("div", "list shopmore", ...groups.rest.map(row)));
}

// A shop relic or potion: its picture, name, text and price; a tap buys (asking first), a hold
// or its `i` reads it.
function ware(r, kind) {
  const st = stateOf(r);
  const asking = st === "Picked" && r.confirm;
  const e = el("button", `ware w-${kind}${st === "Dim" ? " unaffordable" : ""}${asking ? " asking" : ""}`);
  e.disabled = st === "Disabled";
  e.dataset.bid = r.id;
  const id = r.info[kind === "relic" ? "Relic" : "Potion"];
  const pic = artImg(kind, id, "wareart") ?? el("span", "wareart", icon(kind));
  let lines = [];
  try {
    lines = JSON.parse(game.describe(kind, id));
  } catch {
    lines = [];
  }
  const name = plain(r.label).replace(/^[^\p{L}\p{N}]+/u, "");
  e.append(
    pic,
    el("span", "warebody", el("b", "warename", name), lines.length ? el("span", "waretext", ...lines.map((l) => rich(l))) : null),
    asking ? el("span", "ask", forTouch(plain(r.confirm))) : r.right ? rich(r.right, "price") : null,
  );
  // Its text is on the ware (a desktop shows it in the hover tip): the `i` only for the glossary.
  if (glossary(lines)) {
    const info = el("span", "info", "i");
    info.setAttribute("role", "button");
    info.setAttribute("aria-label", "Read");
    tap(info, () => readOffer(r, e));
    e.append(info);
  }
  hold(e, () => readOffer(r, e));
  attachTip(e, () => ({ title: name, nodes: [textSheet(lines)] }));
  tap(e, () => send({ Tap: r.id }));
  return e;
}

function story(lines) {
  // A one-line prompt (`Pick one for your deck, or skip.`) is a quiet line, not a story's box.
  const said = lines.filter((l) => plain(l).trim());
  if (said.length === 1 && plain(said[0]).length <= 70) return el("p", "prompt", rich(said[0]));
  const box = el("div", "story");
  let p = null;
  for (const line of lines) {
    if (!plain(line).trim()) {
      p = null;
      continue;
    }
    if (!p) {
      p = el("p");
      box.append(p);
    } else p.append(el("br"));
    p.append(rich(line));
  }
  return box;
}

function list(rows) {
  const out = el("div", "list");
  let grid = null;
  // The run's end: Quit and Character select both go to the start screen; keep one.
  if (view.over && rows.some((r) => /^Character select/.test(plain(r.label))))
    rows = rows.filter((r) => !/^Quit\b/.test(plain(r.label)));
  for (const [i, r] of rows.entries()) {
    if (r.card) {
      if (!grid) {
        grid = el("div", "cards");
        if (r.card.before) out.append(upgradeToggle(grid));
        out.append(grid);
      }
      grid.append(cardTile(r, { price: view.decision === "shop" ? r.right : null }));
    } else {
      grid = null;
      const e = row(r);
      if (view.over && i < 2) e.classList.add("lead");
      out.append(e);
    }
  }
  return out;
}

function row(b) {
  const st = stateOf(b);
  const e = el("button", `row s-${st.toLowerCase()}`);
  e.disabled = st === "Disabled";
  const main = el("span", "main");
  const label = plain(b.label);
  const cues = view?.decision === "rewards" ? REWARD_ICONS : view?.decision === "rest" ? REST_ICONS : null;
  // A relic or potion row is known by what it offers, not by its name ("◆ Old Coin" has no "relic").
  const offer = b.info?.Relic ? [null, "relic"] : b.info?.Potion ? [null, "potion"] : null;
  const shown = view?.decision === "treasure" || view?.decision === "ancient";
  const cue = st === "Disabled" ? null : cues ? (view?.decision === "rewards" && offer) || cues.find(([re]) => re.test(label)) : shown ? offer : null;
  if (cue) e.append(rowSign(b, cue[1]));
  // With an icon of the page's own, the label's leading glyph (`◉`, `◈`) goes.
  let lead = !!cue;
  const spans = b.label.map((sp) => {
    if (!lead) return sp;
    const text = sp.text.replace(/^[^\p{L}\p{N}]+/u, "");
    lead = !text;
    return { ...sp, text };
  });
  const say = (text) => [{ role: "Plain", bold: false, text }];
  const gold = cue?.[1] === "coin" && label.match(/(\d+) gold/i);
  const words =
    cue?.[1] === "deck" && /^Card reward/.test(label)
      ? say(`${gameLines.addCard ?? "Add a card to your deck"}${label.match(/ \(.+\)$/)?.[0] ?? ""}`)
      : gold && gameLines.gold
        ? say(gameLines.gold.replace("{gold}", gold[1]))
        : spans;
  main.append(st === "Picked" && b.confirm ? el("span", "lab ask", forTouch(label)) : rich(words, "lab"));
  for (const d of b.detail ?? []) if (plain(d).trim()) main.append(rich(d, "sub"));
  // A reward's relic or potion says what it does, not only its name.
  if ((view?.decision === "rewards" || view?.decision === "treasure") && offer && !(b.detail ?? []).some((d) => plain(d).trim())) {
    const [kind, id] = Object.entries(b.info)[0];
    const lines = JSON.parse(game?.describe(kind.toLowerCase(), id) ?? "[]");
    for (const l of lines) if (plain(l).trim()) main.append(rich(l, "sub"));
  }
  const why = dimReason(b);
  if (why) main.append(el("span", "sub why", why));
  e.append(main);
  if (b.right && !(view?.decision === "shop" && /^\d+ \w+$/.test(plain(b.right).trim()))) e.append(rich(b.right, "right"));
  if (cue?.[2]) e.append(el("span", "chev", "›"));
  // The row's own text said, its `i` and hold read only what it does not say (the glossary).
  if (b.info && (b.info.Relic || b.info.Potion)) {
    const lines = offerLines(b);
    const said = plain(lines.flat()).replace(/\s+/g, "");
    const all = said && [...main.querySelectorAll(".sub")].map((t) => t.textContent).join("").replace(/\s+/g, "").includes(said);
    if (!all || glossary(lines)) {
      const info = el("span", "info", "i");
      info.setAttribute("role", "button");
      info.setAttribute("aria-label", "Read");
      tap(info, () => readOffer(b, e));
      e.append(info);
    }
    hold(e, () => readOffer(b, e));
  }
  e.dataset.bid = b.id;
  tap(e, () => send({ Tap: b.id }));
  return e;
}

// A reward row's icon, and whether it opens a choice.
const REWARD_ICONS = [
  [/^\W*\d+ gold/i, "coin"],
  [/^Card reward|cards?$/i, "deck", true],
  [/potion/i, "potion"],
  [/relic/i, "relic"],
];

// A reward or rest row's sign: the game's reward icon, the potion's or relic's own picture,
// the rest option's painting; else our icon.
const SIGN_ART = { coin: "reward_icon_money", deck: "reward_icon_card" };
const REST_ART = { Rest: "option_heal", Smith: "option_smith", Lift: "option_lift", Toke: "option_toke", Dig: "option_dig", Mend: "option_mend", Cook: "option_cook", Hatch: "option_hatch", Clone: "option_clone" };
function rowSign(b, name) {
  const own = (b.info?.Potion && artImg("potion", b.info.Potion, "signart")) || (b.info?.Relic && artImg("relic", b.info.Relic, "signart"));
  if (own) return el("span", `lead-ic art ${name}`, own);
  const word = plain(b.label).trim().split(/\s/)[0];
  const id = view?.decision === "rest" ? REST_ART[word] : SIGN_ART[name];
  const a = id && uiArt(id);
  if (a) return el("span", `lead-ic art ${view?.decision === "rest" ? "painting" : name}`, Object.assign(el("img", "signart"), { src: a.href, alt: "" }));
  return el("span", `lead-ic ${name}`, icon(name));
}

const ROOM_ART = { rest: "rest", treasure: "chest", shop: "shop" };
// Rooms with a story still get a sign under it: Neow's eye.
const STORY_ART = {};

const REST_ICONS = [
  [/^Rest\b/, "rest"],
  [/^Smith\b/, "hammer"],
  [/^(Lift|Toke|Dig|Recall|Mend|Cook|Hatch|Clone)\b/, "star"],
];

// -- the map --

const MAP_ROW = 74;
const MAP_PAD = 46;
// The paper is narrower than its picture: nodes keep to its inner 76% (the phone widens the page).
const MAP_SIDE = 44;

function jitter(x, y) {
  const h = Math.sin(x * 12.9898 + y * 78.233) * 43758.5453;
  return (h - Math.floor(h) - 0.5) * 18;
}

// The game's map pieces (`art-import`): each room kind's icon (its `_outline` silhouette lights
// it), the ink circles round visited rooms, the character's marker, the boss's icon, the act's
// page.
const MAP_ART = { sword: "map_monster", skull: "map_elite", unknown: "map_unknown", shop: "map_shop", chest: "map_chest", rest: "map_rest", ancient: "ancient_node_neow" };
const CHARACTER_IDS = ["IRONCLAD", "SILENT", "DEFECT", "REGENT", "NECROBINDER"];
const uiArt = (id) => {
  const file = artFiles.get(`ui/${id}`);
  return file ? { href: `${ART}${file}`, size: artSizes.get(`ui/${id}`) } : null;
};
// The game's piece `id` as an <img>, else our icon `fallback`.
function uiIcon(id, fallback) {
  const a = uiArt(id);
  return a ? Object.assign(el("img", "uiic"), { src: a.href, alt: "", draggable: false }) : icon(fallback);
}
// An SVG <image> of the piece `id` centred on (0, 0), its longest side `px`.
function mapImage(id, px, cls) {
  const a = uiArt(id);
  if (!a) return "";
  const [w, h] = a.size;
  const k = px / Math.max(w, h);
  return `<image class="${cls}" href="${a.href}" x="${(-w * k) / 2}" y="${(-h * k) / 2}" width="${w * k}" height="${h * k}"/>`;
}

function mapView(g, inSheet) {
  const W = 360;
  const cols = Math.max(g.width, 1);
  const H = (g.height + 1) * MAP_ROW + MAP_PAD * 2;
  // A room beside another in its row keeps to its column (no jitter), so neighbours never draw
  // closer than a column apart; each room's icon and tap circle fit the gap to its nearest
  // neighbour, so no two overlap or share a tap area.
  const colW = (W - 2 * MAP_SIDE) / cols;
  const taken = new Set(g.cells.map((c) => `${c.x},${c.y}`));
  const crowded = (x, y) => taken.has(`${x - 1},${y}`) || taken.has(`${x + 1},${y}`);
  const px = (x, y) => (y >= g.boss_at[1] ? W / 2 : MAP_SIDE + (x + 0.5) * colW + (crowded(x, y) ? 0 : jitter(x, y)));
  const room = (c) => {
    const x = px(c.x, c.y);
    let d = Infinity;
    for (const o of g.cells) if (o.y === c.y && o !== c) d = Math.min(d, Math.abs(px(o.x, o.y) - x));
    return d;
  };
  const py = (y) => H - MAP_PAD - y * MAP_ROW;
  const cell = new Map(g.cells.map((c) => [`${c.x},${c.y}`, c]));
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
  svg.setAttribute("class", "mapsvg");
  let paths = "";
  for (const e of g.edges) {
    const a = cell.get(`${e.from[0]},${e.from[1]}`);
    const b = cell.get(`${e.to[0]},${e.to[1]}`);
    const fromYou = g.you && e.from[0] === g.you[0] && e.from[1] === g.you[1];
    const walked = a?.visited && b?.visited;
    const next = (fromYou || (!g.you && !a)) && b?.reachable;
    const ahead = (a?.ahead || fromYou || !g.you) && (b ? b.ahead || b.reachable : true);
    const cls = walked ? "walked" : next ? "next" : ahead ? "ahead" : "faded";
    paths += `<line class="${cls}" data-e="${e.from[0]},${e.from[1]} ${e.to[0]},${e.to[1]}" x1="${px(e.from[0], e.from[1])}" y1="${py(e.from[1])}" x2="${px(e.to[0], e.to[1])}" y2="${py(e.to[1])}"/>`;
  }
  const art = !!uiArt("map_monster");
  let nodes = "";
  for (const c of g.cells) {
    const name = GLYPH_ICON[c.glyph] ?? "unknown";
    const you = g.you && g.you[0] === c.x && g.you[1] === c.y;
    const cls = ["node", `g-${c.role}`, `kind-${name}`, c.visited ? "visited" : "", c.ahead ? "ahead" : "", c.reachable ? "reach" : "", you ? "you" : ""].join(" ");
    const x = px(c.x, c.y);
    const y = py(c.y);
    const tag = you ? `<g class="youtag" transform="translate(${x < W / 2 ? 30 : -30} 0)"><rect x="${x < W / 2 ? 0 : -40}" y="-11" width="40" height="22" rx="11"/><text x="${x < W / 2 ? 20 : -20}" y="4.5" text-anchor="middle">YOU</text></g>` : "";
    const gap = room(c);
    const hit = c.reachable ? `<circle class="hit" r="${Math.min(30, gap / 2)}"/>` : c.through?.length ? `<circle class="hit" r="${Math.min(19, gap / 2)}"/>` : "";
    if (art) {
      const id = MAP_ART[name] ?? "map_unknown";
      const px = Math.min(name === "ancient" ? 72 : 62, gap - 4);
      // A visited room is ringed in ink, as the game marks it; the ring varies by room.
      const ring = c.visited && !you ? mapImage(`map_circle_${(c.x * 7 + c.y * 3) % 5}`, px * 0.8, "done") : "";
      const marker = you ? `<g class="marker" transform="translate(0 -30)">${mapImage(`map_marker_${CHARACTER_IDS[view?.save?.character]?.toLowerCase()}`, 34, "mk")}</g>` : "";
      nodes += `<g class="${cls} art" data-at="${c.x},${c.y}" data-button="${c.button ?? ""}" transform="translate(${x} ${y})">${hit}${mapImage(`${id}_outline`, px, "glow")}${mapImage(id, px, "mi")}${ring}${marker}</g>`;
      continue;
    }
    nodes += `<g class="${cls}" data-at="${c.x},${c.y}" data-button="${c.button ?? ""}" transform="translate(${x} ${y})">${hit}<circle r="${Math.min(c.reachable ? 21 : you ? 20 : 17, gap / 2 - 1)}"/>${you ? '<circle class="ring" r="26"/>' : ""}<svg x="-11" y="-11" width="22" height="22" viewBox="0 0 24 24">${iconSvg(name)}</svg>${tag}</g>`;
  }
  const bx = W / 2;
  const by = py(g.boss_at[1]);
  const bossIcon = art && mapImage(`${g.boss_art.toLowerCase()}_icon`, 150, "mi");
  if (bossIcon) {
    // The boss's own icon, its name under it.
    nodes += `<g class="node boss art${g.boss_button ? " go" : ""}"${g.boss_button ? ` data-boss="${g.boss_button}"` : ""} transform="translate(${bx} ${by - 44})">${bossIcon}</g>`;
  } else {
    const act = g.act ? `ACT ${g.act} BOSS` : "BOSS";
    nodes += `<g class="node boss${g.boss_button ? " go" : ""}"${g.boss_button ? ` data-boss="${g.boss_button}"` : ""} transform="translate(${bx} ${by})"><rect x="-92" y="-34" width="184" height="68" rx="14"/><svg x="-80" y="-14" width="28" height="28" viewBox="0 0 24 24">${iconSvg("boss")}</svg><text class="bosstag" x="-44" y="-6">${act}</text><text class="bossname" x="-44" y="16">${escapeHtml(g.boss)}</text></g>`;
  }
  // Tints for the white silhouettes: teal where you can go, gold for a kind lit from the legend.
  const tints = `<defs><filter id="tint-go"><feColorMatrix type="matrix" values="0 0 0 0 .35  0 0 0 0 .85  0 0 0 0 .9  0 0 0 1 0"/></filter><filter id="tint-lit"><feColorMatrix type="matrix" values="0 0 0 0 .96  0 0 0 0 .76  0 0 0 0 .3  0 0 0 1 0"/></filter><filter id="tint-ink"><feColorMatrix type="matrix" values="0 0 0 0 .17  0 0 0 0 .11  0 0 0 0 .07  0 0 0 1.6 0"/></filter></defs>`;
  svg.innerHTML = `${art ? tints : ""}<g class="edges">${paths}</g>${nodes}`;
  if (art) svg.classList.add("inked");
  const wrap = el("div", `map${inSheet ? " insheet" : ""}`);
  if (g.kinds?.length) wrap.append(routePlanner(g, svg, { el, send, uiArt, MAP_ART, art }));
  // The act's page under the map: its top, its middle repeated, its bottom.
  const page = el("div", "mappage", svg);
  const area = (g.area || "").toLowerCase();
  const parts = ["top", "middle", "bottom"].map((p) => uiArt(`map_${p}_${area}`) ?? uiArt(`map_${p}_overgrowth`));
  if (art && parts.every(Boolean)) {
    page.classList.add("parchment");
    page.style.setProperty("--pg-top", `url("${parts[0].href}")`);
    page.style.setProperty("--pg-mid", `url("${parts[1].href}")`);
    page.style.setProperty("--pg-bot", `url("${parts[2].href}")`);
  }
  const clip = el("div", "mapclip", page);
  wrap.append(inSheet ? clip : el("div", "mapscroll", clip));
  return wrap;
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

// On a fresh map: start at where you stand, then glide to the next row.
let mapFresh = false;
function scrollMapToYou() {
  const box = document.querySelector("#screen .mapscroll");
  if (!box) return;
  const at = (n, f) => {
    const r = n.getBoundingClientRect();
    const b = box.getBoundingClientRect();
    return box.scrollTop + r.top - b.top - b.height * f;
  };
  const next = box.querySelector("g.node.reach");
  const you = box.querySelector("g.node.you");
  if (!next && !you) return;
  const glide = mapFresh && you && next && !reduced() && !document.body.classList.contains("proof");
  mapFresh = false;
  if (!glide) {
    box.scrollTop = at(next ?? you, 0.68);
    return;
  }
  box.scrollTop = at(you, 0.8);
  requestAnimationFrame(() => box.scrollTo({ top: at(next, 0.68), behavior: "smooth" }));
}

// -- the dock: End Turn, Proceed, the next rooms --

function renderDock(s) {
  const dock = $("dock");
  dock.replaceChildren();
  if (view.fight) {
    const you = blocksOf(s, "You")[0];
    const bar = el("div", "fightbar");
    if (you) {
      // The energy and the piles in one group (a laptop stacks it at the left, over the hand's gutter).
      const left = el("div", "fleft", energyOrb(you, s), el("div", "fpiles"));
      const piles = left.lastChild;
      piles.append(
        pileButton("draw_pile", you.draw, "Draw pile", "draw"),
        pileButton("discard_pile", you.discard, "Discard pile", "discard"),
      );
      if (you.exhaust) piles.append(pileButton("exhaust_pile", you.exhaust, "Exhaust pile", "exhaust"));
      fx.undoButton(piles);
      bar.append(left);
    }
    if (s.primary) {
      const end = primary(s.primary, "endturn");
      // The game's plate; it glows once nothing in the hand can be played.
      if (uiArt("end_turn_button")) {
        end.classList.add("plate");
        end.style.setProperty("--plate", `url("${uiArt(!canPlay(s) && uiArt("end_turn_button_glow") ? "end_turn_button_glow" : "end_turn_button").href}")`);
        if (!canPlay(s)) end.classList.add("nothing-left");
      }
      if (prefs.confirmEnd && you?.energy && canPlay(s)) askFirst(end);
      fx.endHint(end, s);
      end.addEventListener("click", () => ((endingTurn = true), sound("enemyturn")), true);
      bar.append(end);
    }
    dock.append(bar);
  } else if (view.decision === "map") {
    // On a phone and a desktop the map's own rooms are the buttons (one tap or click travels; the
    // arrows and Enter reach them too); a mid-size screen keeps these.
    const onMap = new Set(phone() || DESKTOP.matches ? blocksOf(s, "Map").flatMap((g) => [...g.cells.map((c) => c.button), g.boss_button]).filter(Boolean) : []);
    const rows = blocksOf(s, "List").flatMap((l) => l.rows).filter((r) => !onMap.has(r.id));
    if (rows.length) {
      const grid = el("div", `next n${Math.min(rows.length, 4)}`);
      for (const r of rows) grid.append(nextRoom(r));
      dock.append(grid);
    }
  } else if (s.primary) {
    dock.append(primary(s.primary, "wide"));
  } else if (phone() && escHinted(s)) {
    dock.append(tap(el("button", "primary wide back", "Back"), () => send("Back")));
  }
  dock.hidden = !dock.childElementCount;
  document.body.style.setProperty("--dock", dock.hidden ? "0px" : `${dock.getBoundingClientRect().height}px`);
  document.body.style.setProperty("--top", `${$("top").getBoundingClientRect().height}px`);
  if (view.decision === "map") requestAnimationFrame(scrollMapToYou);
}

// The dock and top bar change height after they are drawn (the Hint row, fonts, a wrapping row of relics).
new ResizeObserver(() => {
  const dock = $("dock");
  document.body.style.setProperty("--dock", dock.hidden ? "0px" : `${dock.getBoundingClientRect().height}px`);
  document.body.style.setProperty("--top", `${$("top").getBoundingClientRect().height}px`);
}).observe($("dock"));
new ResizeObserver(() => document.body.style.setProperty("--top", `${$("top").getBoundingClientRect().height}px`)).observe($("top"));

const phone = () => PHONE.matches;

const canPlay = (s) => blocksOf(s, "List").some((l) => l.rows.some((r) => r.card && stateOf(r) === "Enabled"));

// End Turn with energy left and a card to play: the first tap asks, a second within 3 s ends it.
function askFirst(end) {
  let asked = false;
  let timer = 0;
  end.addEventListener("click", (ev) => {
    if (asked) return;
    ev.stopImmediatePropagation();
    asked = true;
    buzz(10);
    const label = [...end.childNodes];
    end.replaceChildren("Energy left", el("small", "endhint", "tap again"));
    end.classList.add("s-picked");
    timer = setTimeout(() => {
      asked = false;
      end.replaceChildren(...label);
      end.classList.remove("s-picked");
    }, 3000);
  }, true);
  end.addEventListener("click", () => clearTimeout(timer));
}

function energyOrb(you, s) {
  const playable = canPlay(s);
  const was = before?.fight ? blocksOf(before.screen, "You")[0]?.energy : null;
  const cls = ["energy", you.energy ? "" : "out", you.energy && playable && !s.title ? "live" : "", was != null && you.energy < was ? "spent" : ""];
  // The character's own counter: its layers stacked, the middle ones turning (the scene's
  // `RotationLayers`: layer 2, and layer 3 when more lie above it).
  const who = CHARACTER_IDS[view?.save?.character]?.toLowerCase();
  const layers = [1, 2, 3, 4, 5].map((n) => [n, uiArt(`${who}_orb_layer_${n}`)]).filter(([, a]) => a);
  if (!layers.length) return el("span", cls.filter(Boolean).join(" "), el("b", null, `${you.energy}`), el("small", null, `/${you.max_energy}`));
  const top = Math.max(...layers.map(([n]) => n));
  const orb = el("span", [...cls, "orbart"].filter(Boolean).join(" "));
  for (const [n, a] of layers) {
    const img = Object.assign(el("img", `layer${n === 2 || (n === 3 && top > 3) ? " turn" : ""}`), { src: a.href, alt: "" });
    // The turn runs on the page's clock, so a redraw does not snap it back to the start.
    img.style.animationDelay = `-${Math.round(performance.now() % 14000)}ms`;
    orb.append(img);
  }
  const num = el("b", null, `${you.energy}/${you.max_energy}`);
  orb.style.setProperty("--orbline", ORB_LINE[who] ?? "#333");
  orb.append(num);
  return orb;
}

// A pile's button: the game's pile icon with its count on the corner.
function pileButton(id, count, word, sheet) {
  const a = uiArt(id);
  const b = a
    ? el("button", "pile pileart", Object.assign(el("img", null), { src: a.href, alt: "" }), el("b", null, `${count}`))
    : el("button", "pile", el("b", null, `${count}`), el("small", null, word));
  b.setAttribute("aria-label", `${word}: ${count}`);
  b.title = word;
  b.dataset.pile = sheet;
  return tap(b, () => open(sheet));
}

function primary(b, cls) {
  const st = stateOf(b);
  const label = st === "Picked" && b.confirm ? forTouch(plain(b.confirm)) : plain(inGameWords(b.label));
  const e = el("button", `primary ${cls} s-${st.toLowerCase()}`, label);
  e.disabled = st === "Disabled";
  e.dataset.bid = b.id;
  tap(e, () => send({ Tap: b.id }));
  return e;
}

function nextRoom(r) {
  const spans = r.label ?? [];
  const glyph = (spans[0]?.text ?? "").trim();
  const name = spans.slice(1).map((s) => s.text).join("").trim();
  const kind = GLYPH_ICON[glyph] ?? "unknown";
  const pic = uiArt(MAP_ART[kind] ?? "map_unknown");
  const sign = pic ? Object.assign(el("img", "nrart"), { src: pic.href, alt: "" }) : icon(kind, "big");
  const b = el("button", `nextroom r-${spans[0]?.role ?? "Plain"}`, sign, el("span", "nname", name));
  b.dataset.bid = r.id;
  tap(b, () => send({ Tap: r.id }));
  return b;
}

// -- sheets --

function showLocal(sheet) {
  local = sheet;
  renderSheet();
}

function closeSheet() {
  if (local) {
    local.onClose?.();
    local = null;
    renderSheet();
  } else if (view?.sheet) send("Back");
}

function renderSheet() {
  const root = $("sheet");
  const body = el("div", "sheetbody");
  let title = null;
  let dismiss = true;
  let lift = false;
  if (local) {
    title = local.title;
    body.append(...local.body());
  } else if (view?.sheet) {
    const sh = view.sheet;
    const [kind, v] = Object.entries(sh.sheet ?? {})[0] ?? [];
    title = plain(v?.title ?? sh.title);
    for (const b of sh.blocks) if (b.Map) body.append(mapView(b.Map, true));
    if (kind === "Menu") body.append(menuSheet(v.rows));
    else if (kind === "Detail" && v.cards?.length) body.append(pileView(v.cards));
    else if (kind === "Detail" && view.sheet_kind === "Relics") body.append(relicsList());
    else if (kind === "Detail" && view.sheet_kind === "Info") body.append(...infoBody(v.lines));
    else if (kind === "Detail" && view.sheet_kind === "History") body.append(...historyBody(v.lines));
    else if (kind === "Detail" && view.sheet_kind !== "Map") body.append(detailLines(v.lines), ...v.buttons.map(row));
    lift = view.sheet_kind === "Map";
  } else if (view?.screen.sheet && view.fight) {
    const [kind, v] = Object.entries(view.screen.sheet)[0];
    dismiss = false;
    if (kind === "Choice") {
      title = plain(v.title);
      const grid = el("div", "cards");
      v.rows.forEach((r) => grid.append(cardTile(r, { armed: stateOf(r) === "Picked" })));
      body.append(grid);
    } else {
      title = plain(v.title);
      if (v.lines) body.append(detailLines(v.lines));
      for (const r of v.rows ?? v.buttons ?? []) body.append(row(r));
    }
  }
  if (title == null) {
    if (root.hidden || root.classList.contains("closing")) return;
    if (reduced() || document.body.classList.contains("proof") || root.classList.contains("pinned")) {
      root.hidden = true;
      root.className = "";
      root.replaceChildren();
      return;
    }
    root.classList.add("closing");
    setTimeout(() => {
      if (!root.classList.contains("closing")) return;
      root.hidden = true;
      root.className = "";
      root.replaceChildren();
    }, 180);
    return;
  }
  root.hidden = false;
  root.className = `${dismiss ? "" : "pinned"}${!dismiss && choiceHidden ? " collapsed" : ""}`;
  const head = el("div", "sheethead", el("span", "grip"), el("h2", null, title));
  if (dismiss) head.append(tap(el("button", "iconbtn close", icon("close")), closeSheet));
  else {
    head.append(tap(el("button", "chipbtn", choiceHidden ? "Show" : "Look"), () => {
      choiceHidden = !choiceHidden;
      renderSheet();
    }));
  }
  const panel = el("div", "panel", head, body);
  root.replaceChildren(el("div", "backdrop"), panel);
  if (dismiss) tap(root.firstChild, closeSheet);
  if (lift) requestAnimationFrame(() => {
    const n = body.querySelector("g.node.you") ?? body.querySelector("g.node.reach");
    if (n) body.scrollTop = n.getBoundingClientRect().top - body.getBoundingClientRect().top - body.clientHeight / 2;
  });
  runHooks("sheet");
}

// The deck or a pile as card tiles: identical cards stacked with a count, sorted by type, cost
// or name.
let pileSort = "type";
const TYPE_ORDER = ["Attack", "Skill", "Power", "Status", "Curse"];
const costOf = (c) => (/^\d+/.test(c.cost) ? Number.parseInt(c.cost, 10) : c.cost.startsWith("X") ? 50 : 99);
const typeOf = (c) => (TYPE_ORDER.includes(c.kind) ? TYPE_ORDER.indexOf(c.kind) : 9);

function pileView(faces) {
  const groups = new Map();
  for (const c of faces) {
    const k = `${c.name}|${c.cost}|${plain(c.effect)}`;
    const g = groups.get(k);
    if (g) g.n++;
    else groups.set(k, { c, n: 1 });
  }
  const by = {
    type: (a, b) => typeOf(a) - typeOf(b) || costOf(a) - costOf(b) || a.name.localeCompare(b.name),
    cost: (a, b) => costOf(a) - costOf(b) || typeOf(a) - typeOf(b) || a.name.localeCompare(b.name),
    name: (a, b) => a.name.localeCompare(b.name),
  }[pileSort];
  const list = [...groups.values()].sort((a, b) => by(a.c, b.c));
  const wrap = el("div", "pile-view");
  const seg = el("div", "sorts", el("span", null, "Sort"));
  for (const [k, label] of [["type", "Type"], ["cost", "Cost"], ["name", "Name"]]) {
    seg.append(tap(el("button", pileSort === k ? "on" : "", label), () => {
      pileSort = k;
      const body = document.querySelector("#sheet .sheetbody");
      body?.querySelector(".pile-view")?.replaceWith(pileView(faces));
    }));
  }
  const counts = TYPE_ORDER.map((t) => [t, faces.filter((c) => c.kind === t).length]).filter(([, n]) => n);
  wrap.append(seg, el("p", "note tally", counts.map(([t, n]) => `${n} ${t.toLowerCase()}${n > 1 ? "s" : ""}`).join(" · ")));
  const grid = el("div", "cards");
  for (const { c, n } of list) {
    const b = { id: 0, state: "Enabled", label: [], detail: [], card: c };
    const t = cardTile(b, { onTap: () => showLocal(cardSheet(b)) });
    if (n > 1) t.append(el("span", "count", `×${n}`));
    grid.append(t);
  }
  wrap.append(grid);
  return wrap;
}

const MENU_ICONS = { d: "deck", v: "relic", i: "potion", m: "map", l: "log", a: "draw", s: "discard", x: "exhaust", h: "stairs", "?": "unknown", o: "gear", q: "back" };
// The game's own art for an entry (the relics tile shows your first relic).
const MENU_ART = { d: "top_bar_deck", i: "potion_icon", m: "top_bar_map", a: "draw_pile", s: "discard_pile", x: "exhaust_pile", h: "map_monster", o: "top_bar_settings", q: "back_button" };
const FIGHT_KEYS = "lsax";

function menuSheet(rows) {
  const wrap = el("div", "menu");
  const core = el("div", "tiles");
  const fight = el("div", "tiles fighttiles");
  for (const r of rows) {
    const k = r.key?.Char;
    // Deck and Map are the top bar's buttons; the menu keeps what the bar lacks.
    if (!k || k === "d" || k === "m") continue;
    const label = plain(r.label).replace("Quit (the run is saved)", "Save & quit");
    const pic = k === "v" ? artImg("relic", view.screen.top.relics?.[0]?.art, "uiic") : MENU_ART[k] ? uiIcon(MENU_ART[k], MENU_ICONS[k]) : null;
    // The map's skull is a small drawing in a large frame: it is scaled to the other tiles' size.
    if (k === "h") pic?.classList?.add("zoom");
    const t = tap(el("button", `tile${k === "q" ? " quit" : ""}`, pic ?? icon(MENU_ICONS[k] ?? "unknown"), el("span", null, label)), () => send({ Tap: r.id }));
    (FIGHT_KEYS.includes(k) ? fight : core).append(t);
  }
  wrap.append(core);
  // The fight's own entries sit below, so the rest never moves between a fight and a room.
  if (fight.children.length) wrap.append(fight);
  return wrap;
}

function detailLines(lines) {
  const box = el("div", "lines");
  for (const l of lines) {
    const t = plain(l);
    if (!t.trim()) {
      box.append(el("div", "gap"));
      continue;
    }
    const head = l.length && l.every((s) => s.role === "Title" || (s.role === "Plain" && !s.text.trim()));
    // A list entry (`4x Strike (Attack, Basic, 1 energy)`) and the text under it.
    const entry = /\((Attack|Skill|Power|Status|Curse|Quest)\b|^\d+x |\(\d+ energy\)$/.test(t);
    box.append(el("div", head ? "line head" : entry ? "line entry" : "line", rich(l)));
  }
  return box;
}

// `shown`: opened from a row that already shows the text: its glossary and the actions.
function potionSheet(slot, shown = false) {
  return {
    title: view.screen.top.potions[slot]?.name ?? "Potion",
    body() {
      const p = view.screen.top.potions[slot];
      if (!p || p.empty) {
        local = null;
        return [];
      }
      const out = shown ? [glossary(p.detail ?? [])].filter(Boolean) : [textSheet(p.detail)];
      const use = el("button", `primary wide${p.usable ? "" : " s-disabled"}`, p.usable ? "Use" : "Can't use now");
      use.disabled = !p.usable;
      tap(use, () => {
        local = null;
        send({ Tap: p.id });
        const now = view.screen.top.potions[slot];
        if (now?.selected && now.asks) send({ Tap: now.id });
      });
      const toss = tap(el("button", "row danger", el("span", "main", el("span", "lab", "Throw away"))), () => {
        local = null;
        apply(game.toss(slot));
      });
      out.push(el("div", "actions", use, toss));
      return out;
    },
  };
}

// The game's explanations of a text's lit words (Doom, Block, Vulnerable) and of `names`.
function glossary(texts, names = []) {
  const words = new Set(names);
  for (const r of texts)
    for (const sp of r ?? [])
      if (!["Body", "Plain", "Muted"].includes(sp.role) && /[A-Za-z]/.test(sp.text)) words.add(sp.text.trim());
  if (!words.size) return null;
  const terms = JSON.parse(game ? game.glossary(JSON.stringify([...words])) : library_glossary(JSON.stringify([...words])));
  if (!terms.length) return null;
  return el("div", "terms", terms.map((t) => el("div", "term", el("b", null, t.title), el("p", null, rich(t.text)))));
}

// The sheet's title is the card's name, unless the game's card, large, says it already.
function cardSheet(b) {
  const c = b.card;
  return {
    title: c && uiArt("card_frame_attack_red") ? "" : c?.name ?? plain(b.label),
    body() {
      const text = c?.text?.length ? [c.text] : b.detail ?? [];
      const kind = [c?.kind, c?.rarity, c?.cost && c.cost !== "—" ? `${c.cost.replace("·s", " ★")} energy` : null].filter(Boolean).join(" · ");
      const why = dimReason(b);
      // The card itself, large, as the game shows a card it inspects.
      if (c && uiArt("card_frame_attack_red")) {
        const name = c.name ?? plain(b.label);
        const big = el("div", `bigcard card gamecard inspect ${kindClass(c.kind)}${/\+\d*$/.test(name) ? " upgraded" : ""}`, ...gameCard({ ...c, effect: text[0] ?? c.effect }, name, (c.cost ?? "").replace("·s", " ★")));
        return [el("div", "inspectwrap", big), why ? el("p", "note", why) : null, upgradeSection(c), glossary(text)].filter(Boolean);
      }
      return [
        el("div", `bigcard card ${kindClass(c?.kind)}${c?.rarity ? ` rar-${c.rarity.toLowerCase()}` : ""}`, el("span", "ckind", kind), el("div", "cfull", text.map((r) => el("p", null, rich(r))))),
        why ? el("p", "note", why) : null,
        upgradeSection(c),
        glossary(text),
      ].filter(Boolean);
    },
  };
}

// A creature's sheet, opened from its card, which shows its name, HP, Block, next move and powers
// (a picture and its amount): only what the card does not, the move in words where they say more,
// the game's explanations, and a power's name the card shows only as a picture. Null when nothing is left.
function creatureSheet(cr) {
  const said = (r) => plain(r).replace(/^\S+\s*/, "").replace(/\s+/g, " ").trim();
  const long = cr.intent_long?.length ? cr.intent_long : null;
  const tips = view.intent_tips?.[cr.id] ?? [];
  // The game's tip says the move in words; without its text, the long form where it says more.
  const move = tips.length ? null : long && said(long) !== said(cr.intent ?? []) ? long : null;
  const shown = (cr.powers ?? []).filter((p) => p.shown);
  // The move's own words (`Thorns 2`, `Shrink -1`) name a keyword too.
  const moveWord = said(cr.intent ?? []).replace(/[-+−]?\d+(×\d+)?/g, "").trim();
  const g = glossary(tips.length ? tips.map((t) => t.text) : [long ?? cr.intent ?? []], [...shown.map((p) => p.name), ...(moveWord && !tips.length ? [moveWord] : [])]);
  const named = new Set([...(g?.querySelectorAll(".term > b") ?? [])].map((b) => b.textContent.trim()));
  const unnamed = shown.filter((p) => !named.has(p.name) && artFiles.has(`power/${p.art}`));
  if (!move && !tips.length && !g && !unnamed.length) return null;
  return {
    title: cr.name,
    body() {
      const out = [];
      for (const t of tips) out.push(el("div", "tip", el("b", null, t.title), el("p", null, rich(t.text))));
      if (move) out.push(el("div", "tip", el("b", null, "Next move"), el("p", null, rich(move))));
      if (unnamed.length) out.push(el("div", "powers big", unnamed.map((p) => el("span", `pw${p.debuff ? " debuff" : ""}`, p.name))));
      if (g) out.push(g);
      return out;
    },
  };
}
const readCreature = (cr) => {
  const sheet = creatureSheet(cr);
  if (sheet) showLocal(sheet);
};

// One row of choices: `[value, label, icon]`; the one in `prefs[key]` is lit.
function segment(key, options, then) {
  const seg = el("div", `seg n${options.length}`);
  for (const [k, label, ic] of options) {
    seg.append(tap(el("button", prefs[key] === k ? "on" : "", ic ? icon(ic) : null, label), () => {
      prefs[key] = k;
      savePrefs();
      applyPrefs();
      then?.();
      // End Turn's asking first applies at once, not on the next tap.
      if (game && view) {
        renderDock(view.screen);
        runHooks("dock");
      }
      renderSheet();
    }));
  }
  return seg;
}

function settingsSheet() {
  return {
    title: "Settings",
    body() {
      const out = [
        el("h3", null, "Theme"),
        segment("theme", [["dark", "Dark", "moon"], ["light", "Light", "sun"], ["auto", "Phone", "phone"]]),
        el("h3", null, "Sound"),
        segment("sound", [[true, "On", "sound"], [false, "Off", "mute"]], () => {
          unlockSound();
          sound("block");
        }),
        prefs.sound ? segment("volume", [["low", "Low"], ["mid", "Medium"], ["high", "High"]], () => sound("gold")) : null,
        el("h3", null, "Motion"),
        segment("motion", [["auto", "Phone"], ["full", "Full"], ["reduced", "Reduced"]]),
        el("h3", null, "Text size"),
        segment("text", [["normal", "Normal"], ["large", "Large"]]),
        el("h3", null, "End Turn"),
        segment("confirmEnd", [[false, "One tap"], [true, "Ask if energy is left"]]),
        ...(window.sts2platform?.settings(segment) ?? []),
        el("h3", null, "Bot hints"),
        segment("hints", [["manual", "Hint button"], ["always", "Always show"]], () => runHooks("screen")),
        el("p", "note", "Always show outlines the bot's move in gold at every decision."),
      ];
      if (game && view) out.push(el("h3", null, "This run"), el("p", "note runinfo", `Seed ${view.save.seed} · Ascension ${view.save.ascension}`), ...modifierNotes(view.run?.modifiers), shareThisRun(view.save));
      out.push(el("h3", null, "Bug report"), bugButton());
      out.push(el("p", "note", `Game text: ${textTables ? `${textTables} tables loaded` : "not found (engine names)"}.`));
      return out;
    },
  };
}

// Copy bug report: the text goes to the clipboard or the share sheet; if neither works it shows
// below the button to select by hand.
function bugButton() {
  const box = el("div", "bugbox");
  const btn = tap(el("button", "chipbtn", icon("unknown"), "Copy bug report"), async () => {
    const save = game && view ? view.save : JSON.parse(localStorage.getItem(SAVE) ?? "null");
    const text = bugReport(save, game && view ? view : null, CHARACTERS.find((c) => c.id === save?.character)?.name);
    window.sts2.lastReport = text;
    const r = await bugDeliver(text);
    if (r === "failed") {
      const t = el("textarea");
      t.readOnly = true;
      t.value = text;
      t.rows = 6;
      box.replaceChildren(t);
      t.focus();
      t.select();
    }
    toast([{ role: "Plain", bold: false, text: { copied: "Bug report copied", shared: "Bug report shared", failed: "Select the report and copy it", cancelled: "" }[r] }]);
  });
  box.append(btn);
  return box;
}

// The start screen's ascension and seed for the next run.
const startOpts = { practice: new URLSearchParams(location.search).has("practice"), asc: JSON.parse(localStorage.getItem("sts2-last") ?? "null")?.asc ?? 0, seed: new URLSearchParams(location.search).get("seed") ?? "" };
// The game's ascension text (null without an import): the picker shows the level's title in
// bold gold over its description; the levels stack, so a tap lists every one in force.
let ascText = null;
const ascKey = (n, f) => ascText?.[`LEVEL_${String(n).padStart(2, "0")}.${f}`];
const ascRich = (t) =>
  (t ?? "")
    .split(/(\[(?:gold|blue|red|green)\][\s\S]*?\[\/(?:gold|blue|red|green)\])/)
    .map((x) => {
      const m = /^\[(gold|blue|red|green)\]([\s\S]*)\[\/\1\]$/.exec(x);
      return m ? el("span", `rt-${m[1]}`, modText(m[2])) : modText(x) ? document.createTextNode(x.replace(/\[\/?[a-z_]+\]/gi, "")) : "";
    });
function ascensionSheet(n) {
  return {
    title: "Ascension",
    body: () =>
      Array.from({ length: n }, (_, i) => i + 1).map((i) => el("div", "tip asclvl", el("b", null, `${i} · ${ascKey(i, "title") ?? ""}`), el("p", null, ...ascRich(ascKey(i, "description"))))),
  };
}
function runOpts(withPractice = true) {
  const ascVal = el("b", null, `${startOpts.asc}`);
  const info = el("button", "ascinfo");
  const paintAsc = () => {
    const n = startOpts.asc;
    const title = ascKey(n, "title");
    info.hidden = !title;
    info.disabled = n === 0;
    info.dataset.level = `${n}`;
    info.replaceChildren(el("span", "asctext", el("b", "asctitle", title ?? ""), " ", el("span", "ascdesc", ...ascRich(ascKey(n, "description")))), n > 0 ? el("i", "asclist", `1–${n}`) : "");
    info.setAttribute("aria-label", n > 0 ? `Ascension ${n}: ${title}. Show levels 1 to ${n}` : `Ascension 0: ${title}`);
  };
  tap(info, () => startOpts.asc > 0 && showLocal(ascensionSheet(startOpts.asc)));
  const step = (d) => {
    startOpts.asc = Math.max(0, Math.min(10, startOpts.asc + d));
    ascVal.textContent = `${startOpts.asc}`;
    paintAsc();
  };
  paintAsc();
  const seed = el("input");
  seed.placeholder = "random";
  seed.autocapitalize = "characters";
  seed.autocomplete = "off";
  seed.spellcheck = false;
  seed.value = startOpts.seed;
  seed.addEventListener("input", () => (startOpts.seed = seed.value));
  return el(
    "div",
    "opts",
    el("label", "opt", el("span", null, "Ascension"), el("span", "stepper", tap(el("button", null, "−"), () => step(-1)), ascVal, tap(el("button", null, "+"), () => step(1)))),
    el("label", "opt", el("span", null, "Seed"), seed),
    info,
    withPractice ? fx.practiceOption(startOpts) : null,
  );
}

// The first run's one screen of help; never again once closed.
function tutorialSheet() {
  const steps = [
    ["play", "Tap or drag a card to play it", "Drag it onto an enemy, or tap it and tap one. Drag it back down to put it back."],
    ["deck", "Hold a card to read it", "Hold a card, or tap an enemy or yourself, for the full text and what each keyword means."],
    ["energy", "Spend your energy, then End Turn", "The orb at the bottom left is your energy; it refills every turn."],
  ];
  return {
    title: "How to play",
    onClose() {
      prefs.tutored = true;
      localStorage.setItem(PREFS, JSON.stringify(prefs));
    },
    body: () => [
      el("div", "steps", steps.map(([ic, h, t], i) => el("div", "step", el("span", "num", `${i + 1}`), el("div", null, el("b", null, icon(ic), h), el("p", null, t))))),
      tap(el("button", "primary wide", "Let's go"), closeSheet),
    ],
  };
}

function helpSheet() {
  const tips = [
    ["Play a card", "Drag it up onto a lit enemy, or anywhere above the hand if it needs no target; drag it back down to keep it. Or tap it, then tap an enemy."],
    ["Read anything", "Hold a card for its full text and keywords. Tap an enemy, yourself or Osty for powers and the next move."],
    ["Read the fight", phone() ? "Enemy cards show their next move." : "The red line under the enemies is what their turn does to you. Enemy cards show their next move."],
    ["Previews", "While a card waits for its target, each enemy shows its HP after the hit."],
    ["Potions", "Tap one in the belt under the top bar to read it, use it or throw it away."],
    ["Piles and lists", "Tap the draw and discard counts by End Turn. The deck, map and menu are at the top right."],
    ["Back", "The phone's Back closes a sheet or cancels an aimed card; it opens the menu otherwise."],
    ["Saving", "The run saves after every tap. Continue on the start screen picks it up."],
  ];
  return {
    title: "How to play",
    body: () => tips.map(([h, t]) => el("div", "tip", el("b", null, h), el("p", null, t))),
  };
}

// -- feedback --

let endingTurn = false;
let bannerTimer = 0;
function banner(big, small, cls = "", ms = 1300) {
  const b = $("banner");
  clearTimeout(bannerTimer);
  b.className = "";
  void b.offsetWidth;
  b.className = `show ${cls}`;
  b.style.setProperty("--ms", `${ms}ms`);
  b.replaceChildren(el("b", null, big), small ? el("small", null, small) : null);
  if (!window.sts2?.pinBanner) bannerTimer = setTimeout(() => (b.className = ""), ms);
}

// The run's end: a full-screen beat (Defeat or Victory, what ended it), then the end screen.
// A tap or a few seconds clear it.
function endMoment() {
  if (!view.over || before?.over || !before) return;
  const won = view.decision === "victory";
  const text = blocksOf(view.screen, "Text")[0] ?? [];
  const cause = plain(text.find((l) => plain(l).trim()) ?? []);
  const m = el(
    "div",
    `moment ${won ? "won" : "lost"}`,
    el("div", "mark", icon(won ? "star" : "skull")),
    el("b", null, won ? "Victory" : "Defeat"),
    cause ? el("p", null, cause) : null,
    el("small", null, `Floor ${view.save.floor} · tap to continue`),
  );
  buzz(won ? [20, 40, 20, 40, 60] : [80, 60, 120]);
  sound(won ? "victory" : "defeat");
  if (!won) setTimeout(() => sound("die", { who: character(view.save.character).name }), 500);
  setTrack(null);
  const done = () => {
    m.classList.add("out");
    setTimeout(() => m.remove(), 300);
  };
  tap(m, done);
  document.body.append(m);
  if (!window.sts2?.pinMoment) setTimeout(done, 3200);
}

// Your turn starts: when a fight begins and after End Turn. No banner: play goes on at once.
function turnMoment() {
  const ended = endingTurn;
  endingTurn = false;
  if (before?.fight && !view.fight && !view.over) {
    const what = { Boss: "Boss defeated", Elite: "Elite defeated" }[view.room] ?? "Fight won";
    banner("Victory", what, "win");
    sound("victory");
    return;
  }
  if (!view.fight || view.over) return;
  const round = /Round (\d+)/.exec(plain(blocksOf(view.screen, "Last")[0]))?.[1];
  if (!before?.fight) {
    const foes = [...new Set(blocksOf(view.screen, "Enemy").map((e) => e.name))].join(" · ");
    if (view.room === "Boss" || view.room === "Elite") banner(view.room, foes, `start ${view.room.toLowerCase()}`, 1900);
    sound("start");
    dealHand();
    return;
  }
  const prevRound = /Round (\d+)/.exec(plain(blocksOf(before.screen, "Last")[0]))?.[1];
  const energy = (v) => blocksOf(v.screen, "You")[0]?.energy ?? 0;
  const last = plain(blocksOf(view.screen, "Last")[0]);
  const advanced = /^Enemy turn/.test(last) || (round && round !== prevRound) || energy(view) > energy(before);
  if (!ended || !advanced) return;
  const was = before.screen.top?.hp ?? 0;
  const now = view.screen.top?.hp ?? 0;
  const lost = was - now;
  if (lost > 0) buzz([30, 50, 30]);
  setTimeout(() => sound("turn"), 350);
  dealHand();
}

let toastTimer = 0;
function toast(status) {
  const t = $("toast");
  clearTimeout(toastTimer);
  if (!status?.length) {
    t.classList.remove("show");
    return;
  }
  t.replaceChildren(rich(status));
  t.classList.add("show");
  toastTimer = setTimeout(() => t.classList.remove("show"), 2600);
}

// A card leaves the hand: its ghost flies to `target` (an enemy or Osty), else to you (a skill or
// power) or up into the field (an attack), then into its pile (fightfx.js). Nothing flies if the
// tap only aimed it.
let pendingFly = null;
function flyFrom(tile, cardId, target = null) {
  if (reduced() || document.body.classList.contains("proof")) return;
  pendingFly = { rect: tile.getBoundingClientRect(), ghost: tile.cloneNode(true), cardId, to: target?.getBoundingClientRect() ?? null };
}

function dealHand() {
  sound("draw");
  if (!reduced()) document.querySelector("#screen .hand")?.classList.add("deal");
}

// The music for where the run is: the act's track by room, a boss's own, else the menu's.
function music() {
  if (!view || view.over) return;
  const a = Math.min(Math.max(view.act ?? 1, 1), 3);
  const act = (s) => `act${a}_a1_${s}`;
  if (view.fight && view.room === "Boss") {
    const slugs = [...new Set(blocksOf(view.screen, "Enemy").map((e) => String(e.name).toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "")))];
    return setTrack([...slugs.flatMap((s) => [`boss_${s}_0`, `boss_the_${s}_0`, `boss_${s.replace(/^the_/, "")}_0`]), act("elite")]);
  }
  if (view.fight) return setTrack(act(view.room === "Elite" ? "elite" : "enemy"));
  setTrack(act(view.decision === "shop" ? "merchant" : "unknown"));
}

// The sounds of what the last input did: a card played, a hit, block, gold.
// `played`: the fight's events are being played back, which sound their own hits and block.
function sounds(played) {
  if (!before) return;
  let playedCard = false;
  const at = (v) => (v?.fight ? blocksOf(v.screen, "List").flatMap((l) => l.rows).map((r) => r.card?.card) : []);
  if (view.fight && before.fight && !endingTurn) {
    const now = new Set(at(view));
    if (at(before).some((c) => c != null && !now.has(c))) playedCard = true;
  }
  const hit = view.fight && blocksOf(view.screen, "Enemy").some((e) => (prevEnemy(e.id)?.hp ?? e.hp) > e.hp);
  const you = (v) => (v?.fight ? blocksOf(v.screen, "You")[0] : null);
  const hurt = (before.screen.top?.hp ?? 0) > (view.screen.top?.hp ?? 0);
  const blocked = view.fight && before.fight && (you(view)?.block ?? 0) > (you(before)?.block ?? 0);
  const gold = (view.screen.top?.gold ?? 0) > (before.screen.top?.gold ?? 0);
  if (playedCard) sound("play", { who: character(view.save.character).name, attack: hit });
  const later = (name, ms) => setTimeout(() => sound(name), ms);
  if (hit && !played) later("hit", 90);
  if (hurt && !played) later("hurt", hit ? 260 : 60);
  if (blocked && !played) later("block", 70);
  if (gold) later("gold", 40);
}

function animate() {
  const flying = pendingFly;
  pendingFly = null;
  // A peeked or redrawn moment (before is the view itself) replays nothing.
  const played = before !== view && ffx.play(flying);
  if (before !== view) ffx.reveal(view.reveal);
  sounds(played);
  fx.floats();
  requestAnimationFrame(() => {
    for (const f of document.querySelectorAll(".hpbar .fill[data-to]:not([data-step])")) f.style.width = f.dataset.to;
    if (reduced() || played) return;
    for (const e of document.querySelectorAll(".enemy[data-loss], .you .me[data-loss]:not([data-loss=''])")) {
      e.classList.add("hit");
      e.append(el("span", "float", `−${e.dataset.loss}`));
    }
    for (const e of document.querySelectorAll(".you .me[data-gain]:not([data-gain='']), .enemy[data-gain]")) {
      e.classList.add("glow");
      e.append(el("span", "float gain", `+${e.dataset.gain}`));
    }
    const hp = before?.screen.top?.hp;
    const now = view.screen.top?.hp;
    if (hp != null && now != null && now < hp) {
      const s = document.querySelector("#top .hp");
      s?.classList.add("hurt");
    }
  });
}

// ---- modifiers (Custom Run) -----------------------------------------------------------------

// A modifier's text with the game's markup ([gold]…[/gold], {Amount}) taken out.
const modText = (t) => (t ?? "").replace(/\[\/?[a-z_]+(?:=[^\]]*)?\]/gi, "").replace(/\s+/g, " ").trim();

function modifierNotes(mods) {
  if (!mods?.length) return [];
  return [el("h3", null, "Modifiers"), ...mods.map((m) => el("p", `note mod ${m.good ? "good" : "bad"}`, el("b", null, m.title), ` ${modText(m.description)}`))];
}

function modsSheet(mods) {
  return { title: "Modifiers", body: () => modifierNotes(mods).slice(1) };
}

let customSel = new Set();
// The Custom Run screen: hero, ascension, seed and the modifier list; Randomize picks two good
// and one bad. A modifier that excludes another greys it out while it is on.
function showCustom() {
  const root = $("start");
  const rows = JSON.parse(modifier_list());
  const byKey = new Map(rows.map((r) => [r.key, r]));
  const blocked = (key) => {
    for (const on of customSel) {
      if (byKey.get(on)?.excludes.includes(key) || byKey.get(key)?.excludes.includes(on)) return byKey.get(on).title;
    }
    return null;
  };
  const list = el("div", "modlist");
  const paint = () => {
    list.replaceChildren(
      ...rows.map((r) => {
        const on = customSel.has(r.key);
        const by = !on && blocked(r.key);
        const b = el(
          "button",
          `modrow ${r.good ? "good" : "bad"}${on ? " on" : ""}${by ? " off" : ""}`,
          el("span", "modtick", on ? "✓" : ""),
          el("span", "modtext", el("b", null, r.title), el("small", null, modText(r.description))),
        );
        b.title = by ? `Not with ${by}` : modText(r.description);
        b.setAttribute("aria-pressed", on ? "true" : "false");
        b.disabled = !!by;
        tap(b, () => {
          if (on) customSel.delete(r.key);
          else customSel.add(r.key);
          paint();
          count();
        });
        return b;
      }),
    );
  };
  const hero = el("div", "heroes");
  const panel = el("div", "hero");
  const showHero = () => {
    for (const [i, x] of [...hero.children].entries()) x.classList.toggle("on", CHARACTERS[i].id === picked);
    panel.replaceChildren(el("span", "htext", el("b", null, character(picked).name)));
  };
  CHARACTERS.forEach((c) => {
    const b = el("button", "chip", portrait(c));
    b.setAttribute("aria-label", c.name);
    tap(b, () => {
      picked = c.id;
      showHero();
    });
    hero.append(b);
  });
  showHero();
  const n = el("span", "modcount");
  const count = () => (n.textContent = customSel.size ? `${customSel.size} on` : "none on");
  count();
  const random = tap(el("button", "chipbtn", "Randomize"), () => {
    customSel = new Set(JSON.parse(random_modifiers(Math.floor(Math.random() * 2 ** 32), picked)));
    paint();
    count();
    buzz(5);
  });
  const clear = tap(el("button", "chipbtn", "Clear"), () => {
    customSel.clear();
    paint();
    count();
  });
  const go = tap(el("button", "primary wide", "Start run"), () => {
    startRun(picked, startOpts.seed.trim().toUpperCase() || randomSeed(), startOpts.asc, rows.filter((r) => customSel.has(r.key)).map((r) => r.key));
  });
  const back = tap(el("button", "chipbtn back", "‹ Back"), () => showStart());
  paint();
  root.replaceChildren(
    el(
      "div",
      "startin custom",
      el("div", "brand", el("span", "big", "Custom Run")),
      el("div", "scol l", back, el("h3", null, "Hero"), hero, panel, runOpts(false), el("div", "go", go)),
      el("div", "scol r", el("div", "modhead", el("h3", null, "Modifiers"), n, random, clear), list),
    ),
  );
  renderSheet();
}

// ---- start screen -------------------------------------------------------------------------

let picked = 4;
let sharedChar = null; // the hero a share link named, used once by the next start screen

function showStart(error = null) {
  const root = $("start");
  root.hidden = false;
  game = null;
  setTrack("menu");
  const save = JSON.parse(localStorage.getItem(SAVE) ?? "null");
  const last = JSON.parse(localStorage.getItem("sts2-last") ?? "null");
  if (last) picked = last.character;
  if (sharedChar != null) [picked, sharedChar] = [sharedChar, null];
  const kids = [
    el(
      "div",
      "brand",
      el("span", "big", "Slay the Spire 2"),
    ),
  ];
  if (error) kids.push(el("div", "err", error));
  if (save) {
    const c = character(save.character);
    const cont = el(
      "button",
      "continue",
      portrait(c),
      el("span", "ctext", el("b", null, "Continue"), el("small", null, `${c.name} · Floor ${save.floor} · ♥ ${save.hp}/${save.max_hp}${save.ascension ? ` · A${save.ascension}` : ""}`)),
      icon("play"),
    );
    kids.push(tap(cont, continueRun));
  }
  // The hero: five round chips, and the chosen one's panel under them.
  const hero = el("div", "hero");
  const chips = el("div", "heroes");
  const showHero = () => {
    const c = character(picked);
    hero.replaceChildren(el("span", "htext", el("b", null, c.name)));
    for (const [i, x] of [...chips.children].entries()) x.classList.toggle("on", CHARACTERS[i].id === picked);
  };
  CHARACTERS.forEach((c) => {
    const b = el("button", "chip", portrait(c));
    b.setAttribute("aria-label", c.name);
    tap(b, () => {
      picked = c.id;
      buzz(5);
      showHero();
    });
    chips.append(b);
  });
  showHero();
  const go = tap(el("button", "primary wide", save ? "New run" : "Start run"), () => {
    localStorage.setItem("sts2-last", JSON.stringify({ character: picked, asc: startOpts.asc }));
    startRun(picked, startOpts.seed.trim().toUpperCase() || randomSeed(), startOpts.asc);
  });
  const ex = startExtras({ picked: () => picked, opts: startOpts, startRun, custom: showCustom });
  const left = el("div", "scol l");
  if (save) left.append(kids.pop());
  left.append(
    ex.menu,
    el(
      "div",
      "foot",
      ex.share,
      tap(el("button", "chipbtn", icon("gear"), "Settings"), () => showLocal(settingsSheet())),
      tap(el("button", "chipbtn", icon("unknown"), "How to play"), () => showLocal(helpSheet())),
    ),
  );
  const right = el("div", "scol r", el("h3", save ? null : "pickhead", save ? "Or a new run" : "Choose your hero"), chips, hero, runOpts(), el("div", "go", go));
  kids.push(left, right);
  root.replaceChildren(el("div", "startin", kids));
  $("dock").hidden = true;
  renderSheet();
  runHooks("start");
}

// ---- boot ---------------------------------------------------------------------------------

function applyPrefs() {
  applyTheme();
  document.body.classList.toggle("still", reduced());
  document.documentElement.dataset.text = prefs.text;
  setSound(prefs.sound, prefs.volume);
}

function applyTheme() {
  const dark = prefs.theme === "dark" || (prefs.theme === "auto" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.querySelector('meta[name="theme-color"]').content = dark ? "#0e1015" : "#f3efe6";
}

// Our words → the game's (`gameplay_ui.json`, `static_hover_tips.json`), where it has them;
// and the game's lines for the reward rows.
const gameWords = {};
const gameLines = {};
// Our own explanatory lines that the game does not show.
const OUR_LINES = new Set(["Pick one for your deck, or skip."]);
// The game's word for a header or button whose whole text is `rich`, else `rich` itself.
const inGameWords = (r) => {
  const w = gameWords[plain(r).trim()];
  return w ? [{ role: "Plain", bold: false, text: w }] : r;
};

async function loadText() {
  const tables = JSON.parse(text_tables());
  const got = await Promise.all(
    tables.map((t) =>
      fetch(`text/${t}.json`)
        .then((r) => (r.ok ? r.text() : null))
        .catch(() => null),
    ),
  );
  const parts = [];
  tables.forEach((t, i) => {
    if (got[i] && got[i].trim().startsWith("{")) parts.push(`${JSON.stringify(t)}:${got[i]}`);
  });
  // The game's own words for the page's headers and buttons.
  const table = (t) => {
    try {
      return JSON.parse(got[tables.indexOf(t)] ?? "{}");
    } catch {
      return {};
    }
  };
  try {
    const a = await fetch("text/ascension.json").then((r) => (r.ok ? r.json() : null));
    ascText = a && a["LEVEL_01.title"] ? a : null;
  } catch {
    ascText = null;
  }
  const ui = table("gameplay_ui");
  const tips = table("static_hover_tips");
  Object.assign(gameWords, {
    Rewards: ui.COMBAT_REWARD_HEADER_LOOT,
    "Choose a card": ui.CHOOSE_CARD_HEADER,
    "Rest site": tips["ROOM_REST.title"],
    Leave: ui.PROCEED_BUTTON,
    Treasure: tips["ROOM_TREASURE.title"],
    Shop: tips["ROOM_MERCHANT.title"],
  });
  for (const k of Object.keys(gameWords)) if (!gameWords[k]) delete gameWords[k];
  if (ui.COMBAT_REWARD_ADD_CARD) gameLines.addCard = ui.COMBAT_REWARD_ADD_CARD;
  if (ui.COMBAT_REWARD_GOLD) gameLines.gold = ui.COMBAT_REWARD_GOLD;
  if (parts.length) textTables = load_text(`{${parts.join(",")}}`);
}

async function main() {
  const q = new URLSearchParams(location.search);
  if (q.has("theme")) prefs.theme = q.get("theme");
  if (q.has("text")) prefs.text = q.get("text");
  if (q.has("proof") && q.get("proof") !== "tutorial") prefs.tutored = true;
  applyPrefs();
  matchMedia("(prefers-reduced-motion: reduce)").addEventListener("change", applyPrefs);
  // iOS Safari applies :active only when something listens for touches.
  document.addEventListener("touchstart", () => {}, { passive: true });
  // Audio unlocks on the first tap.
  // iOS only lets a touchend/click start audio, so those unlock too (until the context runs).
  for (const t of ["pointerdown", "pointerup", "touchend", "click"]) document.addEventListener(t, unlockSound, { capture: true, passive: true });
  // Cues for pressing: a click on anything pressable (cards in the hand have their own), and the
  // hover cue: the mouse over a pressable on desktop, a finger down on a card in the hand on touch.
  const PRESSABLE = "button, [data-tap], .card, .node, a, summary";
  document.addEventListener("click", (e) => {
    const t = e.target.closest?.(PRESSABLE);
    if (t && !t.closest(".hand")) sound("click");
  }, true);
  let hovered = null;
  document.addEventListener("pointerover", (e) => {
    const t = e.target.closest?.(PRESSABLE);
    if (e.pointerType === "mouse" && t && t !== hovered) sound("hover");
    hovered = t ?? null;
  }, true);
  document.addEventListener("pointerdown", (e) => {
    if (e.pointerType !== "mouse" && e.target.closest?.(".hand .card")) sound("hover");
  }, true);
  if (q.has("lite") || (navigator.hardwareConcurrency ?? 8) <= 4 || (navigator.deviceMemory ?? 8) <= 2) document.body.classList.add("lite");
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", applyTheme);
  await init();
  initHover({ el });
  initExtras({ daily_run, el, tap, rich, plain, icon, character, showLocal, closeSheet, startRun, cardTile, cardSheet, textSheet, artImg, characters: CHARACTERS, glossary, library: { counts: () => JSON.parse(library_counts()), part: (k, f, n) => JSON.parse(library_part(k, f, n)) }, game: () => game, say: (m) => m && toast([{ role: "Plain", bold: false, text: m }]) });
  const shared = q.has("proof") ? null : takeSharedLink();
  if (shared) {
    startOpts.seed = shared.seed ?? startOpts.seed;
    if (shared.asc != null) startOpts.asc = shared.asc;
    sharedChar = shared.char;
  }
  fx.init({ el, blocksOf, stateOf, reduced, game: () => game, view: () => view, before: () => before, undo: () => apply(game.undo()) });
  ffx.init({ el, blocksOf, reduced, sound, view: () => view, before: () => before, tilesBefore: () => tilesBefore, cardTile: (face) => cardTile({ card: face, label: [{ text: face.name, role: "plain" }], state: "Enabled" }, { onTap: () => {} }) });
  await Promise.all([loadText(), loadArt()]);
  warmArt();
  // Android's Back: closes a sheet, cancels an aimed card or steps back a room, else opens the
  // menu; it never leaves a run (Save & quit does).
  window.addEventListener("popstate", () => {
    if (!game) return;
    history.pushState({ run: 1 }, "");
    if (local || view?.sheet) closeSheet();
    else if (document.querySelector("#screen .aimbar .chipbtn, #screen .head .chipbtn, #dock .primary.back")) send("Back");
    else open("menu");
  });
  document.addEventListener("keydown", (e) => {
    if (!game && e.key === "Escape") closeSheet();
  });
  document.addEventListener("keydown", onKey);
  // Right click cancels an aimed card (as the game does), a mouse's Back.
  document.addEventListener("contextmenu", (e) => {
    if (!game || local || view?.sheet || !document.querySelector("#screen .aimbar .chipbtn, #screen .head .chipbtn, #dock .primary.back")) return;
    e.preventDefault();
    e.stopPropagation();
    send("Back");
  }, { capture: true });
  // A mouse or a finger takes over from the keyboard: the ring goes until an arrow brings it back.
  document.addEventListener("pointerdown", () => {
    kbd = null;
    for (const x of document.querySelectorAll(".kbd")) x.classList.remove("kbd");
  }, { capture: true, passive: true });
  window.sts2 = {
    send,
    open,
    view: () => view,
    toss: (slot) => apply(game.toss(slot)),
    start: startRun,
    continueRun,
    replayCode,
    bugReport: () => bugReport(game && view ? view.save : null, game && view ? view : null),
    showStart,
    showLocal,
    potionSheet,
    settingsSheet,
    helpSheet,
    tutorialSheet,
    cardSheet,
    creatureSheet,
    closeSheet,
    text: () => textTables,
    // Test hook (the engine's `Web::debug`: encounter, cards, potions, hp), while the run is on the map.
    debug: (json) => { const r = game.debug(json); apply(game.view()); return r; },
    hook: (f) => hooks.push(f),
    pref: (k) => prefs[k],
    // How the run's action `index` is tapped on the screen as it is (bot.js lights it).
    taps: (index) => JSON.parse(game?.taps(index) ?? "null"),
    // Back to the player's own run (or the start screen) after the page drew another view.
    resume: () => (game ? apply(game.view()) : showStart()),
    // Another moment of a run, drawn with no entrance, sound or moment; `unpeek` puts back what was there (review.js).
    peek(v) {
      peeked ??= { view, local, start: !$("start").hidden };
      local = null;
      $("start").hidden = true;
      before = v;
      view = v;
      render();
    },
    unpeek() {
      if (!peeked) return;
      const { view: was, local: sheet, start } = peeked;
      peeked = null;
      local = sheet;
      before = was;
      view = was;
      if (start) {
        showStart();
        renderSheet();
      } else if (was) render();
    },
    // The proof's perf check draws a made-up view (more enemies and cards than a seed gives).
    draw(v) {
      before = view;
      view = v;
      render();
    },
  };
  window.sts2platform?.attach(window.sts2);
  import("./bot.js");
  import("./odds.js");
  if (q.has("proof")) {
    document.body.classList.add("proof");
    await import("./proof.js");
  }
  else if (q.has("replay")) replayCode(q.get("replay"));
  else showStart();
  if ("serviceWorker" in navigator && !q.has("proof")) navigator.serviceWorker.register("/sw.js").catch(() => {});
}
main();
