// Draw odds (WEB10): the fight's draw pile sheet says, for each card and card type, the chance it
// is in next turn's draw, and how many cards are left before the reshuffle. It uses only what the
// player knows: the pile's contents (the sheet lists them in a fixed order), never its order.
const S = window.sts2;
// The cards a turn draws (the game's HAND_DRAW; relics and powers that change it are not counted).
const DRAW = 5;
const TYPES = ["Attack", "Skill", "Power", "Status", "Curse"];

function el(tag, cls, ...kids) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null) e.append(k);
  return e;
}

// The chance that at least one of `k` cards is among `d` drawn from `n` without replacement.
export function chance(n, k, d) {
  if (k <= 0) return 0;
  if (d >= n) return 1;
  let none = 1;
  for (let i = 0; i < d; i++) none *= Math.max(0, n - k - i) / (n - i);
  return 1 - none;
}

const pct = (p) => (p >= 1 ? "100%" : p > 0 && p < 0.01 ? "<1%" : `${Math.round(p * 100)}%`);

function oddsView(cards) {
  const n = cards.length;
  const d = Math.min(DRAW, n);
  const box = el("div", "odds");
  if (!n) {
    box.append(el("p", "note", "Empty: next turn reshuffles the discard pile."));
    return box;
  }
  const head = n === DRAW
    ? `Next turn draws all ${n}: the reshuffle comes the turn after.`
    : n < DRAW
    ? `Next turn draws all ${n}, then reshuffles the discard pile for ${DRAW - n} more.`
    : `Next turn draws ${DRAW} of these ${n}. ${n} cards left before the reshuffle (${Math.ceil(n / DRAW)} turns).`;
  box.append(el("p", "note", head));
  const types = el("div", "otypes");
  for (const t of TYPES) {
    const k = cards.filter((c) => c.kind === t).length;
    if (k) types.append(el("span", `ochip k-${t.toLowerCase()}`, `${t} `, el("b", null, pct(chance(n, k, d)))));
  }
  box.append(types);
  const groups = new Map();
  for (const c of cards) groups.set(c.name, { c, k: (groups.get(c.name)?.k ?? 0) + 1 });
  const rows = [...groups.values()].map((g) => ({ ...g, p: chance(n, g.k, d) })).sort((a, b) => b.p - a.p || a.c.name.localeCompare(b.c.name));
  const list = el("ul", "orows");
  for (const { c, k, p } of rows) {
    const bar = el("span", "obar", el("i"));
    bar.firstChild.style.width = `${Math.round(p * 100)}%`;
    list.append(el("li", null, el("span", "oname", c.name, k > 1 ? el("small", null, ` ×${k}`) : null), bar, el("b", null, pct(p))));
  }
  box.append(list);
  return box;
}

S.hook((kind, v) => {
  if (kind !== "sheet" || !v?.fight || v.sheet_kind !== "DrawPile") return;
  const body = document.querySelector("#sheet .sheetbody");
  if (!body || body.querySelector(".odds")) return;
  const [, detail] = Object.entries(v.sheet?.sheet ?? {})[0] ?? [];
  body.prepend(oddsView(detail?.cards ?? []));
});
