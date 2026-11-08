// WEB10's headless proof scenarios (proof.js loads them for `?proof=bot…`; real time: cdp.py):
// ask the bot in a fight and at a choice, its phone timing over a whole fight, the draw odds,
// and watching the bot replay a finished run.
const S = window.sts2;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function until(f, ms = 60000) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const x = f();
    if (x) return x;
    await sleep(50);
  }
  return null;
}

// Asks the bot at the current position; its line and the worker's time.
async function ask() {
  window.sts2bot = { ms: null, label: null };
  (await until(() => document.querySelector("#askbot"), 5000))?.click();
  const got = await until(() => window.sts2bot.label && window.sts2bot);
  return got ? `${got.label}|${document.querySelector("#botsay small")?.textContent}|${got.ms}ms|lit${document.querySelectorAll(".botpick, .botaim").length}` : "no answer";
}

const pct = (xs, p) => xs[Math.min(xs.length - 1, Math.floor(xs.length * p))];

export function scenarios({ play, v }) {
  // A finished run's "Watch the bot", played at 4×; with `list`, then "Where you differed".
  async function watched(list) {
    if (!play((n) => n.over, null, 20000)) return "no end";
    const entry = await until(() => document.querySelector("#screen .watchbot"), 3000);
    if (!entry) return "no entry";
    entry.click();
    document.querySelector("#watchbar .speed").click();
    document.querySelector("#watchbar .speed").click();
    await until(() => document.querySelector("#watchbar .cap small")?.textContent.includes("Win chance"), 60000);
    await sleep(list ? 15000 : 4000);
    if (!list) return document.querySelector("#watchbar .cap").textContent.slice(0, 60);
    document.querySelector("#watchbar .diff").click();
    await until(() => !document.querySelector("#watchlist")?.textContent.includes("Reading your run"), 30000);
    const l = document.querySelector("#watchlist");
    return `${document.querySelector("#watchbar .cap").textContent.slice(0, 60)}|${l.querySelectorAll("li").length} rows|${l.querySelector(".ends").textContent}`;
  }
  const fight = (n) => n.fight && !n.screen.sheet;
  return {
    botask: async () => (play(fight) ? ask() : "no fight"),
    botchoice: async () => (play((n) => n.decision === "rewards" && !document.querySelector("#screen .cards:not(.peek)")) ? ask() : "no rewards"),
    botmap: async () => (play((n) => n.decision === "map" && n.save.floor > 1) ? ask() : "no map"),
    botshop: async () => (play((n) => n.decision === "shop", "Shop") ? ask() : "no shop"),
    // The bot plays the run through the page: at each position the "?" is asked and the lit
    // buttons tapped (a proof step where nothing is lit: a potion's own sheet). The worker's time
    // a move: all of them, and in boss fights.
    botrun: async () => {
      const times = [];
      const boss = [];
      let unlit = 0;
      const unlitLabels = new Map();
      for (let i = 0; i < 4000 && !v().over; i++) {
        const at = v().save.actions.length;
        window.sts2bot = { ms: null, label: null };
        (await until(() => document.querySelector("#askbot"), 5000))?.click();
        const got = await until(() => window.sts2bot.ms != null && window.sts2bot, 30000);
        if (!got) return `no answer at ${at}`;
        if (v().fight) times.push(got.ms);
        if (v().fight && v().room === "Boss") boss.push(got.ms);
        for (let k = 0; k < 4 && v().save.actions.length === at; k++) {
          const lit = document.querySelector("#sheet .botpick, #screen .botpick, #dock .botpick");
          if (!lit) break;
          lit.click();
        }
        if (v().save.actions.length === at) {
          unlit++;
          unlitLabels.set(got.label, (unlitLabels.get(got.label) ?? 0) + 1);
          play(() => false, null, 1);
        }
      }
      times.sort((a, b) => a - b);
      boss.sort((a, b) => a - b);
      return `${v().decision}:floor${v().save.floor}:moves${times.length}:p50=${pct(times, 0.5)}:p90=${pct(times, 0.9)}:max=${times.at(-1)}:boss${boss.length}:p50=${pct(boss, 0.5) ?? "-"}:max=${boss.at(-1) ?? "-"}:unlit${unlit}:${[...unlitLabels].map(([l, n]) => `${l}x${n}`).join(",").slice(0, 400)}`;
    },
    botodds: async () => {
      if (!play((n) => fight(n) && n.save.floor >= 6)) return "no fight";
      S.open("draw");
      const o = await until(() => document.querySelector("#sheet .odds"), 3000);
      return o ? [...o.querySelectorAll(".note, .ochip")].map((e) => e.textContent).join("|") : "no odds";
    },
    botwatch: () => watched(true),
    botwatchplay: () => watched(false),
    botstart: async () => {
      if (!play((n) => n.over, null, 20000)) return "no end";
      S.showStart();
      return document.querySelector("#start .watchruns") ? `runs${document.querySelectorAll("#start .watchruns .row").length}` : "no runs";
    },
  };
}
