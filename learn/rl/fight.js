// The mini fight every toy in the course is built on. StS-flavoured, not StS-exact:
// a Strike/Defend deck against a Jaw Worm-like enemy with three intents. The real
// engine is crates/sts2-core; this is a few dozen lines so the ideas stay visible.
'use strict';

const Fight = (() => {
  const CFG = {
    playerHp: 28, enemyHp: 48, energy: 3, handSize: 5,
    strikes: 5, defends: 5, strikeDmg: 6, defendBlock: 5,
    moves: {
      chomp:  { name: 'Chomp',  dmg: 12 },
      thrash: { name: 'Thrash', dmg: 8, block: 6 },
      bellow: { name: 'Bellow', str: 4, block: 8 },
    },
    weights: { bellow: 0.45, thrash: 0.30, chomp: 0.25 },
  };
  const ACTIONS = ['Strike', 'Defend', 'End turn'];
  const STRIKE = 0, DEFEND = 1, END = 2;

  function drawHand(s, rand) {
    s.strikes = 0; s.defends = 0;
    for (let i = 0; i < CFG.handSize; i++) {
      if (s.drawS + s.drawD === 0) { s.drawS = s.discS; s.drawD = s.discD; s.discS = 0; s.discD = 0; }
      if (s.drawS + s.drawD === 0) break;
      if (rand() * (s.drawS + s.drawD) < s.drawS) { s.drawS--; s.strikes++; } else { s.drawD--; s.defends++; }
    }
  }

  function newFight(rand) {
    const s = {
      playerHp: CFG.playerHp, maxHp: CFG.playerHp, block: 0, energy: CFG.energy,
      enemyHp: CFG.enemyHp, enemyMaxHp: CFG.enemyHp, enemyBlock: 0, str: 0,
      intent: 'chomp', lastIntent: null, turn: 1,
      strikes: 0, defends: 0, drawS: CFG.strikes, drawD: CFG.defends, discS: 0, discD: 0,
      done: false, won: false, steps: 0,
    };
    drawHand(s, rand);
    return s;
  }
  function clone(s) { return Object.assign({}, s); }

  // Damage the enemy would do this turn, before the player's block.
  function incoming(s) { const m = CFG.moves[s.intent]; return m.dmg ? m.dmg + s.str : 0; }

  function legal(s) {
    if (s.done) return [false, false, false];
    return [s.strikes > 0 && s.energy > 0, s.defends > 0 && s.energy > 0, true];
  }

  function pickIntent(s, rand) {
    // Weighted roll; Chomp and Bellow never twice in a row (Jaw Worm-like).
    for (let tries = 0; tries < 10; tries++) {
      const r = rand();
      const k = r < CFG.weights.bellow ? 'bellow' : r < CFG.weights.bellow + CFG.weights.thrash ? 'thrash' : 'chomp';
      if (k !== 'thrash' && k === s.lastIntent) continue;
      return k;
    }
    return 'thrash';
  }

  // Applies action `a` to `s` in place. Returns { reward, done, events }.
  // Reward is SpireZero's default: +1 win, -1 loss, 0 otherwise.
  function step(s, a, rand) {
    const ev = [];
    if (s.done) return { reward: 0, done: true, events: ev };
    s.steps++;
    if (a === STRIKE) {
      s.strikes--; s.energy--; s.discS++;
      const absorbed = Math.min(s.enemyBlock, CFG.strikeDmg);
      s.enemyBlock -= absorbed;
      const dmg = CFG.strikeDmg - absorbed;
      s.enemyHp = Math.max(0, s.enemyHp - dmg);
      ev.push(`Strike: ${dmg} damage` + (absorbed ? ` (${absorbed} blocked)` : ''));
      if (s.enemyHp === 0) { s.done = true; s.won = true; ev.push('Enemy dies. You win.'); return { reward: 1, done: true, events: ev }; }
      return { reward: 0, done: false, events: ev };
    }
    if (a === DEFEND) {
      s.defends--; s.energy--; s.discD++; s.block += CFG.defendBlock;
      ev.push(`Defend: +${CFG.defendBlock} block (${s.block})`);
      return { reward: 0, done: false, events: ev };
    }
    // End turn: discard hand, enemy acts, next turn.
    s.discS += s.strikes; s.discD += s.defends; s.strikes = 0; s.defends = 0;
    const m = CFG.moves[s.intent];
    s.enemyBlock = 0;
    if (m.dmg) {
      const raw = m.dmg + s.str;
      const absorbed = Math.min(s.block, raw);
      const dmg = raw - absorbed;
      s.block -= absorbed;
      s.playerHp = Math.max(0, s.playerHp - dmg);
      ev.push(`${m.name}: ${dmg} damage to you` + (absorbed ? ` (${absorbed} blocked)` : ''));
    }
    if (m.block) { s.enemyBlock = m.block; ev.push(`${m.name}: enemy gains ${m.block} block`); }
    if (m.str) { s.str += m.str; ev.push(`${m.name}: enemy gains ${m.str} Strength (${s.str})`); }
    if (s.playerHp === 0) { s.done = true; s.won = false; ev.push('You die.'); return { reward: -1, done: true, events: ev }; }
    s.lastIntent = s.intent; s.intent = pickIntent(s, rand);
    s.turn++; s.block = 0; s.energy = CFG.energy;
    drawHand(s, rand);
    ev.push(`Turn ${s.turn}: drew ${s.strikes} Strike, ${s.defends} Defend. Enemy intends ${intentText(s)}.`);
    return { reward: 0, done: false, events: ev };
  }

  function intentText(s) {
    const m = CFG.moves[s.intent];
    const parts = [];
    if (m.dmg) parts.push(`attack for ${m.dmg + s.str}`);
    if (m.block) parts.push(`block ${m.block}`);
    if (m.str) parts.push(`+${m.str} Strength`);
    return `${m.name} (${parts.join(', ')})`;
  }

  // Plays one whole fight with `policy(state, legalMask, rand) -> action`. Returns the final state.
  function play(policy, rand, onStep) {
    const s = newFight(rand);
    while (!s.done && s.steps < 400) {
      const a = policy(s, legal(s), rand);
      const r = step(s, a, rand);
      if (onStep) onStep(s, a, r);
    }
    return s;
  }

  // ---- policies -------------------------------------------------------------
  function randomPolicy(s, m, rand) {
    const idx = []; m.forEach((ok, i) => { if (ok) idx.push(i); });
    return idx[Math.floor(rand() * idx.length)];
  }
  // A hand-written rule: block if the hit would otherwise hurt, otherwise attack.
  function heuristicPolicy(s, m) {
    const inc = Math.max(0, incoming(s) - s.block);
    if (m[STRIKE] && s.enemyHp - s.enemyBlock <= CFG.strikeDmg) return STRIKE; // lethal now
    if (m[DEFEND] && inc > 0 && s.playerHp <= inc + 6) return DEFEND;
    if (m[STRIKE]) return STRIKE;
    if (m[DEFEND] && inc > 0) return DEFEND;
    return END;
  }
  // Attacks only. Never blocks.
  function berserkPolicy(s, m) { return m[STRIKE] ? STRIKE : END; }
  // Random over cards; ends the turn only when nothing else is playable.
  function randomCardPolicy(s, m, rand) {
    if (!m[STRIKE] && !m[DEFEND]) return END;
    return randomPolicy(s, [m[STRIKE], m[DEFEND], false], rand);
  }
  // Blocks whenever a hit is coming, strikes with what is left.
  function turtlePolicy(s, m) {
    if (m[DEFEND] && incoming(s) > s.block) return DEFEND;
    return m[STRIKE] ? STRIKE : END;
  }

  // ---- featurizers: what the learning agent gets to see --------------------
  // coarse: a handful of buckets. Small table, learns fast, cannot tell everything apart.
  function coarseKey(s) {
    const inc = Math.max(0, incoming(s) - s.block);
    const danger = inc === 0 ? 'safe' : s.playerHp <= inc ? 'lethal' : s.playerHp <= inc + 6 ? 'low' : 'ok';
    const hp = s.playerHp <= 9 ? 'hp1' : s.playerHp <= 18 ? 'hp2' : 'hp3';
    const toKill = Math.min(6, Math.ceil((s.enemyHp + s.enemyBlock) / CFG.strikeDmg));
    return `${s.intent}|e${s.energy}|s${s.strikes}|d${s.defends}|${danger}|${hp}|k${toKill}`;
  }
  // full: every number that matters. Exact, but a far bigger table.
  function fullKey(s) {
    return `${s.intent}|${s.energy}|${s.strikes}|${s.defends}|${s.playerHp}|${s.block}|${s.enemyHp}|${s.enemyBlock}|${s.str}`;
  }
  // blind: ignores the intent and the enemy. Shows what a bad observation costs.
  function blindKey(s) { return `e${s.energy}|s${s.strikes}|d${s.defends}`; }
  const FEATURIZERS = { coarse: coarseKey, full: fullKey, blind: blindKey };

  // ---- a DOM renderer for the fight panel ----------------------------------
  function render(root, s, opts) {
    opts = opts || {};
    const E = Course.el;
    root.innerHTML = '';
    const m = legal(s);
    const inc = incoming(s);
    const player = E('div', { class: 'fighter' }, [
      E('div', { class: 'name' }, ['You', E('span', { class: 'muted' }, [`turn ${s.turn}`])]),
      E('div', { class: 'bar hp' }, [E('div', { style: `width:${100 * s.playerHp / s.maxHp}%` })]),
      E('div', { class: 'stat' }, [`HP ${s.playerHp}/${s.maxHp}`, `block ${s.block}`, `energy ${s.energy}/${CFG.energy}`].map(t => E('span', {}, [t]))),
      E('div', { class: 'stat muted' }, [`draw ${s.drawS + s.drawD}`, `discard ${s.discS + s.discD}`].map(t => E('span', {}, [t]))),
    ]);
    const enemy = E('div', { class: 'fighter' }, [
      E('div', { class: 'name' }, ['Worm', E('span', { class: 'muted' }, [s.str ? `str +${s.str}` : ''])]),
      E('div', { class: 'bar hp' }, [E('div', { style: `width:${100 * s.enemyHp / s.enemyMaxHp}%` })]),
      E('div', { class: 'stat' }, [`HP ${s.enemyHp}/${s.enemyMaxHp}`, `block ${s.enemyBlock}`].map(t => E('span', {}, [t]))),
      E('div', { class: 'intent' }, [s.done ? (s.won ? 'dead' : 'victorious') : `intent: ${intentText(s)}` + (inc > s.block ? ` → you take ${inc - s.block}` : inc ? ' → blocked' : '')]),
    ]);
    root.appendChild(E('div', { class: 'fight' }, [player, enemy]));
    const hand = E('div', { class: 'hand' });
    const mk = (cls, label, sub, a) => {
      const b = E('button', { class: `card ${cls}`, disabled: !m[a] ? 'disabled' : null }, [label, E('span', { class: 'cost' }, [sub])]);
      if (!m[a]) b.disabled = true; else b.removeAttribute('disabled');
      if (opts.onAction) b.addEventListener('click', () => opts.onAction(a));
      if (opts.highlight === a) b.style.outline = '2px solid var(--accent)';
      return b;
    };
    hand.appendChild(mk('strike', `Strike ×${s.strikes}`, `1 energy · ${CFG.strikeDmg} damage`, STRIKE));
    hand.appendChild(mk('defend', `Defend ×${s.defends}`, `1 energy · ${CFG.defendBlock} block`, DEFEND));
    hand.appendChild(mk('end', 'End turn', 'enemy acts', END));
    root.appendChild(hand);
    if (s.done) root.appendChild(E('div', { class: `banner ${s.won ? 'win' : 'loss'}` }, [s.won ? 'You won. Reward +1.' : 'You lost. Reward −1.']));
  }

  // ---- learners used by more than one lesson -------------------------------
  // Tabular Q-learning over keys from a featurizer.
  function QAgent(o) {
    const self = {
      Q: new Map(), alpha: o.alpha ?? 0.1, gamma: o.gamma ?? 0.99, eps: o.eps ?? 0.1,
      key: o.key || coarseKey, rewardFn: o.rewardFn || null, rand: o.rand || Course.rng(1),
      episodes: 0, wins: 0, steps: 0, history: [],
    };
    self.q = (k) => { let v = self.Q.get(k); if (!v) { v = [0, 0, 0]; self.Q.set(k, v); } return v; };
    self.greedy = (s, m) => {
      const q = self.q(self.key(s));
      let best = -1, bv = -Infinity;
      for (let a = 0; a < 3; a++) if (m[a] && q[a] > bv) { bv = q[a]; best = a; }
      return best;
    };
    self.act = (s, m) => (self.rand() < self.eps ? randomPolicy(s, m, self.rand) : self.greedy(s, m));
    self.policy = (s, m) => self.greedy(s, m);
    self.episode = () => {
      const s = newFight(self.rand);
      let ret = 0;
      while (!s.done && s.steps < 400) {
        const k = self.key(s), m = legal(s);
        const a = self.act(s, m);
        const before = clone(s);
        let { reward, done } = step(s, a, self.rand);
        if (self.rewardFn) reward = self.rewardFn(before, a, s, reward, done);
        ret += reward;
        const q = self.q(k);
        let target = reward;
        if (!done) {
          const q2 = self.q(self.key(s)), m2 = legal(s);
          let best = -Infinity; for (let b = 0; b < 3; b++) if (m2[b]) best = Math.max(best, q2[b]);
          target += self.gamma * best;
        }
        q[a] += self.alpha * (target - q[a]);
        self.steps++;
      }
      self.episodes++; if (s.won) self.wins++;
      return s;
    };
    return self;
  }

  // Win rate of a policy over n fights.
  function evaluate(policy, n, seed) {
    const rand = Course.rng(seed || 7);
    let w = 0, steps = 0;
    for (let i = 0; i < n; i++) { const s = play(policy, rand); if (s.won) w++; steps += s.steps; }
    return { winRate: w / n, meanSteps: steps / n };
  }

  return { CFG, ACTIONS, STRIKE, DEFEND, END, newFight, clone, step, legal, incoming, intentText, play, render,
    randomPolicy, randomCardPolicy, heuristicPolicy, berserkPolicy, turtlePolicy, coarseKey, fullKey, blindKey, FEATURIZERS, QAgent, evaluate };
})();
