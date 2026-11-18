// WEB82: the combat net's parity in wasm, the page's own build (www/pkg-bot), against PyTorch's
// logits on the human panel's rows (`python/web82_export.py dump`; local data, never committed).
//   node crates/sts2-web/tools/net_parity.mjs [ROWS] [WEIGHTS]
// Defaults: ~/.cache/spirebot-data/web82/parity.bin and ~/.cache/spirebot-data/net/cb3.bin.
import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import init, { CombatNet } from "../www/pkg-bot/sts2_web_bot.js";

const data = join(homedir(), ".cache/spirebot-data");
const rows = readFileSync(process.argv[2] ?? join(data, "web82/parity.bin"));
const weights = readFileSync(process.argv[3] ?? join(data, "net/cb3.bin"));
await init({ module_or_path: readFileSync(new URL("../www/pkg-bot/sts2_web_bot_bg.wasm", import.meta.url)) });
const net = new CombatNet(new Uint8Array(weights));
if (rows.toString("latin1", 0, 4) !== "CB3P") throw new Error("not a CB3P file");
const [n, width, acts] = [rows.readUInt32LE(4), rows.readUInt32LE(8), rows.readUInt32LE(12)];
const stride = width + 48 + acts * 4 + 4;
const argmax = (l, legal) => {
  let best = -1;
  for (let a = 0; a < acts; a++) if (legal(a) && (best < 0 || l[a] > l[best])) best = a;
  return best;
};
let same = 0, worst = 0, over = 0, ms = 0;
for (let i = 0; i < n; i++) {
  const at = 16 + i * stride;
  const obs = new Uint8Array(rows.buffer, rows.byteOffset + at, width);
  const mask = new BigUint64Array(6);
  for (let k = 0; k < 6; k++) mask[k] = rows.readBigUInt64LE(at + width + 8 * k);
  const want = new Float32Array(acts);
  for (let a = 0; a < acts; a++) want[a] = rows.readFloatLE(at + width + 48 + 4 * a);
  const t = performance.now();
  const got = net.logits(obs, mask);
  ms += performance.now() - t;
  let rowWorst = 0;
  for (let a = 0; a < acts; a++) {
    const legal = want[a] > -1e29;
    if (legal !== Number.isFinite(got[a])) throw new Error(`row ${i} index ${a}: legality differs`);
    if (legal) rowWorst = Math.max(rowWorst, Math.abs(want[a] - got[a]));
  }
  worst = Math.max(worst, rowWorst);
  over += rowWorst > 1e-3;
  same += argmax(want, (a) => want[a] > -1e29) === argmax(got, (a) => Number.isFinite(got[a]));
}
console.log(`wasm parity: ${n} rows, argmax agrees on ${same} (${((100 * same) / n).toFixed(2)}%), max |logit diff| ${worst.toExponential(2)}, rows over 1e-3: ${over}, ${(ms / n).toFixed(1)} ms per forward pass`);
process.exit(n >= 500 && same / n >= 0.995 && worst <= 1e-3 ? 0 : 1);
