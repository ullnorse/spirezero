// Hover tooltips for a mouse (relics, potions, powers, intents, shop wares): the words a tap or hold
// shows in a sheet, in a small box by the thing, kept on the screen. A touch or a pen never sees one.
// Nothing here knows a rule: `make()` returns what the page's own sheet builders return.
let ctx = null;
let box = null;
let timer = 0;
let at = null;

export function initHover(c) {
  ctx = c;
  document.addEventListener("pointerdown", hideTip, { capture: true, passive: true });
  document.addEventListener("keydown", hideTip, { capture: true });
  window.addEventListener("scroll", hideTip, { passive: true, capture: true });
  window.addEventListener("blur", hideTip);
}

export function hideTip() {
  clearTimeout(timer);
  at = null;
  box?.remove();
  box = null;
}

function place(e) {
  const r = e.getBoundingClientRect();
  const w = box.offsetWidth;
  const h = box.offsetHeight;
  const left = Math.max(8, Math.min(innerWidth - w - 8, r.left + r.width / 2 - w / 2));
  const above = r.top - h - 8;
  const top = above >= 8 ? above : Math.max(8, Math.min(innerHeight - h - 8, r.bottom + 8));
  box.style.left = `${left}px`;
  box.style.top = `${top}px`;
}

function show(e, make) {
  const tip = make();
  const nodes = (tip?.nodes ?? []).filter(Boolean);
  if (!e.isConnected || (!tip?.title && !nodes.length)) return;
  box?.remove();
  box = ctx.el("div", "hovertip", tip.title ? ctx.el("b", "hovertitle", tip.title) : null, ...nodes);
  box.style.visibility = "hidden";
  document.body.append(box);
  place(e);
  box.style.visibility = "";
}

// Hover `e` (a mouse, after a beat) for `make()`: `{ title, nodes }`.
export function attachTip(e, make) {
  if (!e) return e;
  e.addEventListener("pointerenter", (ev) => {
    if (ev.pointerType !== "mouse" || !matchMedia("(hover: hover) and (pointer: fine)").matches) return;
    clearTimeout(timer);
    at = e;
    timer = setTimeout(() => at === e && show(e, make), 280);
  });
  e.addEventListener("pointerleave", () => at === e && hideTip());
  return e;
}
