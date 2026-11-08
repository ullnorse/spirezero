// What the start screen gains: Daily, History and sharing a seed; and the glue that hands the
// page's helpers to the preview, history and share modules. app.js calls `initExtras` once.
import { dailyResult, historySheet, initHistory, recordRun } from "./history.js";
import { markDaily, utcDate } from "./daily.js";
import { compendiumSheet, initCompendium } from "./compendium.js";
import { initPreview, attachPreview, upgradeSection } from "./preview.js";
import { shareRun, shareWords, takeSharedLink } from "./share.js";

export { recordRun, attachPreview, upgradeSection, takeSharedLink };

let ctx = null;
export function initExtras(c) {
  ctx = c;
  initHistory(c);
  initCompendium(c);
  initPreview(c);
}

// The start screen's menu (Daily, Custom Run, History) and its Share chip for the seed typed in,
// with the chosen hero and ascension. Daily is the game's own pick for the UTC date: hero,
// ascension, seed and modifiers.
export function startExtras({ picked, opts, startRun, custom }) {
  const { el, tap } = ctx;
  const date = utcDate();
  const done = dailyResult(date);
  const daily = tap(
    el("button", "dailybtn", el("b", null, "Daily"), el("small", null, done ? `${done.result === "win" ? "Won" : "Lost"} · floor ${done.floor}` : date)),
    () => {
      const [y, m, d] = date.split("-");
      const run = JSON.parse(ctx.daily_run(`${d}_${m}_${y}`));
      markDaily(run.seed, date);
      startRun(run.character, run.seed, run.ascension, run.modifiers, true);
    },
  );
  const cust = tap(el("button", "dailybtn", el("b", null, "Custom Run"), el("small", null, "Modifiers, seed, ascension")), custom);
  const compendium = tap(el("button", "dailybtn", el("b", null, "Compendium"), el("small", null, "Cards, relics, bestiary")), () => ctx.showLocal(compendiumSheet()));
  const history = tap(el("button", "dailybtn", el("b", null, "History"), el("small", null, "Runs and stats")), () => ctx.showLocal(historySheet()));
  const share = tap(el("button", "chipbtn", "Share seed"), async () => {
    const seed = opts.seed.trim().toUpperCase();
    if (!seed) return ctx.say("Type a seed to share it.");
    ctx.say(shareWords[await shareRun({ seed, char: picked(), asc: opts.asc })]);
  });
  return { menu: el("div", "startx", daily, cust, compendium, history), share };
}

// Settings' "This run": the seed as a link.
export function shareThisRun(save) {
  return ctx.tap(ctx.el("button", "chipbtn", "Share seed"), async () => ctx.say(shareWords[await shareRun({ seed: save.seed, char: save.character, asc: save.ascension })]));
}
