// Helpers for the UNTOUCHED list (realtest.py). A control is keyed by the screen it is on, its kind and, for the fixed ones
// (top bar, dock, sheet buttons, rest and menu rows), its label; a card, reward, ware or enemy by kind only.
const screenOf = () => {
  const st = document.getElementById('start');
  if (st && !st.hidden) return 'start';
  const sh = document.querySelector('#sheet:not([hidden])');
  if (sh) return 'sheet:' + ((sh.querySelector('h1,h2,.title')?.textContent || 'sheet').replace(/\d+/g, '#').split(/[(,:·]/)[0].trim().slice(0, 20));
  const v = window.sts2 && sts2.view();
  return v ? v.decision + (v.fight ? ':fight' : '') : 'none';
};
const FIXED = '#top *, #dock *, #start *, #sheet .close, .chipbtn, .primary, .tile, .segment *, .hintbtn';
const SKIP = /^(s-|r-|sel|aim|kbd|usable|long|word|squeeze|asking|unaffordable|sale|dead|peek|in|out|lift|drag)/;
const kindOf = (e) => {
  const c = e.closest('.card, .enemy, .pet, .pot, .pile, .ware, .showitem, .row, g.node, .iconbtn, .chip, .tile, button, input, [role=button]') || e;
  const cls = typeof c.className === 'string' ? c.className : '';
  let k = c.tagName === 'g' ? 'node' : cls.split(/\s+/).filter((x) => x && !SKIP.test(x)).slice(0, 2).join('.') || c.tagName.toLowerCase();
  if (c.closest('.peekreward')) k = 'reward-tile';
  const fixed = c.matches(FIXED) || (c.matches('.row') && /^(rest|sheet:)/.test(screenOf()));
  const named = fixed && !c.matches('#top .relic, #top .pot, #top .pot *, #top .relic *, .pile');
  const lab = named ? ' ' + (c.getAttribute('aria-label') || c.textContent).trim().replace(/\s+/g, ' ').replace(/\d+/g, '#').slice(0, 24) : '';
  return screenOf() + ' | ' + k + lab;
};
window.__ctlAt = (x, y) => {
  const e = document.elementFromPoint(x, y);
  const c = e && e.closest('button, [data-bid]:not(:disabled), [role=button], input, select, summary, g.node, .card, .enemy, .pet, .pile, .pot, .tile, .chip');
  return c ? kindOf(c) : null;
};
window.__ctlAll = () => {
  const out = new Set();
  for (const e of document.querySelectorAll('button:not(:disabled), [data-bid]:not(:disabled), [role=button], input, select, summary, g.node.reach, .card, .enemy:not(.dead), .pet, .pile, .tile, .chip')) {
    if (e.closest('[hidden]') || e.closest('.dragcard, #banner')) continue;
    if (e.tagName !== 'g' && e.closest('svg')) continue;
    const r = e.getBoundingClientRect();
    if (r.width < 2 || r.height < 2) continue;
    const sh = e.closest('#sheet');
    const open = document.querySelector('#sheet:not([hidden])');
    if (open ? !sh : sh) continue;  // a sheet open covers the screen under it
    out.add(kindOf(e));
  }
  return [...out];
};
