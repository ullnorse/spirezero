// Upgrade preview: a card shown beside its upgraded self with the changes lit. The text is the
// engine's (`Web.upgrade_preview`: the same card faces the page draws everywhere), so the
// numbers and words are the game's. Hold a card (touch) and its sheet shows the pair; hover it or
// right-click it (desktop) for a popover. Nothing here knows a rule.

let ctx = null;
export function initPreview(c) {
  ctx = c;
  const hide = () => hidePopover();
  document.addEventListener("pointerdown", (e) => {
    if (!e.target.closest?.(".uppop")) hide();
  }, { capture: true, passive: true });
  document.addEventListener("keydown", (e) => e.key === "Escape" && hide());
  window.addEventListener("scroll", () => performance.now() - shownAt > 600 && hide(), { passive: true, capture: true });
}

// "Strike+" → 1, "Strike+2" → 2.
const levelOf = (name) => {
  const m = /\+(\d*)$/.exec(name ?? "");
  return m ? Number(m[1] || 1) : 0;
};

// {now, up} faces for a card face, or null when it cannot be upgraded (or the page has no run).
export function upgradePair(face) {
  const game = ctx?.game();
  if (!game || !face?.art) return null;
  // An upgrade selection's face is the card upgraded, with the card now beside it.
  if (face.before) return { now: face.before, up: face };
  try {
    return JSON.parse(game.upgrade_preview(face.art, levelOf(face.name)));
  } catch {
    return null;
  }
}

const tokens = (rich) => {
  const out = [];
  (rich ?? []).forEach((s, si) => {
    for (const w of s.text.split(/(\s+)/)) if (w) out.push({ si, w, space: /^\s+$/.test(w) });
  });
  return out;
};

// Which of `b`'s words are not in the longest common run with `a`.
function changed(a, b) {
  const x = a.filter((t) => !t.space).map((t) => t.w);
  const y = b.filter((t) => !t.space).map((t) => t.w);
  const dp = Array.from({ length: x.length + 1 }, () => new Array(y.length + 1).fill(0));
  for (let i = x.length - 1; i >= 0; i--)
    for (let j = y.length - 1; j >= 0; j--) dp[i][j] = x[i] === y[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const inA = new Set();
  const inB = new Set();
  for (let i = 0, j = 0; i < x.length && j < y.length; ) {
    if (x[i] === y[j]) {
      inA.add(i++);
      inB.add(j++);
    } else if (dp[i + 1][j] >= dp[i][j + 1]) i++;
    else j++;
  }
  return { a: (n) => !inA.has(n), b: (n) => !inB.has(n) };
}

// `rich` drawn with the words `lit` says are changed marked `hl`.
function litRich(rich, lit) {
  const root = ctx.el("span");
  let n = 0;
  for (const t of tokens(rich)) {
    const s = rich[t.si];
    const hot = !t.space && lit(n++);
    root.append(ctx.el("span", `r-${s.role}${s.bold ? " b" : ""}${hot ? ` hl ${lit.cls}` : ""}`, t.w));
  }
  return root;
}

function face(f, lit, costLit) {
  const b = { id: 0, state: "Enabled", label: [], detail: [], card: f };
  const t = ctx.cardTile(b, { onTap: () => {} });
  const text = f.text?.length ? f.text : f.effect;
  const desc = t.querySelector(".g-desc, .ctext");
  if (desc) desc.replaceChildren(litRich(text, lit));
  if (costLit) {
    t.querySelector(".g-cost, .cost")?.classList.add("hl-cost");
    t.querySelector(".g-cost b, .cost.gem b, .cost")?.classList.add("upcost");
  }
  t.classList.add("uppreview");
  t.removeAttribute("data-card");
  return t;
}

// The pair as a block: the card now, an arrow, the card upgraded.
export function pairBlock(pair, cls = "") {
  const diff = changed(tokens(pair.now.text?.length ? pair.now.text : pair.now.effect), tokens(pair.up.text?.length ? pair.up.text : pair.up.effect));
  const old = Object.assign((n) => diff.a(n), { cls: "old" });
  const fresh = Object.assign((n) => diff.b(n), { cls: "new" });
  const costLit = pair.now.cost !== pair.up.cost;
  return ctx.el("div", `uppair ${cls}`, face(pair.now, old, false), ctx.el("span", "uparrow", "→"), face(pair.up, fresh, costLit));
}

// The sheet section for a card face: heading and the pair, or null.
export function upgradeSection(face) {
  const pair = upgradePair(face);
  if (!pair) return null;
  return ctx.el("div", "upsection", ctx.el("h3", null, "Upgrade"), pairBlock(pair));
}

let pop = null;
let shownAt = 0;
let timer = 0;
function hidePopover() {
  clearTimeout(timer);
  pop?.remove();
  pop = null;
}

function showPopover(tile, face) {
  hidePopover();
  const pair = upgradePair(face);
  // The words the card lights (Doom, Block, Ethereal), as the game explains them.
  const terms = ctx.glossary?.([face?.text?.length ? face.text : face?.effect ?? []]);
  if ((!pair && !terms) || !tile.isConnected) return;
  pop = ctx.el("div", "uppop", pair ? pairBlock(pair, "small") : null, terms);
  document.body.append(pop);
  shownAt = performance.now();
  const r = tile.getBoundingClientRect();
  const p = pop.getBoundingClientRect();
  const left = Math.max(8, Math.min(innerWidth - p.width - 8, r.left + r.width / 2 - p.width / 2));
  const top = r.top - p.height - 8 >= 8 ? r.top - p.height - 8 : Math.min(innerHeight - p.height - 8, r.bottom + 8);
  pop.style.left = `${left}px`;
  pop.style.top = `${Math.max(8, top)}px`;
}

// Hover (a mouse, after a beat) and right-click on a card tile show the popover; a finger's
// long-press is the tile's own hold, whose sheet carries the pair.
export function attachPreview(tile, face) {
  if (!face?.art) return;
  const fine = matchMedia("(hover: hover) and (pointer: fine)");
  tile.addEventListener("pointerenter", (e) => {
    if (e.pointerType !== "mouse" || !fine.matches) return;
    clearTimeout(timer);
    timer = setTimeout(() => showPopover(tile, face), 320);
  });
  tile.addEventListener("pointerleave", () => hidePopover());
  tile.addEventListener("contextmenu", (e) => {
    if (e.pointerType === "touch" || !fine.matches) return;
    e.preventDefault();
    showPopover(tile, face);
  });
}
