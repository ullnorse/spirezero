// What the start screen gains: History and sharing a seed; and the glue that hands the
// page's helpers to the preview, history and share modules. app.js calls `initExtras` once.
import { historySheet, initHistory, recordRun, runSheet, lastRunId } from "./history.js";
import { compendiumSheet, initCompendium } from "./compendium.js";
import { initPreview, attachPreview, upgradeSection } from "./preview.js";
import { shareRun, shareWords, takeSharedLink } from "./share.js";

export { recordRun, runSheet, lastRunId, attachPreview, upgradeSection, takeSharedLink };

let ctx = null;
export function initExtras(c) {
  ctx = c;
  initHistory(c);
  initCompendium(c);
  initPreview(c);
}

// The start screen's menu (Custom Run, Compendium, History) and its Share chip for the seed typed in,
// with the chosen hero and ascension.
export function startExtras({ picked, opts, startRun, custom }) {
  const { el, tap } = ctx;
  const cust = tap(el("button", "dailybtn", el("b", null, "Custom Run"), el("small", null, "Modifiers and seed")), custom);
  const compendium = tap(el("button", "dailybtn", el("b", null, "Compendium"), el("small", null, "Cards, relics, bestiary")), () => ctx.showLocal(compendiumSheet()));
  const history = tap(el("button", "dailybtn", el("b", null, "History"), el("small", null, "Runs and stats")), () => ctx.showLocal(historySheet()));
  const share = tap(el("button", "chipbtn", "Share seed"), async () => {
    const seed = opts.seed.trim().toUpperCase();
    if (!seed) return ctx.say("Type a seed to share it.");
    ctx.say(shareWords[await shareRun({ seed, char: picked(), asc: opts.asc })]);
  });
  return { menu: el("div", "startx", cust, compendium, history), share };
}

// Settings' "This run": the seed as a link.
export function shareThisRun(save) {
  return ctx.tap(ctx.el("button", "chipbtn", "Share seed"), async () => ctx.say(shareWords[await shareRun({ seed: save.seed, char: save.character, asc: save.ascension })]));
}
