// The page's side of the bot's Web Worker (bot-worker.js): one worker, started on the first call;
// `call(type, data)` resolves with the worker's answer.
let worker = null;
let req = 0;
const waiting = new Map();
let started = false;

// Whether the bot's wasm has answered once (the first call downloads it).
export const warm = () => started;

export function call(type, data = {}) {
  if (!worker) {
    worker = new Worker(new URL("./bot-worker.js", import.meta.url), { type: "module" });
    worker.onmessage = (e) => {
      started = true;
      const w = waiting.get(e.data.req);
      waiting.delete(e.data.req);
      if (!w) return;
      if (e.data.error) w.reject(new Error(e.data.error));
      else w.resolve(e.data);
    };
    // A panic aborts the wasm: every call waiting fails and the next one starts a new worker.
    worker.onerror = (e) => {
      e.preventDefault?.();
      for (const w of waiting.values()) w.reject(new Error("the bot stopped"));
      waiting.clear();
      worker?.terminate();
      worker = null;
    };
  }
  const r = ++req;
  return new Promise((resolve, reject) => {
    waiting.set(r, { resolve, reject });
    worker.postMessage({ type, req: r, ...data });
  });
}
