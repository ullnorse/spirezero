// Shared-password gate (HTTP Basic auth). Any username; password = SITE_PASSWORD secret.
// Fails closed: no secret set -> nobody gets in.
const enc = new TextEncoder();
async function same(a, b) {
  const [x, y] = await Promise.all([a, b].map(s => crypto.subtle.digest("SHA-256", enc.encode(s))));
  const u = new Uint8Array(x), v = new Uint8Array(y);
  let d = 0; for (let i = 0; i < u.length; i++) d |= u[i] ^ v[i];
  return d === 0;
}
export async function onRequest({ request, env, next }) {
  const want = env.SITE_PASSWORD;
  const h = request.headers.get("Authorization") || "";
  if (want && h.startsWith("Basic ")) {
    let pass = "";
    try { const t = atob(h.slice(6)); pass = t.slice(t.indexOf(":") + 1); } catch {}
    if (await same(pass, want)) return next();
  }
  return new Response("Password required.", {
    status: 401,
    headers: { "WWW-Authenticate": 'Basic realm="SpireZero", charset="UTF-8"', "Cache-Control": "no-store" },
  });
}
