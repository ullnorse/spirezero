// Offline play (secure contexts only: https or localhost; on a plain-http LAN address the browser
// has no service workers and the HTTP cache alone gives the fast repeat loads).
// serve.py gives every build its own addresses, `/v/<build>/...`, named by the `<base>` in `/`; so a
// page only ever uses the files of its build. Here: `/` is network first (3 s, then the cached copy),
// and each build the page announces is cached whole once (its shell, then whatever else the page
// asks for), answering from the cache first. Builds the page has moved on from are dropped.
const INDEX = "sts2-index";
const SHELL = ["index.html", "style.css", "extras.css", "map.css", "bot.css", "combatfx.css", "fightfx.css", "platform.css", "desktop.css", "platform.js", "app.js", "map.js", "combatfx.js", "fightfx.js", "icons.js", "sound.js", "extras.js", "hover.js", "bugreport.js", "preview.js", "history.js", "compendium.js", "daily.js", "share.js", "bot.js", "replay.js", "review.js", "worker.js", "odds.js", "bot-worker.js", "manifest.webmanifest", "icon.svg",
  "pkg/sts2_web.js", "pkg/sts2_web_bg.wasm"];
const buildOf = (html) => /<base href="\/v\/([0-9a-f]{12})\/">/.exec(html)?.[1];

// Caches the build's shell (once); the marker goes in last, so a half-finished try is redone.
async function ensure(build) {
  const name = `sts2-b-${build}`;
  const cache = await caches.open(name);
  if (!(await cache.match("done"))) {
    await Promise.all(SHELL.map((f) => cache.add(`/v/${build}/${f}`)));
    await cache.put("done", new Response(String(Date.now())));
  }
  // Keep this build and the one before it: a tab left open across a deploy still asks for its own
  // build's files (lazy modules, art, sounds), and must never get the new build's. Older ones go.
  const stamp = async (k) => Number(await (await (await caches.open(k)).match("done"))?.text()) || 0;
  const others = [];
  for (const k of await caches.keys()) if (k.startsWith("sts2-b-") && k !== name) others.push([await stamp(k), k]);
  others.sort((a, b) => b[0] - a[0]);
  for (const [, k] of others.slice(1)) await caches.delete(k);
}

self.addEventListener("install", (e) => e.waitUntil(self.skipWaiting()));
// The first visit's page was loaded before any worker existed, so the worker fetches "/" itself once it is
// active: the game is cached for offline after the first visit, not the second.
async function warm() {
  const r = await fetch("/", { cache: "no-store" });
  if (!r.ok) return;
  (await caches.open(INDEX)).put("/", r.clone());
  const build = buildOf(await r.text());
  if (build) await ensure(build);
}
self.addEventListener("activate", (e) => e.waitUntil(self.clients.claim().then(() => warm()).catch(() => {})));

async function page(e) {
  const index = await caches.open(INDEX);
  try {
    const r = await Promise.race([fetch(e.request), new Promise((_, no) => setTimeout(no, 3000))]);
    if (r.ok) {
      index.put("/", r.clone());
      e.waitUntil(r.clone().text().then((t) => buildOf(t) && ensure(buildOf(t))).catch(() => {}));
    }
    return r;
  } catch {
    return (await index.match("/")) ?? fetch(e.request);
  }
}

async function versioned(e) {
  const build = new URL(e.request.url).pathname.split("/")[2];
  const cache = await caches.open(`sts2-b-${build}`);
  const hit = await cache.match(e.request);
  if (hit) return hit;
  const r = await fetch(e.request);
  if (r.ok) cache.put(e.request, r.clone());
  return r;
}

// Pictures at `/a/<art id>/...` outlive builds: one cache per art import, kept across deploys.
let artPruned = "";
async function artwise(e) {
  const name = `sts2-a-${new URL(e.request.url).pathname.split("/")[2]}`;
  const cache = await caches.open(name);
  const hit = await cache.match(e.request);
  if (hit) return hit;
  const r = await fetch(e.request);
  if (r.ok && !r.redirected) {
    cache.put(e.request, r.clone());
    if (artPruned !== name) artPruned = name, e.waitUntil(caches.keys().then((ks) => Promise.all(ks.filter((k) => k.startsWith("sts2-a-") && k !== name).map((k) => caches.delete(k)))));
  }
  return r;
}

self.addEventListener("fetch", (e) => {
  const u = new URL(e.request.url);
  if (e.request.method !== "GET" || u.origin !== location.origin || e.request.headers.has("range")) return;
  if (u.pathname === "/") e.respondWith(page(e));
  else if (u.pathname.startsWith("/v/")) e.respondWith(versioned(e));
  else if (u.pathname.startsWith("/a/")) e.respondWith(artwise(e));
});
