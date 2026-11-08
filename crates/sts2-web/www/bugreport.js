// "Copy bug report" (WEB42): a short text block the owner pastes to describe a bug, with the run's
// moves as a compact code and a page address that replays it (`/?replay=<code>`). Nothing leaves
// the browser. The last 20 page errors and unhandled rejections are kept in memory from page load.
// Code: `v1:<hero index>:<seed>:<ascension>:<modifier keys joined by +>:<flags p/d>:<moves, base 36, joined by ,>`.
import { shareUrl } from "./share.js";

const errors = [];
const keep = (what) => {
  errors.push(`${new Date().toISOString().slice(11, 19)} ${String(what).replace(/\s+/g, " ").slice(0, 200)}`);
  if (errors.length > 20) errors.shift();
};
addEventListener("error", (e) => keep(`${e.message}${e.filename ? ` (${e.filename.split("/").pop()}:${e.lineno})` : ""}`));
addEventListener("unhandledrejection", (e) => keep(`rejected: ${e.reason?.message ?? e.reason}`));

export const encodeRun = (s) =>
  ["v1", s.character, s.seed, s.ascension ?? 0, (s.modifiers ?? []).join("+"), (s.practice ? "p" : "") + (s.daily ? "d" : ""), (s.actions ?? []).map((a) => a.toString(36)).join(",")].join(":");

// The save object a code stands for, or null when it is not one.
export function decodeRun(code) {
  const p = String(code ?? "").trim().split(":");
  if (p.length !== 7 || p[0] !== "v1") return null;
  const character = Number(p[1]);
  const ascension = Number(p[3]);
  const actions = p[6] ? p[6].split(",").map((a) => parseInt(a, 36)) : [];
  if (!Number.isInteger(character) || !Number.isInteger(ascension) || !/^[A-Z0-9_-]+$/.test(p[2]) || actions.some((a) => !Number.isInteger(a) || a < 0)) return null;
  return { character, seed: p[2], ascension, modifiers: p[4] ? p[4].split("+") : [], practice: p[5].includes("p"), daily: p[5].includes("d"), actions };
}

export const replayUrl = (code) => {
  const u = new URL(shareUrl({ seed: "X", char: 0, asc: 0 }));
  u.search = "";
  u.searchParams.set("replay", code);
  return u.toString();
};

const build = () => document.baseURI.match(/\/v\/([^/]+)\//)?.[1] ?? "unversioned";

// `save` is the run's save (view.save) or null on the start screen; `view` the engine's view.
export function report(save, view, hero) {
  const lines = [
    "Slay the Spire 2 bug report",
    `build ${build()}`,
    `screen ${innerWidth}x${innerHeight} @${devicePixelRatio}x, ${matchMedia("(pointer: coarse)").matches ? "touch" : "mouse"}`,
    `browser ${navigator.userAgent}`,
    view ? `decision ${view.decision}${view.fight ? " (fight)" : ""} · floor ${save.floor} · HP ${save.hp}/${save.max_hp}` : save ? "decision start screen (a saved run is waiting)" : "decision start screen",
  ];
  if (save) {
    lines.push(`run ${hero ?? save.character} · seed ${save.seed} · ascension ${save.ascension}${save.modifiers?.length ? ` · modifiers ${save.modifiers.join(", ")}` : ""}${save.practice ? " · practice" : ""}`);
  }
  lines.push(errors.length ? `errors (${errors.length})\n${errors.map((e) => `  ${e}`).join("\n")}` : "errors none");
  if (save) {
    const code = encodeRun(save);
    lines.push(`moves ${save.actions.length}: ${code}`, `replay ${replayUrl(code)}`);
  }
  return lines.join("\n");
}

// The clipboard; where it is not allowed (plain http has none) the phone's share sheet, else the
// old copy command; "failed" when neither works, and the sheet then shows the text to select.
export async function deliver(text) {
  if (navigator.clipboard?.writeText && window.isSecureContext) {
    try {
      await navigator.clipboard.writeText(text);
      return "copied";
    } catch {}
  }
  if (matchMedia("(pointer: coarse)").matches && navigator.share) {
    try {
      await navigator.share({ title: "Slay the Spire 2 bug report", text });
      return "shared";
    } catch (e) {
      if (e?.name === "AbortError") return "cancelled";
    }
  }
  const t = Object.assign(document.createElement("textarea"), { value: text, readOnly: true });
  t.style.cssText = "position:fixed;top:0;left:0;opacity:0";
  document.body.append(t);
  t.focus();
  t.select();
  t.setSelectionRange(0, text.length);
  let ok = false;
  try {
    ok = document.execCommand?.("copy") ?? false;
  } catch {}
  t.remove();
  if (ok) return "copied";
  return "failed";
}
