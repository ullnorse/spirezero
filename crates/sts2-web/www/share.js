// Share links: `/?seed=…&char=…&asc=…` opens the start screen with that run prefilled. `char` is
// the engine's character index (the same number a save and the proof use).

export function shareUrl({ seed, char, asc }) {
  const u = new URL(location.href);
  u.search = "";
  u.hash = "";
  u.searchParams.set("seed", seed);
  u.searchParams.set("char", String(char));
  u.searchParams.set("asc", String(asc ?? 0));
  return u.toString();
}

// What the page was opened with, once: `{seed, char, asc}` (each null when absent or invalid).
// The proof's `?seed=&char=` come through the same door but it never shows the start screen.
let opened = null;
export function takeSharedLink() {
  if (opened === false) return null;
  const q = new URLSearchParams(location.search);
  const num = (k, max) => {
    const n = Number(q.get(k));
    return q.has(k) && Number.isInteger(n) && n >= 0 && n <= max ? n : null;
  };
  const seed = (q.get("seed") ?? "").trim().toUpperCase().replace(/[^A-Z0-9_-]/g, "").slice(0, 32);
  const link = { seed: seed || null, char: num("char", 4), asc: num("asc", 10) };
  opened = false;
  return link.seed || link.char != null || link.asc != null ? link : null;
}

// The phone's share sheet where it has one, else the clipboard. Resolves to what happened.
export async function shareRun(run, label = "Slay the Spire 2 seed") {
  const url = shareUrl(run);
  const touch = matchMedia("(pointer: coarse)").matches;
  if (touch && navigator.share) {
    try {
      await navigator.share({ title: label, text: `${label} ${run.seed}`, url });
      return "shared";
    } catch (e) {
      if (e?.name === "AbortError") return "cancelled";
    }
  }
  try {
    await navigator.clipboard.writeText(url);
    return "copied";
  } catch {
    const t = Object.assign(document.createElement("textarea"), { value: url });
    document.body.append(t);
    t.select();
    const ok = document.execCommand?.("copy");
    t.remove();
    return ok ? "copied" : "failed";
  }
}

export const shareWords = { shared: "Shared", copied: "Link copied", failed: "Could not copy the link", cancelled: "" };
