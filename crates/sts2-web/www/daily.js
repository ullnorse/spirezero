// The daily seed: the same seed for everyone on a UTC date, a hash of the date. A daily run is
// remembered (`sts2-daily-run`) so the history records it as that day's.

const KEY = "sts2-daily-run";
const ABC = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

export const utcDate = (d = new Date()) => d.toISOString().slice(0, 10);

// 10 characters of the seed alphabet from FNV-1a over the date, stepped by xorshift32.
export function dailySeed(date = utcDate()) {
  let h = 0x811c9dc5;
  for (const ch of `sts2-daily-${date}`) h = Math.imul(h ^ ch.charCodeAt(0), 0x01000193) >>> 0;
  let s = "";
  for (let i = 0; i < 10; i++) {
    h ^= h << 13;
    h >>>= 0;
    h ^= h >>> 17;
    h ^= h << 5;
    h >>>= 0;
    s += ABC[h % ABC.length];
  }
  return s;
}

export const markDaily = (seed, date = utcDate()) => localStorage.setItem(KEY, JSON.stringify({ seed, date }));
export const clearDaily = () => localStorage.removeItem(KEY);
// The date of the daily the run on `seed` is, else null.
export function dailyOf(seed) {
  try {
    const d = JSON.parse(localStorage.getItem(KEY) ?? "null");
    return d?.seed === seed ? d.date : null;
  } catch {
    return null;
  }
}
