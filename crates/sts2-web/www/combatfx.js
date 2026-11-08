// Combat feedback and practice undo (WEB11): exact numbers before a card is committed, floating
// heals, the turn line and the Practice undo. Every number comes from the
// engine (`Web.preview` plays the card on a copy of the fight); nothing here works one out.
// app.js hands in its helpers once with `init`.
let ctx;
export const init = (c) => (ctx = c);

const proof = () => document.body.classList.contains("proof");

// ---- previews ------------------------------------------------------------------------------

let cur = { view: null, card: null, by: new Map(), hover: null };

const selectedCard = (s) =>
  ctx.blocksOf(s, "List").flatMap((l) => l.rows).find((r) => r.card && ctx.stateOf(r) === "Selected")?.card?.card ?? null;

// Called as a fight is drawn: asks the engine what the selected card would do at each enemy it can
// be aimed at (or once, when it takes no target).
export function begin(s, enemies) {
  const card = selectedCard(s);
  cur = { view: ctx.view(), card, by: new Map(), hover: null };
  if (card == null) return;
  const ask = (target) => {
    try {
      return JSON.parse(ctx.game().preview(card, target));
    } catch {
      return null;
    }
  };
  const aimed = enemies.filter((e) => e.target && ctx.stateOf(e.target) !== "Disabled");
  if (aimed.length) for (const e of aimed) cur.by.set(e.id, ask(e.id));
  else {
    // An untargeted card (all enemies, or none) is one play; a card that picks its target by itself
    // (one enemy left) is asked once per living enemy.
    const once = ask(-1);
    if (once) cur.by.set(-1, once);
    else for (const e of enemies) if (e.alive) cur.by.set(e.id, ask(e.id));
  }
}

// The preview for what is being aimed at: the enemy the drag is over, else the first.
const current = () => cur.by.get(cur.hover) ?? cur.by.get(-1) ?? [...cur.by.values()].find(Boolean) ?? null;

// -- on an enemy: the damage the play really deals it, after every modifier: `18 ×3`, then what
// Block takes and what HP loses (`6 blocked · −12 HP`), or `lethal`. Drawn inside the card's corner
// (no row of its own, so nothing moves) and kept off the finger and the lifted card.
export function enemyNumbers(root, e) {
  const p = cur.by.get(e.id) ?? cur.by.get(-1);
  const x = p?.enemies.find((n) => n.id === e.id);
  if (!x?.damage && !x?.dies) return;
  // The damage itself is the HP bar's own −N (or ☠); the badge keeps only what the bar does not say.
  const parts = [];
  if (x.blocked) parts.push(`${x.blocked} blocked`);
  const kids = [];
  if (parts.length) kids.push(ctx.el("span", "pvs", ...parts.map((t) => ctx.el("span", null, t))));
  if (x.dies && p.partial) kids.push(ctx.el("span", "pvl", "lethal so far"));
  else if (p.partial) kids.push(ctx.el("span", "pvs", "+ more"));
  if (!kids.length) return;
  root.append(ctx.el("div", `pvx${x.dies ? " dies" : ""}`, kids));
}

// -- on you: `+5 block`
export function youNumbers(me) {
  const y = current()?.you;
  if (!y) return;
  const parts = [];
  if (y.block_gain) parts.push(`+${y.block_gain} block`);
  if (y.hp_delta) parts.push(`${y.hp_delta > 0 ? "+" : "−"}${Math.abs(y.hp_delta)} HP`);
  if (parts.length) me.append(ctx.el("div", "pvx you", parts.join(" · ")));
}

const hpWords = (i) =>
  i.lethal ? `Lethal −${i.hp_loss} HP` : i.hp_loss ? `−${i.hp_loss} HP → ${i.hp_after}` : `no damage · ${i.hp_after} HP`;

// -- under the enemies, while a card is aimed: what End Turn would then cost
function afterNode() {
  const i = current()?.incoming;
  if (!i) return null;
  const line = ctx.el("div", `after${i.lethal ? " lethal" : i.hp_loss ? "" : " safe"}`, ctx.el("small", null, "End turn after this"), ctx.el("b", null, hpWords(i) + (i.certain ? "" : " ?")));
  return line;
}
export const afterLine = (parent) => {
  const line = afterNode();
  if (line) parent.append(line);
};

// The drag is over an enemy (or none): the line follows it.
export function hover(id) {
  const key = id == null ? null : Number(id);
  if (key === cur.hover) return;
  cur.hover = key;
  const old = document.querySelector("#screen .after");
  const now = afterNode();
  if (old && now) old.replaceWith(now);
}

// -- on End Turn: always the HP it would cost and what is left
export function endHint(end, s) {
  const i = current()?.incoming ?? ctx.blocksOf(s, "Incoming")[0];
  // A selection (Purity's pick) turns the button into Done: the turn's HP cost is not what it does.
  if (!i || (s.primary?.label ?? []).map((x) => x.text).join("").trim() === "Done") return;
  end.append(ctx.el("small", `endhint${i.lethal ? " lethal" : ""}`, i.lethal ? `lethal −${i.hp_loss} HP` : i.hp_loss ? `−${i.hp_loss} HP → ${i.hp_after}` : `safe · ${i.hp_after} HP`));
}

// ---- feedback -------------------------------------------------------------------------------

// Heals float over the target like damage and Block do (app.js floats those).
export function floats() {
  if (ctx.reduced()) return;
  const was = ctx.before();
  if (!was?.fight || !ctx.view().fight) return;
  const up = (now, old, node) => {
    if (old != null && now > old && node) node.append(ctx.el("span", "float heal", `+${now - old}`));
  };
  const you = (v) => ctx.blocksOf(v.screen, "You")[0];
  up(you(ctx.view())?.hp, you(was)?.hp, document.querySelector(".you .me"));
  for (const e of ctx.blocksOf(ctx.view().screen, "Enemy")) {
    const old = ctx.blocksOf(was.screen, "Enemy").find((o) => o.id === e.id);
    up(e.hp, old?.hp, document.querySelector(`.enemy[data-id="${e.id}"]`));
  }
}

// ---- practice -------------------------------------------------------------------------------

// The start screen's switch.
export function practiceOption(opts) {
  const box = ctx.el("button", `switch${opts.practice ? " on" : ""}`);
  box.setAttribute("role", "switch");
  box.setAttribute("aria-checked", `${!!opts.practice}`);
  box.append(ctx.el("i"));
  const row = ctx.el("label", "opt practice", ctx.el("span", null, "Practice", ctx.el("small", null, "Undo a card or potion; saved as practice")), box);
  box.addEventListener("click", (ev) => {
    ev.preventDefault();
    opts.practice = !opts.practice;
    box.classList.toggle("on", opts.practice);
    box.setAttribute("aria-checked", `${opts.practice}`);
  });
  return row;
}

// Undo in the fight's dock: only in a practice run, lit while something can be taken back.
export function undoButton(parent) {
  const v = ctx.view();
  if (!v.practice) return;
  const b = ctx.el("button", "pile undo", ctx.el("b", null, "↶"), ctx.el("small", null, "Undo"));
  b.disabled = !v.undo;
  b.setAttribute("aria-label", "Undo the last card or potion");
  b.addEventListener("click", (ev) => {
    ev.stopPropagation();
    ctx.undo();
  });
  parent.append(b);
}
