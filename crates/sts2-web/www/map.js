// The map's route planner: the per-kind room ranges (from sts2-view's `MapKind::range` and
// `MapCell::through`), and highlighting every path through a node. app.js draws the SVG and hands it
// here; nothing in this file knows a game rule.
import { icon, GLYPH_ICON } from "./icons.js";

const dash = "–";
export const rangeText = (r) => (!r || r.min === r.max ? `${r?.max ?? 0}` : `${r.min}${dash}${r.max}`);

const key = (x, y) => `${x},${y}`;

// The nearest scrolling ancestor (the map screen's box, or the sheet's body).
function scroller(from) {
  for (let e = from.parentElement; e; e = e.parentElement) {
    if (/(auto|scroll)/.test(getComputedStyle(e).overflowY) && e.scrollHeight > e.clientHeight + 1) return e;
  }
  return null;
}

// Scrolls the map's box up or down (never sideways) so `node` sits at `f` of the visible height; the
// sheet's sticky counts cover its foot, so they end the visible part. Only moves when it is not
// already comfortably in view, unless `force`.
function bring(svg, node, f, force) {
  const box = scroller(svg);
  if (!box || !node) return;
  const b = box.getBoundingClientRect();
  const planner = box.querySelector(".insheet > .planner");
  const foot = planner ? Math.min(b.bottom, planner.getBoundingClientRect().top) : b.bottom;
  const r = node.getBoundingClientRect();
  const h = foot - b.top;
  if (!force && r.top > b.top + h * 0.15 && r.bottom < b.top + h * 0.85) return;
  const top = box.scrollTop + r.top + r.height / 2 - b.top - h * f;
  const calm = matchMedia("(prefers-reduced-motion: reduce)").matches || document.body.classList.contains("proof");
  box.scrollTo({ top: Math.max(0, top), behavior: calm ? "instant" : "smooth" });
}

// A kind tapped under the map: scroll to its nearest lit room (nearest to where you stand, ahead of
// you first); tapped again, go back to where you are.
function follow(svg, g, kind) {
  const here = g.you?.[1] ?? -1;
  if (!kind) {
    bring(svg, svg.querySelector("g.node.reach") ?? svg.querySelector("g.node.you"), 0.68, true);
    return;
  }
  const lit = [...svg.querySelectorAll(`g.node.kind-${kind}[data-at]`)];
  const row = (n) => Number(n.dataset.at.split(",")[1]);
  const ahead = lit.filter((n) => n.classList.contains("ahead") || n.classList.contains("reach"));
  const pool = ahead.length ? ahead : lit;
  pool.sort((a, b) => Math.abs(row(a) - here) - Math.abs(row(b) - here));
  bring(svg, pool[0], 0.5, false);
}

const plural = (w, n) => (n === 1 || w.endsWith("s") ? w : `${w}s`);

// Every node on a path through `at`, the walkers' nodes included: its ancestors (back to where the
// player stands) and its descendants, over the cells still ahead. `up`/`down` map a node to its
// neighbours along the paths.
function through(at, g, down, up) {
  const ahead = new Set(g.cells.filter((c) => c.ahead || c.visited).map((c) => key(c.x, c.y)));
  if (g.you) ahead.add(key(...g.you));
  const seen = new Set([at]);
  for (const links of [down, up]) {
    const stack = [at];
    while (stack.length) {
      for (const n of links.get(stack.pop()) ?? []) {
        if (!seen.has(n) && (ahead.has(n) || n === key(...g.boss_at))) {
          seen.add(n);
          stack.push(n);
        }
      }
    }
  }
  return seen;
}

// The legend (a chip per room kind with its range over every path to the boss) and a bar above it
// naming what the chips count: all routes, or the paths through the chosen node. Returns the
// element; `svg`'s nodes and edges (`data-at`, `data-e`) are what it highlights.
export function routePlanner(g, svg, ctx) {
  const { el, send, uiArt, MAP_ART, art } = ctx;
  const cells = new Map(g.cells.map((c) => [key(c.x, c.y), c]));
  const down = new Map();
  const up = new Map();
  for (const e of g.edges) {
    const a = key(...e.from);
    const b = key(...e.to);
    down.set(a, [...(down.get(a) ?? []), b]);
    up.set(b, [...(up.get(b) ?? []), a]);
  }
  const kinds = g.kinds ?? [];
  const bar = el("div", "routebar");
  const label = el("span", "routelabel");
  const go = el("button", "routego", "Travel");
  go.hidden = true;
  bar.append(label, go);
  const chips = kinds.map((k, i) => {
    const kind = GLYPH_ICON[k.glyph] ?? "unknown";
    const pic = art && uiArt(MAP_ART[kind] ?? "map_unknown");
    const num = el("b");
    const lg = el("button", `lg r-${k.role}`, pic ? Object.assign(el("img", "lgart"), { src: pic.href, alt: "" }) : icon(kind), num);
    // Hover or tap a kind: every room of it lights up, the rest dim (tap again: all).
    const light = (on) => {
      svg.dataset.light = on ? kind : "";
      for (const x of lg.parentElement?.children ?? []) x.classList.toggle("on", on && x === lg);
    };
    lg.addEventListener("pointerenter", (ev) => ev.pointerType === "mouse" && light(true));
    lg.addEventListener("pointerleave", (ev) => ev.pointerType === "mouse" && !lg.dataset.pinned && light(false));
    lg.addEventListener("click", () => {
      const pinned = svg.dataset.light === kind && lg.dataset.pinned;
      for (const x of lg.parentElement.children) delete x.dataset.pinned;
      if (!pinned) lg.dataset.pinned = "1";
      light(!pinned);
      follow(svg, g, pinned ? null : kind);
    });
    return { lg, num, k, i };
  });
  const legend = el("div", "legend", chips.map((c) => c.lg));

  const show = (cell) => {
    const via = cell?.through?.length ? cell : null;
    let any = false;
    for (const { lg, num, k, i } of chips) {
      const r = via ? via.through[i] : k.range;
      if (r.max) any = true;
      num.textContent = rangeText(r);
      lg.title = `${k.word}: ${r.min === r.max ? r.max : `${r.min} to ${r.max}`} ${via ? "on the paths through the chosen room" : "on the paths to the boss"}`;
    }
    legend.classList.toggle("via", !!via);
    // Nothing to count (the boss is next): the chips go, their space stays.
    legend.style.visibility = any ? "" : "hidden";
  };
  show(null);

  let picked = null;
  const choose = (at) => {
    picked = at;
    const cell = at ? cells.get(at) : null;
    const set = cell?.through?.length ? through(at, g, down, up) : null;
    svg.classList.toggle("routing", !!set);
    for (const n of svg.querySelectorAll("g.node[data-at]")) {
      n.classList.toggle("inroute", !!set?.has(n.dataset.at));
      n.classList.toggle("picked", !!set && n.dataset.at === at);
    }
    for (const l of svg.querySelectorAll("line[data-e]")) {
      const [a, b] = l.dataset.e.split(" ");
      l.classList.toggle("inroute", !!set && set.has(a) && set.has(b));
    }
    show(set ? cell : null);
    // What the chosen room's paths hold, from the engine's counts: "2 elites · 1 rest".
    const held = set ? kinds.flatMap((k, i) => (cell.through[i]?.max > 0 ? [`${rangeText(cell.through[i])} ${plural(k.word, cell.through[i].max)}`] : [])) : [];
    label.textContent = set ? held.join(" · ") || "No rooms ahead" : "";
    go.hidden = !(set && cell.reachable && cell.button);
    go.onclick = () => cell?.button && send({ Tap: Number(cell.button) });
  };
  choose(null);

  const nodeAt = (ev) => ev.target.closest?.("g.node[data-at]");
  // A tap or click on a next room travels. Mouse: hover previews the paths. Touch: a tap on any
  // other room previews its paths (tap it again to let go).
  let pointer = "mouse";
  let pinned = null;
  svg.addEventListener("pointerdown", (ev) => (pointer = ev.pointerType || "mouse"));
  svg.addEventListener("pointerover", (ev) => {
    if (ev.pointerType !== "mouse" || pinned) return;
    const n = nodeAt(ev);
    if (n && cells.get(n.dataset.at)?.through?.length) choose(n.dataset.at);
  });
  svg.addEventListener("pointerleave", (ev) => ev.pointerType === "mouse" && !pinned && choose(null));
  // The next room nearest the tap, within a thumb's reach of its centre: a tap that lands between
  // two close rooms, or on a later room drawn over a next one, still travels.
  const nearNext = (ev) => {
    let best = null;
    let bestD = 40;
    for (const g of svg.querySelectorAll("g.node[data-at], g.node.boss[data-boss]")) {
      const c = g.dataset.boss ? { button: g.dataset.boss } : cells.get(g.dataset.at);
      if (!(c?.reachable || g.dataset.boss) || !c.button) continue;
      const r = g.getBoundingClientRect();
      const d = Math.hypot(ev.clientX - (r.left + r.width / 2), ev.clientY - (r.top + r.height / 2));
      if (d < bestD) [best, bestD] = [c, d];
    }
    return best;
  };
  svg.addEventListener("click", (ev) => {
    // The boss, when it is the next room: its icon travels like any room.
    const boss = ev.target.closest?.("g.node.boss[data-boss]");
    if (boss) {
      send({ Tap: Number(boss.dataset.boss) });
      return;
    }
    const n = nodeAt(ev);
    const at = n?.dataset.at;
    const cell = at && cells.get(at);
    if (!cell?.reachable && pointer !== "mouse") {
      const next = nearNext(ev);
      if (next) {
        send({ Tap: Number(next.button) });
        return;
      }
    }
    if (!cell?.through?.length) {
      // Not a room with paths ahead (the boss, rooms behind, the page): clear, or travel as before.
      if (cell?.reachable && cell.button) send({ Tap: Number(cell.button) });
      else {
        pinned = null;
        choose(null);
      }
      return;
    }
    if (cell.reachable && cell.button) {
      send({ Tap: Number(cell.button) });
      return;
    }
    // A second tap on the previewed room that cannot be travelled to lets go of it.
    pinned = picked === at && pointer !== "mouse" ? null : at;
    choose(pinned);
  });
  return el("div", "planner", bar, legend);
}
