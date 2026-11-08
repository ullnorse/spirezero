// WEB62: the fight's events played back in order, as the game paces them (fast mode). The page is
// already drawn in its final state, so input is never held: a tap or key ends the playback at
// once. Only transforms and opacity move. Timings from the game: `CreatureCmd.Damage` waits
// 0.1 s after a hit and shakes the screen by the unblocked damage (< 6, < 11, more);
// `GainBlock` waits 0.1 s; the enemy turn starts after 0.5 s; `NDamageNumVfx` pops at 2.5× and
// settles to 1× as it rises and fades.
let ctx;
export const init = (c) => (ctx = c);

const HIT = 130; // between hits
const BLOCK = 90;
const POWER = 80;
const MOVE = 220; // an enemy's wind-up before its first hit lands
const ENEMY_TURN = 260; // the beat before the enemies act

let timers = [];
let ends = [];

// Ends the playback: every bar at its final value, every ghost gone.
export function finish() {
  for (const t of timers) clearTimeout(t);
  timers = [];
  const todo = ends;
  ends = [];
  for (const f of todo) f();
}
for (const kind of ["pointerdown", "keydown"]) document.addEventListener(kind, finish, { capture: true, passive: true });

const at = (ms, f) => (ms <= 0 ? f() : timers.push(setTimeout(f, ms)));
const restart = (node, cls) => {
  node.classList.remove(cls);
  void node.offsetWidth;
  node.classList.add(cls);
};

const nodeOf = (id) => document.querySelector(`#screen .enemy[data-id="${id}"], #screen .you .me[data-id="${id}"], #screen .you .pet[data-id="${id}"]`);
const creatures = (v) => (v?.fight ? [...ctx.blocksOf(v.screen, "Enemy"), ...ctx.blocksOf(v.screen, "You"), ...ctx.blocksOf(v.screen, "Pet")] : []);

// `x`: where across the node it rises (a fraction), else its middle.
function float(node, cls, text, ms, x) {
  if (!node) return;
  const f = ctx.el("span", `ffx ${cls}`, text);
  f.style.setProperty("--dx", `${Math.round(Math.random() * 24 - 12)}px`);
  if (x != null) f.style.left = `${Math.round(Math.max(0.1, Math.min(0.9, x)) * 100)}%`;
  if (ms) f.style.animationDuration = `${ms}ms`;
  node.append(f);
  ends.push(() => f.remove());
  timers.push(setTimeout(() => f.remove(), ms ?? 1100));
}

// A creature's bar fill steps with the hits; its number is the final one throughout (what the
// page says always matches the engine).
function showHp(node, hp, max) {
  const fill = node?.querySelector(".hpbar .fill");
  if (fill) fill.style.width = `${max ? Math.max(0, Math.min(100, (100 * hp) / max)) : 0}%`;
}
function settleHp(node) {
  const fill = node?.querySelector(".hpbar .fill");
  if (fill?.dataset.to) fill.style.width = fill.dataset.to;
}

const SHAKE = (lost) => (lost < 6 ? "s1" : lost < 11 ? "s2" : "s3");

// Where across `node` the blow comes from: under its dealer, so on your row each enemy's hits
// rise below it; a repeated hit steps aside from the one before.
function across(node, dealer, k) {
  const from = dealer != null && nodeOf(dealer);
  const r = node.getBoundingClientRect();
  const f = from && from !== node ? from.getBoundingClientRect() : null;
  // Only when the dealer stands above (a phone); beside you (a laptop's stage) it is the middle.
  const base = f && f.bottom <= r.top + 4 ? (f.left + f.width / 2 - r.left) / r.width : 0.5;
  return base + ((k % 3) - 1) * (36 / Math.max(r.width, 1));
}

function hit(node, lost, blocked, you, x) {
  if (!node) return;
  if (lost > 0) {
    node.classList.remove("s1", "s2", "s3");
    restart(node, "hit");
    node.classList.add(SHAKE(lost));
    float(node, "dmg", `${lost}`, null, x);
    if (you) {
      const board = document.querySelector("#screen .board");
      if (board) {
        board.classList.remove("s1", "s2", "s3");
        restart(board, "quake");
        board.classList.add(SHAKE(lost));
      }
      document.querySelector("#top .hp")?.classList.add("hurt");
    }
  } else if (blocked > 0) {
    restart(node, "blockhit");
    float(node, "blocked", "Blocked", null, x);
  }
}

// `PowerChanged` carries the power's new amount; the change is against the last amount seen.
const words = (kind) => kind.replace(/([a-z])([A-Z])/g, "$1 $2");
const key = (s) => String(s).replace(/[^a-z]/gi, "").toLowerCase();

// The played card's ghost: to its target, then into the pile the event names (a power into you,
// an exhausted card burns where it is played). `f` is app.js's: the tile's rect and clone.
function fly(f, to) {
  if (!f || !view().fight) return 0;
  if (f.cardId != null && document.querySelector(`#screen .hand .card[data-card="${f.cardId}"], #screen .hand .card.armed`)) return 0;
  const g = f.ghost;
  g.classList.remove("armed", "peek");
  g.classList.add("flycard");
  Object.assign(g.style, { left: `${f.rect.left}px`, top: `${f.rect.top}px`, width: `${f.rect.width}px`, height: `${f.rect.height}px`, transition: "none" });
  const attack = g.classList.contains("k-attack");
  const aim = f.to ?? document.querySelector(attack ? "#screen .field" : "#screen .you .me")?.getBoundingClientRect();
  if (!aim) return 0;
  const pile = to === "Discard" ? "discard" : to === "Draw" ? "draw" : to === "Exhaust" ? "exhaust" : null;
  const end = (pile && document.querySelector(`#dock .pile[data-pile="${pile}"]`)?.getBoundingClientRect()) ?? null;
  const cx = f.rect.left + f.rect.width / 2;
  const cy = f.rect.top + f.rect.height / 2;
  const mid = (r) => [r.left + r.width / 2 - cx, r.top + r.height / 2 - cy];
  const [ax, ay] = mid(aim);
  const tilt = ax > 0 ? 6 : -6;
  const frames = [
    { transform: "none", opacity: 1 },
    { transform: `translate(${ax}px, ${ay}px) scale(.62) rotate(${tilt}deg)`, opacity: 1, offset: 0.42 },
  ];
  if (end && pile !== "exhaust") {
    const [ex, ey] = mid(end);
    frames.push({ transform: `translate(${ex}px, ${ey}px) scale(.14) rotate(${-tilt * 2}deg)`, opacity: 0.35 });
  } else if (pile === "exhaust") frames.push({ transform: `translate(${ax}px, ${ay - 30}px) scale(.8)`, opacity: 0 });
  else frames.push({ transform: `translate(${ax}px, ${ay}px) scale(.2)`, opacity: 0 });
  document.body.append(g);
  const dur = 560;
  g.animate(frames, { duration: dur, easing: "cubic-bezier(.4,0,.5,1)", fill: "forwards" });
  ends.push(() => g.remove());
  timers.push(setTimeout(() => g.remove(), dur + 20));
  return Math.round(dur * 0.42); // the card reaches its target: the hits land then
}

const view = () => ctx.view();

// WEB67: a card exhausted without being played (Brand's choice, an Ethereal card at the turn's
// end) burns where it stood, its tile from before the redraw rising and fading as the played
// exhaust does; one from the draw or discard pile pulses that pile. The exhaust pile takes it.
const EXHAUST = 520;
function burn(card, from) {
  const pile = (p) => document.querySelector(`#dock .pile[data-pile="${p}"]`);
  const was = from === "Hand" || from == null ? ctx.tilesBefore().get(card) : null;
  if (was) {
    const g = was.ghost;
    g.classList.remove("armed", "peek", "lifted", "glow");
    g.classList.add("flycard", "burncard");
    Object.assign(g.style, { left: `${was.rect.left}px`, top: `${was.rect.top}px`, width: `${was.rect.width}px`, height: `${was.rect.height}px`, transition: "none" });
    document.body.append(g);
    g.animate(
      [
        { transform: "none", opacity: 1 },
        { transform: "translateY(-8px) scale(1.04)", opacity: 1, offset: 0.3 },
        { transform: "translateY(-34px) scale(.86)", opacity: 0 },
      ],
      { duration: EXHAUST, easing: "ease-in", fill: "forwards" },
    );
    ends.push(() => g.remove());
    timers.push(setTimeout(() => g.remove(), EXHAUST + 20));
  } else if (from === "Draw" || from === "Discard") {
    const p = pile(from.toLowerCase());
    if (p) restart(p, "took");
  }
  const ex = pile("exhaust");
  if (ex) {
    timers.push(setTimeout(() => restart(ex, "took"), was ? EXHAUST * 0.6 : 0));
    ends.push(() => ex.classList.remove("took"));
  }
}

// Plays `view().events` against the view before them. Returns false when there is nothing to
// play (the caller then draws the plain before/after feedback).
export function play(flying) {
  finish();
  const was = ctx.before();
  const now = view();
  const events = now?.events ?? [];
  if (!now?.fight || !was?.fight || !events.length) {
    if (flying) fly(flying, null);
    return false;
  }
  const quiet = ctx.reduced() || document.body.classList.contains("proof");
  const hp = new Map(creatures(was).map((c) => [c.id, c.hp]));
  const max = new Map(creatures(now).map((c) => [c.id, c.max_hp]));
  const youId = ctx.blocksOf(now.screen, "You")[0]?.id;
  const powers = new Map();
  for (const c of creatures(was)) for (const p of c.powers ?? []) powers.set(`${c.id}:${key(p.name)}`, p.amount ?? 0);
  const debuffs = new Set(creatures(now).flatMap((c) => (c.powers ?? []).filter((p) => p.debuff).map((p) => key(p.name))));

  // Each bar starts where it was and steps with the hits.
  const stepped = new Set(events.filter((e) => e.kind === "Damage" && e.hp_lost > 0).map((e) => e.receiver));
  for (const id of stepped) {
    const n = nodeOf(id);
    const fill = n?.querySelector(".hpbar .fill");
    if (!fill) continue;
    fill.dataset.step = "1";
    ends.push(() => settleHp(n));
    if (!quiet) showHp(n, hp.get(id), max.get(id));
  }
  if (quiet) {
    finish();
    return true;
  }

  let t = 0;
  let enemyTurn = false;
  let moves = 0;
  let sounded = 0;
  const hits = new Map(); // per receiver and dealer: the hits so far, to step them aside
  const snd = (name) => {
    if (t - sounded < 60 && sounded) return;
    sounded = t;
    at(t, () => ctx.sound(name));
  };
  for (const e of events) {
    switch (e.kind) {
      case "CardPlayed":
        if (flying) {
          t += fly(flying, e.to);
          flying = null;
        }
        break;
      case "TurnStarted":
        if (e.side === "Enemy") {
          enemyTurn = true;
          t += ENEMY_TURN;
        } else if (enemyTurn) {
          t += 120;
          const hand = document.querySelector("#screen .hand");
          if (hand) {
            hand.classList.add("later");
            hand.style.setProperty("--deal", `${t}ms`);
            ends.push(() => hand.classList.remove("deal", "later"));
          }
        }
        break;
      case "Shuffled": {
        const s = t;
        at(s, () => {
          for (const p of document.querySelectorAll('#dock .pile[data-pile="draw"], #dock .pile[data-pile="discard"]')) restart(p, "shuffle");
          float(document.querySelector('#dock .pile[data-pile="draw"]'), "pilef", "↻", 700);
        });
        t += 60;
        break;
      }
      case "MonsterMove": {
        const n = nodeOf(e.monster);
        if (!n) break;
        if (moves++) t += 90;
        at(t, () => {
          const me = document.querySelector("#screen .you .me")?.getBoundingClientRect();
          const r = n.getBoundingClientRect();
          if (me) {
            const dx = me.left + me.width / 2 - (r.left + r.width / 2);
            const dy = me.top + me.height / 2 - (r.top + r.height / 2);
            const d = Math.hypot(dx, dy) || 1;
            n.style.setProperty("--lx", `${Math.round((dx / d) * 18)}px`);
            n.style.setProperty("--ly", `${Math.round((dy / d) * 18)}px`);
          }
          restart(n, "lunge");
          ends.push(() => n.classList.remove("lunge"));
        });
        t += MOVE;
        break;
      }
      case "Damage": {
        const n = nodeOf(e.receiver);
        const left = (hp.get(e.receiver) ?? 0) - e.hp_lost;
        hp.set(e.receiver, left);
        if (!n || (!e.hp_lost && !e.blocked)) break;
        const you = e.receiver === youId;
        const s = t;
        const hk = `${e.receiver}:${e.dealer}`;
        const k = hits.get(hk) ?? 1;
        hits.set(hk, k + 1);
        at(s, () => {
          hit(n, e.hp_lost, e.blocked, you, across(n, e.dealer, k));
          if (stepped.has(e.receiver)) showHp(n, left, max.get(e.receiver));
        });
        snd(you ? (e.hp_lost ? "hurt" : "block") : "hit");
        t += HIT;
        break;
      }
      case "BlockGained": {
        const n = nodeOf(e.creature);
        if (!n || !e.amount) break;
        at(t, () => {
          restart(n, "glow");
          float(n, "gain", `+${e.amount}`);
        });
        snd("block");
        t += BLOCK;
        break;
      }
      case "PowerChanged": {
        const k = `${e.creature}:${key(e.power)}`;
        const old = powers.get(k) ?? 0;
        powers.set(k, e.amount);
        const delta = e.amount - old;
        // A countdown (Vulnerable 2 → 1) is the turn passing, not news.
        if (!delta || (delta < 0 && old > 0)) break;
        const n = nodeOf(e.creature);
        if (!n) break;
        const bad = debuffs.has(key(e.power)) || e.amount < 0;
        at(t, () => float(n, `pow${bad ? " bad" : ""}`, `${words(e.power)} ${delta > 0 ? "+" : "−"}${Math.abs(delta)}`, 1300));
        t += POWER;
        break;
      }
      case "Exhausted":
        at(t, () => burn(e.card, e.from));
        t += 140;
        break;
      case "Died": {
        const n = nodeOf(e.creature);
        if (!n) break;
        n.style.setProperty("--die", `${t}ms`);
        restart(n, "dying");
        ends.push(() => n.classList.remove("dying"));
        t += 160;
        break;
      }
      default:
    }
  }
  if (flying) fly(flying, null);
  // Nothing outlives the playback's own length: the last float is cleared by `finish`.
  timers.push(setTimeout(() => finish(), t + 1300));
  return true;
}

// WEB67: a transform shows what came of the card, as `NCardTransformVfx` does: the old card
// grows in (0.25 s) and holds to 0.75 s, a white wash covers it (0.75 s, then 0.125 s idle),
// the new card is under it as the wash clears (0.25 s); the game holds it 0.8 s before it flies
// to the deck, here longer, to be read. Several transforms stand side by side. The overlay
// takes the tap that skips it, so nothing under it is pressed; off with Motion "Reduced".
const GROW = 250;
const OLD = 750;
const WASH = 750;
const IDLE = 125;
const CLEAR = 250;
const READ = 1800;
const AWAY = 420;
export function reveal(pairs) {
  if (!pairs?.length || ctx.reduced() || document.body.classList.contains("proof")) return false;
  const swap = OLD + WASH + IDLE;
  const shown = swap + CLEAR;
  const total = shown + READ + AWAY;
  const off = (ms) => ms / total;
  const root = ctx.el("div", `reveal n${Math.min(pairs.length, 3)}`);
  root.setAttribute("aria-live", "polite");
  const row = ctx.el("div", "rv-row");
  const deck = document.querySelector("#top .deckbtn")?.getBoundingClientRect();
  const stacks = pairs.map(({ from, to }) => {
    const old = ctx.cardTile(from);
    const now = ctx.cardTile(to);
    old.classList.add("rv-old");
    now.classList.add("rv-new");
    const wash = ctx.el("span", "rv-wash");
    const stack = ctx.el("div", "rv-card", old, now, wash);
    row.append(stack);
    return { stack, old, now, wash };
  });
  root.append(ctx.el("div", "rv-dim"), row);
  document.body.append(root);
  const anim = (node, frames) => node.animate(frames, { duration: total, fill: "forwards", easing: "linear" });
  anim(root.querySelector(".rv-dim"), [
    { opacity: 0 },
    { opacity: 1, offset: off(GROW) },
    { opacity: 1, offset: off(shown + READ) },
    { opacity: 0 },
  ]);
  for (const { stack, old, now, wash } of stacks) {
    const r = stack.getBoundingClientRect();
    const away = deck ? `translate(${deck.left + deck.width / 2 - (r.left + r.width / 2)}px, ${deck.top + deck.height / 2 - (r.top + r.height / 2)}px) scale(.12)` : "translateY(-20%) scale(.6)";
    anim(stack, [
      { transform: "scale(.6)", opacity: 0, easing: "cubic-bezier(.2,.7,.3,1)" },
      { transform: "none", opacity: 1, offset: off(GROW) },
      { transform: "none", opacity: 1, offset: off(shown + READ), easing: "cubic-bezier(.5,0,.8,.4)" },
      { transform: away, opacity: 0 },
    ]);
    anim(old, [{ opacity: 1 }, { opacity: 1, offset: off(swap) }, { opacity: 0, offset: off(swap) }, { opacity: 0 }]);
    anim(now, [{ opacity: 0 }, { opacity: 0, offset: off(swap) }, { opacity: 1, offset: off(swap) }, { opacity: 1 }]);
    anim(wash, [
      { opacity: 0 },
      { opacity: 0, offset: off(OLD), easing: "cubic-bezier(.76,0,.24,1)" },
      { opacity: 1, offset: off(OLD + WASH) },
      { opacity: 1, offset: off(swap), easing: "cubic-bezier(.25,1,.5,1)" },
      { opacity: 0, offset: off(shown) },
      { opacity: 0 },
    ]);
  }
  ctx.sound("transform");
  // A skip fades the overlay at once, but it stays a moment to take the tap's click.
  let gone = false;
  const end = () => {
    if (gone) return;
    gone = true;
    for (const a of root.getAnimations({ subtree: true })) a.cancel();
    root.style.opacity = "0";
    root.classList.add("done");
    setTimeout(() => root.remove(), 400);
  };
  ends.push(end);
  timers.push(setTimeout(() => finish(), total));
  return true;
}
