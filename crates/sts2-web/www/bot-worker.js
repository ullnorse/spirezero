// The bot, off the page's thread: the honest bot's wasm (sts2-web-bot, www/pkg-bot) answers the
// page's "?" (advise), plays a seed one action a step (watch, step) and lists where a finished
// run's choices differed from its own (differ), and the CB3 combat net's move in a fight (netAdvise,
// WEB82; its weights, `net/cb3.bin`, are fetched on its first use). Loaded on the first use.
import init, { Advisor, CombatNet, Review, Watch, differ, load_text, text_tables } from "./pkg-bot/sts2_web_bot.js";

// The game's text, as the page loads it, so the bot's replay names things the same way.
async function loadText() {
  const tables = JSON.parse(text_tables());
  const got = await Promise.all(tables.map((t) => fetch(`text/${t}.json`).then((r) => (r.ok ? r.text() : null)).catch(() => null)));
  const parts = [];
  tables.forEach((t, i) => {
    if (got[i] && got[i].trim().startsWith("{")) parts.push(`${JSON.stringify(t)}:${got[i]}`);
  });
  if (parts.length) load_text(`{${parts.join(",")}}`);
}

const ready = init().then(loadText);
let advisor = null;
let watch = null;
let watchId = 0;
let review = null;
// The combat net (29 MB of weights, fetched once): a promise, so asks made while it loads wait for it.
let net = null;
const loadNet = () =>
  (net ??= fetch("net/cb3.bin")
    .then((r) => {
      if (!r.ok) throw new Error(`net/cb3.bin: ${r.status}`);
      return r.arrayBuffer();
    })
    .then((b) => new CombatNet(new Uint8Array(b)))
    .catch((err) => {
      net = null;
      throw err;
    }));
let reviewId = 0;
// How long a review's step holds the worker, so a hint or a watch is not kept waiting.
const REVIEW_SLICE_MS = 120;

const newReview = (s, at, found, seen) => new Review(s.character, s.seed, s.ascension, JSON.stringify(s.modifiers ?? []), JSON.stringify(s.actions), at, found, seen);

async function answerNet(m) {
  const n = await loadNet();
  const t = performance.now();
  const s = m.save;
  const advice = JSON.parse(n.advise(s.character, s.seed, s.ascension, JSON.stringify(s.modifiers ?? []), JSON.stringify(s.actions), m.savePotions !== false));
  return { advice, ms: Math.round(performance.now() - t) };
}

function answer(m) {
  const t = performance.now();
  const s = m.save;
  if (m.type === "advise") {
    advisor ??= new Advisor();
    const advice = JSON.parse(advisor.advise(s.character, s.seed, s.ascension, JSON.stringify(s.modifiers ?? []), JSON.stringify(s.actions)));
    return { advice, ms: Math.round(performance.now() - t) };
  }
  if (m.type === "watch") {
    watch?.free();
    watch = new Watch(s.character, s.seed, s.ascension, JSON.stringify(s.modifiers ?? []), JSON.stringify(s.actions ?? []));
    watchId = m.id;
    if (s.script) watch.script(JSON.stringify(s.script));
    return { frame: JSON.parse(watch.first()) };
  }
  if (m.type === "step") {
    if (!watch || m.id !== watchId) return { stale: true };
    const frame = JSON.parse(watch.step());
    return { frame, ms: Math.round(performance.now() - t) };
  }
  if (m.type === "differ") return JSON.parse(differ(s.character, s.seed, s.ascension, JSON.stringify(s.modifiers ?? []), JSON.stringify(s.actions), 12));
  if (m.type === "reviewStart") {
    review?.free();
    review = newReview(s, m.at ?? 0, JSON.stringify(m.found ?? []), m.seen ?? 0);
    reviewId = m.id;
    return { at: review.at(), total: review.total() };
  }
  if (m.type === "reviewStep") {
    if (!review || m.id !== reviewId) return { stale: true };
    let more = review.at() < review.total();
    while (more && performance.now() - t < REVIEW_SLICE_MS) more = review.step();
    const out = { at: review.at(), total: review.total(), more };
    if (m.state || !more) out.state = JSON.parse(review.state());
    if (!more) out.result = JSON.parse(review.result());
    return out;
  }
  if (m.type === "reviewBoard") {
    const r = newReview(s, 0, "[]", 0);
    const board = JSON.parse(r.board(m.at));
    r.free();
    return { board };
  }
  if (m.type === "stop") {
    watch?.free();
    watch = null;
    return {};
  }
  return { error: `unknown ${m.type}` };
}

self.onmessage = async (e) => {
  const m = e.data;
  try {
    await ready;
    const out = m.type === "netAdvise" ? await answerNet(m) : answer(m);
    self.postMessage({ type: m.type, req: m.req, ...out });
  } catch (err) {
    self.postMessage({ type: m.type, req: m.req, error: String(err) });
  }
};
