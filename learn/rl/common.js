// Course scaffolding shared by every page: lesson list, progress in localStorage,
// navigation, quizzes, repo-path boxes, a seedable RNG and small canvas charts.
// Plain JS, no build step, no network.
'use strict';

// Theme. Runs before the body exists (common.js is loaded in <head>) so the first paint
// already has the right colours. Stored choice > ?theme= in the URL > OS preference.
const Theme = (() => {
  const KEY = 'spirebot-rl-theme';
  const mq = window.matchMedia ? window.matchMedia('(prefers-color-scheme: dark)') : null;
  let stored = null;
  try { stored = localStorage.getItem(KEY); } catch (e) { /* private mode */ }
  const fromUrl = (new URLSearchParams(location.search).get('theme') || '').toLowerCase();
  function osTheme() { return mq && !mq.matches ? 'light' : 'dark'; }
  function current() { return document.documentElement.dataset.theme || 'dark'; }
  function apply(t, persist) {
    if (t !== 'light' && t !== 'dark') t = osTheme();
    document.documentElement.dataset.theme = t;
    if (persist) { stored = t; try { localStorage.setItem(KEY, t); } catch (e) { /* ignore */ } }
    document.querySelectorAll('.theme-btn').forEach(b => label(b));
    window.dispatchEvent(new CustomEvent('themechange', { detail: { theme: t } }));
  }
  function label(b) { const dark = current() === 'dark'; b.textContent = dark ? '☀' : '☾'; b.title = dark ? 'Switch to light mode' : 'Switch to dark mode'; b.setAttribute('aria-label', b.title); }
  function toggle() { apply(current() === 'dark' ? 'light' : 'dark', true); }
  // A ☀/☾ button; every page puts one at the top right.
  function button() {
    const b = document.createElement('button');
    b.className = 'theme-btn'; b.type = 'button';
    b.addEventListener('click', toggle);
    label(b);
    return b;
  }
  apply(stored || fromUrl || osTheme(), false);
  if (mq && mq.addEventListener) mq.addEventListener('change', () => { if (!stored) apply(osTheme(), false); });
  return { current, apply, toggle, button };
})();

const Course = (() => {
  // The syllabus: four weeks of five days. A day with a `file` has a page; a day with an
  // `exercise` has a Python file under learn/rl/exercises with TODOs, tests and a solution.
  const SYLLABUS = [
    { week: 1, title: 'The loop, and what a fight looks like to a program', days: [
      { id: '01-loop',     file: '01-loop.html',     title: 'The loop',                         blurb: 'Agent, environment, state, action, reward. Play the mini fight by hand, then let a random agent play it. Exercise: a random legal agent on the real env, reading the mask bits.', exercise: 'week1/day1_loop.py' },
      { id: '02-see-do',   file: '02-see-do.html',   title: 'What the agent sees and may do',   blurb: 'Observations and action masks, from the mini fight to SpireZero’s 972 bytes and 140 indices. Exercise: decode the header, hand and intents; a cards-first rule that wins 60%.', exercise: 'week1/day2_observe.py' },
      { id: '10-returns',  file: '10-returns.html',  title: 'Rewards, returns, discount',        blurb: 'The outcome is copied back onto every step; γ makes fast wins worth more; shaping changes the objective. Exercise: returns, splitting a batch rollout into episodes, an HP-shaped reward.', exercise: 'week1/day3_returns.py' },
      { id: '03-bandit',   file: '03-bandit.html',   title: 'Trial and error',                  blurb: 'Explore against exploit with a bandit over opening lines. Exercise: ε-greedy and UCB on simulated arms, then a bandit over five fixed strategies on real fights.', exercise: 'week1/day4_bandit.py' },
      { id: '04-value',    file: '04-value.html',    title: 'How good is this situation?',      blurb: 'Value as expected return, estimated by rolling fights out; value per action. Exercise: V and Q by rollout on the mini fight, then V of every Act 1 encounter.', exercise: 'week1/day5_value.py' },
    ] },
    { week: 2, title: 'Learning: tables, then nets, then PPO on the real env', days: [
      { id: '05-qlearn',   file: '05-qlearn.html',   title: 'Learning from experience',         blurb: 'Tabular Q-learning trains live in the browser. Exercise: the same agent in Python with three state keys; the full key beats the hand-written rule.', exercise: 'week2/day6_qlearn.py' },
      { id: '11-td',       file: '11-td.html',       title: 'Bootstrapping',                    blurb: 'TD against Monte Carlo: bias against variance, n-step targets. Exercise: both estimators on the mini fight, then a last table on the real env over action classes.', exercise: 'week2/day7_td.py' },
      { id: '06-nets',     file: '06-nets.html',     title: 'Why tables break',                 blurb: 'State spaces explode; nets generalize. Exercise: featurise the real observation, fit a torch value net to fight outcomes, beat the table on held-out fights.', exercise: 'week2/day8_nets.py' },
      { id: '07-policy',   file: '07-policy.html',   title: 'Policy gradients and PPO',         blurb: 'Push up what worked; the advantage; why PPO clips. Exercise: REINFORCE with a masked categorical on the mini fight.', exercise: 'week2/day9_reinforce.py' },
      { id: '08-spirezero', file: '08-spirezero.html', title: 'Training on real StS2',            blurb: 'Features, the mask in the logits, reward design, curriculum and compute. Exercise: GAE with done cuts, the clipped loss, and a PPO run that passes 0.7 win rate in a minute.', exercise: 'week2/day10_ppo.py' },
    ] },
    { week: 3, title: 'Experiments on the real env', days: [
      { id: '09-first',     file: '09-first.html',     title: 'Your first experiment',           blurb: 'Baselines you trust, the smallest PPO, what to log, what the first failures look like. Exercise: a fixed-seed greedy evaluation, checkpoints, and a run of day 10\u2019s trainer that logs the table of this page.', exercise: 'week3/day11_experiment.py' },
      { id: '12-baseline',  file: '12-baseline.html',  title: 'Why REINFORCE stalled',           blurb: 'Day 9\u2019s agent never learned the lethal block: variance hides a rare state\u2019s small advantage, and a collapsing policy stops visiting it. A learned baseline and an entropy bonus. Exercise: actor-critic on the mini fight; P(Defend | lethal hit) above one half.', exercise: 'week3/day12_baseline.py' },
      { id: '13-eval',      file: '13-eval.html',      title: 'An evaluation protocol',          blurb: 'Fixed seeds, greedy actions, a fixed number of fights, standard errors, per-encounter tables, and how many fights it takes to tell two policies apart. Exercise: the protocol as a function, random against cards first against day 10\u2019s net on the same starts.', exercise: 'week3/day13_eval.py' },
      { id: '14-features',  file: '14-features.html',  title: 'Features that see the hand',      blurb: 'Card embeddings instead of a multi-hot, a policy head aligned with the hand slots so index slot \u00d7 8 + target scores that slot\u2019s card. Exercise: the slot net; a permutation test that proves the alignment; PPO with it against day 10 on the same seeds.', exercise: 'week3/day14_features.py' },
      { id: '15-shaping',   file: '15-shaping.html',   title: 'Shaping on the real env',         blurb: 'Sparse, HP-loss, HP-on-win and potential-based rewards trained on the same seeds and judged by the protocol, not by the reward. Exercise: the four reward functions, the telescoping check, and the comparison that shows which one stops killing.', exercise: 'week3/day15_shaping.py' },
    ] },
    { week: 4, title: 'Whole runs', days: [
      { id: '16-runenv',    file: '16-runenv.html',    title: 'The run env',                      blurb: 'Sts2RunEnvs: a run as one episode, the phase byte, indices after 140, fights settled inside the env with combat="greedy". Exercise: decode the run header, random and rule-based run agents on fixed seeds, measured by final floor.', exercise: 'week4/day16_runenv.py' },
      { id: '17-runppo',    file: '17-runppo.html',    title: 'PPO over a run',                   blurb: 'Day 10\u2019s loop over run-level decisions: run features, a net over the whole space, long sparse episodes, floor as a shaping to compare against. Exercise: the run actor-critic and a short training run that climbs past the random agent\u2019s floor.', exercise: 'week4/day17_runppo.py' },
      { id: '18-debug',     file: '18-debug.html',     title: 'Debugging training',               blurb: 'The signatures of the common bugs in the logged numbers, a sweep over learning rate and clip on fixed seeds, picking by the protocol. Exercise: a rule-based diagnosis of histories and a tiny sweep.', exercise: 'week4/day18_debug.py' },
      { id: '19-engine',    file: '19-engine.html',    title: 'The engine side',                  blurb: 'What the mask promises and what the trainer may assume: envs.rs step and reset, action.rs, invariants.rs, StepStats, threads. Exercise: check the promises from Python over thousands of steps, and measure steps/s against threads to size a batch.', exercise: 'week4/day19_engine.py' },
      { id: '20-capstone',  file: '20-capstone.html',  title: 'Capstone',                         blurb: 'One script: train the slot net with PPO on real fights for a time budget, evaluate by the protocol against random and cards first, write the per-encounter table and a report. Minutes on an RTX 4080 Super.', exercise: 'week4/day20_capstone.py' },
    ] },
  ];
  let dayNo = 0;
  const DAYS = SYLLABUS.flatMap(w => w.days.map(d => Object.assign({ week: w.week, day: ++dayNo }, d)));
  const LESSONS = DAYS.filter(d => d.file);
  const KEY = 'spirebot-rl-progress-v1';

  function load() {
    try {
      const p = JSON.parse(localStorage.getItem(KEY) || '{}');
      if (p['08-spirebot'] && !p['08-spirezero']) p['08-spirezero'] = p['08-spirebot']; // lesson renamed
      return p;
    } catch (e) { return {}; }
  }
  function save(p) { try { localStorage.setItem(KEY, JSON.stringify(p)); } catch (e) { /* private mode */ } }
  function mark(id, patch) {
    const p = load();
    p[id] = Object.assign(p[id] || {}, patch);
    save(p);
    return p;
  }
  function reset() { try { localStorage.removeItem(KEY); } catch (e) { /* ignore */ } }

  function el(tag, attrs, children) {
    const e = document.createElement(tag);
    if (attrs) for (const k in attrs) {
      if (k === 'class') e.className = attrs[k];
      else if (k === 'html') e.innerHTML = attrs[k];
      else if (k.startsWith('on')) e.addEventListener(k.slice(2), attrs[k]);
      else e.setAttribute(k, attrs[k]);
    }
    (children || []).forEach(c => e.appendChild(typeof c === 'string' ? document.createTextNode(c) : c));
    return e;
  }

  // Builds the top crumb bar and the previous/next pager for a lesson page.
  function init() {
    const id = document.body.dataset.lesson;
    if (!id) return;
    const i = LESSONS.findIndex(l => l.id === id);
    const L = LESSONS[i];
    const main = document.querySelector('main');
    const top = el('div', { class: 'topnav' }, [
      el('div', { class: 'crumb' }, [el('a', { href: 'index.html' }, ['Learning RL with SpireZero']), ' / ', `Week ${L.week} · Day ${L.day}`]),
      el('div', { class: 'right' }, [el('a', { href: 'index.html' }, ['syllabus']), Theme.button()]),
    ]);
    main.insertBefore(top, main.firstChild);
    if (L.exercise) {
      const test = L.exercise.replace(/day(\d+)_\w+\.py$/, 'test_day$1.py');
      main.appendChild(el('div', { class: 'box exercise' }, [
        el('div', { class: 'kicker' }, ['Today’s exercise']),
        el('p', {}, [el('span', { class: 'path' }, [el('code', {}, [`learn/rl/exercises/${L.exercise}`])]), ': fill in the TODOs; each docstring is the contract the tests check. Solutions are under ', el('code', {}, ['exercises/solutions/']), '.']),
        el('pre', {}, [el('code', {}, [`.venv/bin/python -m pytest learn/rl/exercises/${test}\n.venv/bin/python learn/rl/exercises/${L.exercise}`])]),
      ]));
    }
    const pager = el('div', { class: 'pager' });
    const label = d => `Day ${d.day}. ${d.title}`;
    if (i > 0) pager.appendChild(el('a', { href: LESSONS[i - 1].file }, [el('span', { class: 'lbl' }, ['Previous']), label(LESSONS[i - 1])]));
    else pager.appendChild(el('a', { href: 'index.html' }, [el('span', { class: 'lbl' }, ['Start']), 'Syllabus']));
    if (i < LESSONS.length - 1) pager.appendChild(el('a', { href: LESSONS[i + 1].file, class: 'next' }, [el('span', { class: 'lbl' }, ['Next']), label(LESSONS[i + 1])]));
    else pager.appendChild(el('a', { href: 'index.html', class: 'next' }, [el('span', { class: 'lbl' }, ['Next']), 'Weeks 3 and 4 in the syllabus']));
    main.appendChild(pager);
    mark(id, { visited: Date.now() });
    document.title = `Day ${L.day}. ${L.title} · Learning RL with SpireZero`;
    document.querySelectorAll('.path').forEach(p => {
      const code = p.querySelector('code');
      if (!code) return;
      const b = el('button', { title: 'copy path', onclick: () => { if (navigator.clipboard) navigator.clipboard.writeText(code.textContent).then(() => { b.textContent = 'copied'; setTimeout(() => b.textContent = 'copy', 900); }, () => { b.textContent = 'select it'; }); } }, ['copy']);
      p.appendChild(b);
    });
  }

  // A 2-3 question multiple-choice check. All right => the lesson is marked passed.
  function quiz(selector, questions) {
    const root = document.querySelector(selector);
    root.classList.add('quiz');
    root.appendChild(el('div', { class: 'kicker' }, ['Check yourself']));
    const right = new Array(questions.length).fill(false);
    const status = el('div', { class: 'done' });
    questions.forEach((q, qi) => {
      const why = el('div', { class: 'why' });
      const choices = el('div', { class: 'choices' });
      q.choices.forEach((c, ci) => {
        const b = el('button', {}, [c]);
        b.addEventListener('click', () => {
          choices.querySelectorAll('button').forEach(x => x.classList.remove('right', 'wrong'));
          const ok = ci === q.answer;
          b.classList.add(ok ? 'right' : 'wrong');
          why.textContent = ok ? ('Right. ' + q.why) : ('Not quite. ' + (q.hint || 'Try again.'));
          right[qi] = ok;
          if (right.every(Boolean)) {
            status.textContent = 'All correct. This lesson is marked done.';
            mark(document.body.dataset.lesson, { passed: Date.now() });
          }
        });
        choices.appendChild(b);
      });
      root.appendChild(el('div', { class: 'q' }, [el('div', { class: 'qt' }, [`${qi + 1}. ${q.q}`]), choices, why]));
    });
    root.appendChild(status);
  }

  // SplitMix32-style seedable RNG (deterministic toys). Returns a function in [0, 1).
  function rng(seed) {
    let s = (seed >>> 0) || 0x9e3779b9;
    return function () {
      s = (s + 0x9e3779b9) >>> 0;
      let z = s;
      z = Math.imul(z ^ (z >>> 16), 0x85ebca6b) >>> 0;
      z = Math.imul(z ^ (z >>> 13), 0xc2b2ae35) >>> 0;
      z = (z ^ (z >>> 16)) >>> 0;
      return z / 4294967296;
    };
  }

  // Canvas helpers. Each resizes the backing store to the CSS width and devicePixelRatio.
  function ctx2d(canvas, h) {
    const w = canvas.clientWidth || 600;
    const dpr = window.devicePixelRatio || 1;
    if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
      canvas.style.height = h + 'px';
    }
    const c = canvas.getContext('2d');
    c.setTransform(dpr, 0, 0, dpr, 0, 0);
    c.clearRect(0, 0, w, h);
    return { c, w, h };
  }
  // Colours come from the CSS variables so charts follow the theme. Pass '--name' anywhere a
  // chart takes a colour; literal colours still work. Cached per theme.
  let colorCache = {}, colorTheme = null;
  function color(name) {
    if (!name || typeof name !== 'string' || name[0] !== '-') return name;
    const t = Theme.current();
    if (t !== colorTheme) { colorCache = {}; colorTheme = t; }
    if (!(name in colorCache)) colorCache[name] = getComputedStyle(document.documentElement).getPropertyValue(name).trim() || '#888';
    return colorCache[name];
  }
  const PALETTE = ['--c1', '--c2', '--c3', '--c4', '--c5', '--c6'];
  // Every chart remembers how it was last drawn, so a theme change can repaint all canvases.
  const drawn = new Map();
  function remember(canvas, fn, args) { drawn.set(canvas, () => fn.apply(null, args)); }
  window.addEventListener('themechange', () => { drawn.forEach((f, cv) => { if (cv.isConnected) f(); else drawn.delete(cv); }); });

  // series: [{ name, data: [y...] or [[x,y]...], color }]
  function lineChart(canvas, series, o) {
    o = o || {};
    remember(canvas, lineChart, [canvas, series, o]);
    const H = o.height || 220;
    const { c, w, h } = ctx2d(canvas, H);
    const pad = { l: 44, r: 12, t: 12, b: 28 };
    let xmin = Infinity, xmax = -Infinity, ymin = o.ymin ?? Infinity, ymax = o.ymax ?? -Infinity;
    const pts = series.map(s => s.data.map((d, i) => Array.isArray(d) ? d : [i, d]));
    pts.forEach(ps => ps.forEach(([x, y]) => { xmin = Math.min(xmin, x); xmax = Math.max(xmax, x); if (o.ymin == null) ymin = Math.min(ymin, y); if (o.ymax == null) ymax = Math.max(ymax, y); }));
    if (!isFinite(xmin)) { xmin = 0; xmax = 1; }
    if (!isFinite(ymin)) { ymin = 0; ymax = 1; }
    if (xmax === xmin) xmax = xmin + 1;
    if (ymax === ymin) ymax = ymin + 1;
    const X = x => pad.l + (x - xmin) / (xmax - xmin) * (w - pad.l - pad.r);
    const Y = y => h - pad.b - (y - ymin) / (ymax - ymin) * (h - pad.t - pad.b);
    c.strokeStyle = color('--line'); c.fillStyle = color('--fg3'); c.font = '11px system-ui'; c.lineWidth = 1;
    const ticks = o.yticks || 4;
    for (let i = 0; i <= ticks; i++) {
      const y = ymin + (ymax - ymin) * i / ticks;
      c.beginPath(); c.moveTo(pad.l, Y(y)); c.lineTo(w - pad.r, Y(y)); c.stroke();
      c.textAlign = 'right'; c.fillText(o.yfmt ? o.yfmt(y) : y.toFixed(ymax - ymin < 2 ? 2 : 0), pad.l - 6, Y(y) + 4);
    }
    c.textAlign = 'center';
    for (let i = 0; i <= 4; i++) { const x = xmin + (xmax - xmin) * i / 4; c.fillText(o.xfmt ? o.xfmt(x) : Math.round(x), X(x), h - 8); }
    if (o.xlabel) { c.textAlign = 'right'; c.fillText(o.xlabel, w - pad.r, h - 8); }
    (o.hlines || []).forEach(hl => { c.strokeStyle = color(hl.color || '--fg3'); c.setLineDash([4, 4]); c.beginPath(); c.moveTo(pad.l, Y(hl.y)); c.lineTo(w - pad.r, Y(hl.y)); c.stroke(); c.setLineDash([]); c.fillStyle = color(hl.color || '--fg3'); c.textAlign = 'left'; c.fillText(hl.label || '', pad.l + 4, Y(hl.y) - 4); });
    series.forEach((s, si) => {
      const ps = pts[si]; if (!ps.length) return;
      c.strokeStyle = color(s.color || PALETTE[si % PALETTE.length]); c.lineWidth = s.width || 2; c.beginPath();
      ps.forEach(([x, y], i) => { if (i === 0) c.moveTo(X(x), Y(y)); else c.lineTo(X(x), Y(y)); });
      c.stroke();
      if (s.dots) { c.fillStyle = c.strokeStyle; ps.forEach(([x, y]) => { c.beginPath(); c.arc(X(x), Y(y), 3, 0, 7); c.fill(); }); }
    });
    // legend
    let lx = pad.l + 6;
    c.font = '12px system-ui'; c.textAlign = 'left';
    series.forEach((s, si) => { if (!s.name) return; c.fillStyle = color(s.color || PALETTE[si % PALETTE.length]); c.fillRect(lx, pad.t, 12, 3); c.fillStyle = color('--fg2'); c.fillText(s.name, lx + 16, pad.t + 5); lx += 16 + c.measureText(s.name).width + 14; });
    return { X, Y };
  }

  // Horizontal bars: values[], labels[], o.max, o.fmt, o.colors
  function bars(canvas, values, labels, o) {
    o = o || {};
    remember(canvas, bars, [canvas, values, labels, o]);
    const rowH = o.rowH || 26;
    const { c, w } = ctx2d(canvas, values.length * rowH + 10);
    const max = o.max ?? Math.max(1e-9, ...values.map(Math.abs));
    const lw = o.labelWidth || 120;
    const zero = o.signed ? lw + (w - lw - 60) / 2 : lw;
    const span = o.signed ? (w - lw - 60) / 2 : (w - lw - 60);
    c.font = '12px system-ui';
    values.forEach((v, i) => {
      const y = 5 + i * rowH;
      c.fillStyle = color('--fg2'); c.textAlign = 'right'; c.fillText(labels[i], lw - 8, y + rowH / 2 + 4);
      const len = Math.min(1, Math.abs(v) / max) * span;
      c.fillStyle = color((o.colors && o.colors[i]) || (v < 0 ? '--bad' : PALETTE[i % PALETTE.length]));
      if (v >= 0) c.fillRect(zero, y + 4, len, rowH - 8); else c.fillRect(zero - len, y + 4, len, rowH - 8);
      c.fillStyle = color('--fg'); c.textAlign = 'left'; c.fillText(o.fmt ? o.fmt(v) : v.toFixed(2), (v >= 0 ? zero + len : zero) + 6, y + rowH / 2 + 4);
      if (o.signed) { c.strokeStyle = color('--line'); c.beginPath(); c.moveTo(zero, y); c.lineTo(zero, y + rowH); c.stroke(); }
    });
  }

  // Heat map: grid[row][col] numbers (null = empty), o.min, o.max, o.rows (labels), o.cols, o.fmt (cell text), o.cell
  function heatmap(canvas, grid, o) {
    o = o || {};
    remember(canvas, heatmap, [canvas, grid, o]);
    const rows = grid.length, cols = grid[0].length;
    const lw = o.labelWidth || 70, th = 22;
    const avail = (canvas.clientWidth || 600) - lw - 4;
    const cell = Math.min(o.cell || 34, avail / cols);
    const { c } = ctx2d(canvas, th + rows * cell + 6);
    let min = o.min ?? Infinity, max = o.max ?? -Infinity;
    if (o.min == null || o.max == null) grid.forEach(r => r.forEach(v => { if (v == null) return; if (o.min == null) min = Math.min(min, v); if (o.max == null) max = Math.max(max, v); }));
    if (!isFinite(min)) { min = 0; max = 1; }
    if (max === min) max = min + 1;
    const color = o.color || ((t) => { // blue-grey to warm
      const a = [40, 60, 110], b = [255, 200, 90];
      const m = t < 0.5 ? [64, 110, 170] : null;
      const lerp = (p, q, u) => Math.round(p + (q - p) * u);
      if (t < 0.5) { const u = t / 0.5; return `rgb(${lerp(a[0], 64, u)},${lerp(a[1], 150, u)},${lerp(a[2], 200, u)})`; }
      const u = (t - 0.5) / 0.5; return `rgb(${lerp(64, b[0], u)},${lerp(150, b[1], u)},${lerp(200, b[2], u)})`;
    });
    c.font = '11px system-ui'; c.fillStyle = color('--fg3'); c.textAlign = 'center';
    (o.cols || []).forEach((l, j) => c.fillText(String(l), lw + j * cell + cell / 2, 14));
    c.textAlign = 'right';
    (o.rows || []).forEach((l, i) => { c.fillStyle = color('--fg3'); c.fillText(String(l), lw - 6, th + i * cell + cell / 2 + 4); });
    for (let i = 0; i < rows; i++) for (let j = 0; j < cols; j++) {
      const v = grid[i][j];
      const x = lw + j * cell, y = th + i * cell;
      if (v == null) { c.fillStyle = color('--bg3'); c.fillRect(x + 1, y + 1, cell - 2, cell - 2); continue; }
      c.fillStyle = color(Math.max(0, Math.min(1, (v - min) / (max - min))));
      c.fillRect(x + 1, y + 1, cell - 2, cell - 2);
      if (o.fmt && cell >= 24) { c.fillStyle = '#0f1115'; c.textAlign = 'center'; c.font = '10px ui-monospace, monospace'; c.fillText(o.fmt(v), x + cell / 2, y + cell / 2 + 3); c.font = '11px system-ui'; }
    }
    if (o.title) { c.fillStyle = color('--fg2'); c.textAlign = 'left'; c.fillText(o.title, 2, 14); }
  }

  // Runs fn in slices of `ms` per animation frame until it returns false or stop() is called.
  // Frames come from requestAnimationFrame, with a timer fallback for contexts where it
  // does not fire (headless runs, some embedded views).
  function runLoop(fn, ms) {
    let on = true;
    function schedule() {
      let fired = false;
      const go = () => { if (fired) return; fired = true; cancelAnimationFrame(raf); clearTimeout(to); frame(); };
      const raf = requestAnimationFrame(go), to = setTimeout(go, 50);
    }
    function frame() {
      if (!on) return;
      const t0 = performance.now();
      let go = true, n = 0;
      while (go && performance.now() - t0 < (ms || 12) && n++ < 200) go = fn() !== false;
      if (go) schedule(); else on = false;
    }
    schedule();
    return { stop: () => { on = false; }, get running() { return on; } };
  }

  function pct(x) { return (100 * x).toFixed(1) + '%'; }
  function fmtInt(n) { return Math.round(n).toLocaleString('en-US'); }

  return { SYLLABUS, DAYS, LESSONS, load, save, mark, reset, init, quiz, rng, el, lineChart, bars, heatmap, runLoop, pct, fmtInt, PALETTE, color, theme: Theme };
})();

window.addEventListener('error', e => {
  // Surfaces script errors on the page itself so a broken toy never fails silently.
  let box = document.getElementById('js-errors');
  if (!box) { box = document.createElement('pre'); box.id = 'js-errors'; box.style.cssText = 'color:var(--bad);border:1px solid var(--bad);'; document.body.appendChild(box); }
  box.textContent += (e.message || String(e)) + ' @ ' + (e.filename || '') + ':' + (e.lineno || '') + '\n';
});
