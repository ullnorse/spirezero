// The compendium (start screen): every card, relic and potion the engine ports with the game's
// text and art, the bestiary and the statistics. Touch first: a tap opens a card, relic, potion or
// monster; nothing needs hover. The page (`app.js`) hands over its helpers through `initCompendium`.
import { runs, statsBlocks, killerName } from "./history.js";

let ctx = null;
export const initCompendium = (c) => (ctx = c);

// Where the list stands, kept while a detail sheet is open over it.
const S = { tab: "cards", hero: "all", kind: "all", rarity: "all", up: false, scroll: 0 };
const HEROES = [["all", "All"], ["red", "Ironclad"], ["green", "Silent"], ["orange", "Regent"], ["pink", "Necrobinder"], ["blue", "Defect"], ["other", "Other"]];
const KINDS = ["all", "Attack", "Skill", "Power", "Status", "Curse", "Quest"];
const RARITIES = ["all", "Starter", "Common", "Uncommon", "Rare", "Ancient", "Special"];
const TABS = [["cards", "Cards"], ["relics", "Relics"], ["potions", "Potions"], ["bestiary", "Bestiary"], ["stats", "Stats"]];
const heroOf = (face) => (["red", "green", "orange", "pink", "blue"].includes(face.frame) ? face.frame : "other");

let cache = null;
const data = () => {
  if (cache) return cache;
  const [nc, nr, np] = ctx.library.counts();
  cache = { cards: ctx.library.part("cards", 0, nc), relics: ctx.library.part("relics", 0, nr), potions: ctx.library.part("potions", 0, np), monsters: null };
  return cache;
};

let monsters = null;
async function loadMonsters() {
  if (monsters) return monsters;
  try {
    const t = await (await fetch("text/monsters.json")).json();
    const m = new Map();
    for (const [k, v] of Object.entries(t)) {
      const id = /^([A-Z0-9_]+)\.name$/.exec(k)?.[1];
      if (id) m.set(id, { id, name: v.replace(/\[[^\]]*\]/g, ""), moves: [] });
    }
    for (const [k, v] of Object.entries(t)) {
      const x = /^([A-Z0-9_]+)\.moves\.[A-Z0-9_]+\.title$/.exec(k);
      if (x && m.has(x[1])) m.get(x[1]).moves.push(v);
    }
    monsters = [...m.values()].filter((x) => ctx.artImg("monster", x.id)).sort((a, b) => a.name.localeCompare(b.name));
  } catch {
    monsters = [];
  }
  return monsters;
}

const chips = (items, get, set) =>
  ctx.el("div", "cfilter", items.map(([v, label]) => ctx.tap(ctx.el("button", `chipbtn${get() === v ? " on" : ""}`, label), () => { set(v); paint(); })));

let box = null;
const paint = () => {
  const body = document.querySelector("#sheet .sheetbody");
  if (!body || !box) return;
  box.replaceChildren(...view());
  body.scrollTop = 0;
};

// A detail sheet over the list, with a back chip.
const detail = (title, parts) => ({
  title,
  onClose: () => {},
  body: () => [ctx.tap(ctx.el("button", "chipbtn back cdetail", "‹ Compendium"), () => ctx.showLocal(compendiumSheet(true))), ...parts()],
});
const remember = () => (S.scroll = document.querySelector("#sheet .sheetbody")?.scrollTop ?? 0);

// A long list in chunks as the sheet scrolls: the art loads only for what has been reached.
function chunked(items, make, size = 24) {
  const host = ctx.el("div", "cgrid");
  let at = 0;
  const sentinel = ctx.el("div", "csentinel");
  const more = () => {
    for (const it of items.slice(at, at + size)) host.append(make(it));
    at += size;
    for (const img of host.querySelectorAll("img:not([loading])")) img.loading = "lazy";
    if (at >= items.length) sentinel.remove();
  };
  more();
  const out = ctx.el("div", "clist", host, items.length > size ? sentinel : null);
  if (items.length > size) {
    requestAnimationFrame(() => {
      const root = document.querySelector("#sheet .sheetbody");
      const io = new IntersectionObserver((es) => {
        if (es.some((e) => e.isIntersecting)) more();
        if (at >= items.length) io.disconnect();
      }, { root, rootMargin: "600px" });
      io.observe(sentinel);
    });
  }
  return out;
}

function cardsView() {
  const list = data().cards.filter((c) => {
    const f = c.now;
    return (S.hero === "all" || heroOf(f) === S.hero) && (S.kind === "all" || f.kind === S.kind) && (S.rarity === "all" || f.rarity === S.rarity);
  });
  const sw = ctx.el("button", `switch${S.up ? " on" : ""}`, ctx.el("i"));
  sw.setAttribute("role", "switch");
  sw.setAttribute("aria-checked", `${S.up}`);
  ctx.tap(sw, () => { S.up = !S.up; paint(); });
  const grid = chunked(list, (c) => {
    const face = S.up && c.up ? c.up : c.now;
    const b = { id: 0, state: "Enabled", label: [], detail: [], card: face };
    const open = () => { remember(); const sheet = ctx.cardSheet(b); ctx.showLocal(detail(sheet.title, () => [...sheet.body(), upgradedLine(c)])); };
    return ctx.cardTile(b, { onTap: open });
  });
  const filters = () => [chips(HEROES, () => S.hero, (v) => (S.hero = v)), chips(KINDS.map((k) => [k, k === "all" ? "Any type" : k]), () => S.kind, (v) => (S.kind = v)), chips(RARITIES.map((k) => [k, k === "all" ? "Any rarity" : k]), () => S.rarity, (v) => (S.rarity = v))];
  // A phone: one button with the active count opens the filters in a sheet, and the list gets the screen.
  const active = [S.hero, S.kind, S.rarity].filter((v) => v !== "all").length;
  const phoneBar = ctx.tap(ctx.el("button", `chipbtn cfilterbtn${active ? " on" : ""}`, active ? `Filter (${active})` : "Filter"), () => ctx.showLocal(filterSheet()));
  return [
    ctx.el("div", "cfilters", ...filters()),
    ctx.el("div", "cuprow", phoneBar, ctx.el("span", "cup", "Upgraded"), sw, ctx.el("span", "ccount", `${list.length} cards`)),
    list.length ? grid : ctx.el("p", "note", "No cards match."),
  ];
}

// The filters in a sheet over the list (phone); closing returns to the list.
const filterSheet = () => ({
  title: "Filter",
  onClose: () => {},
  body() {
    const chip = (items, get, set) => ctx.el("div", "cfilter", items.map(([v, label]) => ctx.tap(ctx.el("button", `chipbtn${get() === v ? " on" : ""}`, label), () => { set(v); ctx.showLocal(filterSheet()); })));
    return [
      chip(HEROES, () => S.hero, (v) => (S.hero = v)),
      chip(KINDS.map((k) => [k, k === "all" ? "Any type" : k]), () => S.kind, (v) => (S.kind = v)),
      chip(RARITIES.map((k) => [k, k === "all" ? "Any rarity" : k]), () => S.rarity, (v) => (S.rarity = v)),
      ctx.tap(ctx.el("button", "primary wide", "Done"), () => ctx.showLocal(compendiumSheet(true))),
    ];
  },
});

// The card's upgraded face as a second tile under its sheet, when it has one.
function upgradedLine(c) {
  if (!c.up) return ctx.el("p", "note", "This card cannot be upgraded.");
  const b = { id: 0, state: "Enabled", label: [], detail: [], card: c.up };
  return ctx.el("div", "cup", ctx.el("h3", null, "Upgraded"), ctx.el("div", "cards one", ctx.cardTile(b, { onTap: () => {} })));
}

function itemsView(kind, label) {
  const list = data()[kind];
  return [
    ctx.el("p", "note ccount", `${list.length} ${label}`),
    chunked(list, (x) => {
      const pic = ctx.artImg(kind === "relics" ? "relic" : "potion", x.id, "citem-ic");
      const b = ctx.el("button", "citem", pic ?? ctx.el("span", "citem-ic", x.name[0]), ctx.el("span", "citem-t", ctx.el("b", null, x.name), ctx.el("small", null, x.text.map((l) => l.map((s) => s.text).join("")).join(" "))));
      return ctx.tap(b, () => { remember(); ctx.showLocal(detail(x.name, () => [ctx.el("div", "citem-big", ctx.artImg(kind === "relics" ? "relic" : "potion", x.id, "citem-bigic")), ctx.textSheet(x.text)])); });
    }, 30),
  ];
}

function bestiaryView() {
  const host = ctx.el("div", "clist");
  loadMonsters().then((list) => {
    if (!box) return;
    const seen = runs();
    host.replaceChildren(
      ctx.el("p", "note ccount", `${list.length} monsters`),
      chunked(list, (m) => {
        const b = ctx.el("button", "citem", ctx.artImg("monster", m.id, "citem-ic mon"), ctx.el("span", "citem-t", ctx.el("b", null, m.name), ctx.el("small", null, `${m.moves.length} move${m.moves.length === 1 ? "" : "s"}`)));
        return ctx.tap(b, () => {
          remember();
          const fought = seen.filter((r) => (r.path ?? []).some((p) => p.what?.includes(m.name))).length;
          const killed = seen.filter((r) => r.killer && killerName(r.killer)?.includes(m.name)).length;
          ctx.showLocal(detail(m.name, () => [
            ctx.el("div", "citem-big", ctx.artImg("monster", m.id, "citem-bigic mon")),
            ctx.el("p", "note", `Fought in ${fought} of your ${seen.length} runs · killed you ${killed}×`),
            ctx.el("h3", null, "Moves"),
            ctx.el("div", "hdeck", m.moves.map((x) => ctx.el("span", "hcard", x))),
          ]));
        });
      }, 24),
    );
  });
  host.append(ctx.el("p", "note", "Loading…"));
  return [host];
}

function view() {
  if (S.tab === "cards") return cardsView();
  if (S.tab === "relics") return itemsView("relics", "relics");
  if (S.tab === "potions") return itemsView("potions", "potions");
  if (S.tab === "bestiary") return bestiaryView();
  return runs().length ? statsBlocks() : [ctx.el("p", "note", "No finished runs yet. Win or lose one and the numbers appear here.")];
}

export function compendiumSheet(restore = false) {
  return {
    title: "Compendium",
    body() {
      const tabs = ctx.el("div", "ctabs", TABS.map(([k, label]) => ctx.tap(ctx.el("button", `chipbtn${S.tab === k ? " on" : ""}`, label), () => {
        S.tab = k;
        for (const [i, [kk]] of TABS.entries()) tabs.children[i].classList.toggle("on", kk === k);
        paint();
      })));
      box = ctx.el("div", "cbox", ...view());
      if (restore) requestAnimationFrame(() => { const b = document.querySelector("#sheet .sheetbody"); if (b) b.scrollTop = S.scroll; });
      return [tabs, box];
    },
  };
}
