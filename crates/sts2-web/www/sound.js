// Sound: the game's own effects and music (audio/, rendered from the owner's install by
// tools/audio_import.py, never committed), fetched on demand after the first tap; the sounds
// synthesized below play when audio/ is absent. `play(name)` is a no-op while muted or before the first tap
// (browsers keep audio locked until then). Plain http on the LAN: nothing here needs a secure context.
let ctx = null;
let master = null;
let sfxBus = null;
let musicBus = null;
let level = 0;

const VOLUME = { low: 0.18, mid: 0.35, high: 0.6 };

// What the page can read back: every cue asked for and how it was answered, bytes fetched.
export const stats = (window.__sound = { log: [], missing: [], bytes: 0, bytesBeforeFirst: null, tracks: [] });

// `on` and `volume` ("low", "mid", "high") from the settings.
export function setSound(on, volume = "low") {
  level = on ? (VOLUME[volume] ?? VOLUME.low) : 0;
  if (master) master.gain.value = level;
  if (!level) stopMusic();
  else if (wantTrack && ctx) setTrack(wantTrack);
}

// The first tap unlocks audio; call from a pointer or click handler.
export function unlock() {
  if (!level) return;
  if (!ctx) {
    const AC = window.AudioContext ?? window.webkitAudioContext;
    if (!AC) return;
    ctx = new AC({ latencyHint: "playback" }); // larger output buffers: a busy page must not break up the sound
    unlockedAt = performance.now();
    master = ctx.createGain();
    master.gain.value = level;
    master.connect(ctx.destination);
    sfxBus = ctx.createGain();
    sfxBus.connect(master);
    musicBus = ctx.createGain();
    musicBus.gain.value = 0.55;
    musicBus.connect(master);
    load();
  }
  // iOS counts only touchend/click (not pointerdown) as a gesture that may start audio, and can
  // leave the context "suspended" or "interrupted": resume on every gesture until it runs, with
  // a silent buffer started inside the gesture.
  if (ctx.state !== "running") {
    try {
      const s = ctx.createBufferSource();
      s.buffer = ctx.createBuffer(1, 1, 22050);
      s.connect(ctx.destination);
      s.start(0);
    } catch {}
    ctx.resume?.()?.catch?.(() => {});
  }
}

// ---- the game's audio -----------------------------------------------------------------------
// manifest: undefined while loading, false when audio/ is absent (the synthesized sounds play).
let manifest;
const bufs = new Map(); // file -> AudioBuffer, or a promise of one
let unlockedAt = 0;

async function fetchBytes(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`${r.status} ${url}`);
  const b = await r.arrayBuffer();
  stats.bytes += b.byteLength;
  return b;
}

function load() {
  if (manifest !== undefined) return;
  manifest = null;
  const m = fetchBytes("audio/audio.json").then((b) => JSON.parse(new TextDecoder().decode(b)));
  m.then(
    (j) => {
      manifest = j;
      wantFile(pick(EVENTS.click));
      if (wantTrack) setTrack(wantTrack);
    },
    () => {
      manifest = false;
    },
  );
}

const pick = (path) => {
  const e = manifest?.events?.[path];
  return e?.takes?.[Math.floor(Math.random() * e.takes.length)];
};

// ogg/opus decodes in Chrome, Firefox and Safari 18.4+; older iOS Safari needs the .m4a (AAC) the
// importer writes beside each .ogg and names in the manifest as "alt". Chosen by canPlayType, and
// switched for good on the first failed decode.
let useAlt = (() => {
  try {
    return document.createElement("audio").canPlayType('audio/ogg; codecs="opus"') === "";
  } catch {
    return false;
  }
})();
stats.format = () => (useAlt ? "m4a" : "ogg");
const altOf = (file) => (file.endsWith(".ogg") ? file.slice(0, -4) + (manifest?.alt ?? ".m4a") : file);

// A decoded file, fetched once; resolves null when it can't be had.
function wantFile(file) {
  if (!file || !ctx) return Promise.resolve(null);
  let b = bufs.get(file);
  if (!b) {
    const get = (f) => fetchBytes(`audio/${f}`).then((d) => ctx.decodeAudioData(d));
    const alt = manifest?.alt && file.endsWith(".ogg");
    b = (alt && useAlt ? get(altOf(file)) : get(file))
      .catch((e) => {
        if (!alt) throw e;
        useAlt = !useAlt; // the other format; a failure of both falls through to the synth
        return get(useAlt ? altOf(file) : file);
      })
      .then((x) => {
        bufs.set(file, x);
        return x;
      })
      .catch(() => {
        bufs.delete(file);
        return null;
      });
    bufs.set(file, b);
  }
  return Promise.resolve(b);
}

const EVENTS = {
  click: "event:/sfx/ui/clicks/ui_click",
  hover: "event:/sfx/ui/clicks/ui_hover",
  back: "event:/sfx/ui/clicks/ui_back",
  block: "event:/sfx/block_gain",
  blockhit: "event:/sfx/block_hit",
  hit: "event:/sfx/enemy/enemy_impact_enemy_size/enemy_impact_base",
  gold: "event:/sfx/ui/gold/gold_1",
  energy: "event:/sfx/ui/gain_energy",
  buff: "event:/sfx/buff",
  debuff: "event:/sfx/debuff",
  heal: "event:/sfx/heal",
  map: "event:/sfx/ui/map/map_select",
  mapopen: "event:/sfx/ui/map/map_open",
  deckopen: "event:/sfx/ui/deck_open",
  deckclose: "event:/sfx/ui/deck_close",
  discard: "event:/sfx/ui/cards/card_movement_B_into_discard",
  power: "event:/sfx/ui/cards/card_movement_B_power",
  transform: "event:/sfx/ui/cards/card_transform",
  shop: "event:/sfx/npcs/merchant/merchant_welcome",
  defeat: "event:/temp/sfx/game_over",
};
const FILES = { turn: "player_turn", enemyturn: "enemy_turn", hurt: "heavy_attack", victory: "victory", start: "battle_start_1", draw: "card_deal", deny: "deny" };
const WHO = { ironclad: "ironclad", silent: "silent", defect: "defect", regent: "regent", necrobinder: "necrobinder" };

// The file (and whether it is in the manifest) a cue plays: { key, file } or null for a cue with no sample.
function resolve(name, o) {
  const who = WHO[(o?.who ?? "").toLowerCase()] ?? "ironclad";
  let path = EVENTS[name];
  if (name === "play") path = `event:/sfx/characters/${who}/${who}_${o?.attack ? "attack" : "cast"}`;
  else if (name === "die") path = `event:/sfx/characters/${who}/${who}_die`;
  else if (name === "treasure") path = `event:/sfx/ui/treasure/treasure_act${o?.act ?? 1}`;
  if (path) return { key: path, file: pick(path) };
  if (FILES[name]) return { key: FILES[name], file: manifest.files?.[FILES[name]] };
  return null;
}

function start(buf, bus, gain = 1) {
  stats.bytesBeforeFirst ??= stats.bytes;
  const s = ctx.createBufferSource();
  s.buffer = buf;
  const g = ctx.createGain();
  g.gain.value = gain;
  s.connect(g).connect(bus);
  s.start();
}

// ---- music: one looping track, crossfaded; one act's tracks held at a time --------------------
let wantTrack = null;
let music = null; // { key, src, gain }
let musicGen = 0;
let heldAct = null;

export function stopMusic() {
  musicGen++;
  const m = music;
  music = null;
  if (m) {
    const t = ctx.currentTime;
    m.gain.gain.setTargetAtTime(0, t, 0.15);
    m.src.stop(t + 1);
  }
}

// `keys`: a track of the manifest (menu, act1_a1_enemy, boss_vantom_0, ...) or a list of them,
// the first the manifest has wins; none (or null) is silence.
export function setTrack(keys) {
  wantTrack = keys;
  if (!ctx || !level || !manifest) {
    if (ctx && level && manifest === null) setTimeout(() => wantTrack === keys && setTrack(keys), 250);
    return;
  }
  // Music is background: it waits for the first effect to have played (and for 2 s at most).
  if (stats.bytesBeforeFirst === null && performance.now() - unlockedAt < 2500) {
    setTimeout(() => wantTrack === keys && setTrack(keys), 200);
    return;
  }
  const k = [keys].flat().find((x) => x && manifest.music?.[x]) ?? null;
  if (k === (music?.key ?? null)) return;
  const gen = ++musicGen;
  const old = music;
  music = null;
  if (old) {
    const t = ctx.currentTime;
    old.gain.gain.setTargetAtTime(0, t, 0.3);
    old.src.stop(t + 1.6);
  }
  if (!k) return;
  const act = /^act(\d)_/.exec(k)?.[1] ?? null;
  if (act && act !== heldAct) {
    for (const [name, m] of Object.entries(manifest.music)) if (/^act\d_/.test(name) && !name.startsWith(`act${act}_`)) bufs.delete(m.file);
    heldAct = act;
  }
  stats.tracks.push(k);
  wantFile(manifest.music[k].file).then((buf) => {
    if (!buf || gen !== musicGen || !level) return;
    const src = ctx.createBufferSource();
    src.buffer = buf;
    src.loop = true;
    const gain = ctx.createGain();
    gain.gain.value = 0;
    gain.gain.setTargetAtTime(1, ctx.currentTime, 0.5);
    src.connect(gain).connect(musicBus);
    src.start();
    music = { key: k, src, gain };
    if (act) for (const s of ["unknown", "enemy"]) { const f = manifest.music[`act${act}_a1_${s}`]?.file; if (f && !bufs.has(f)) setTimeout(() => wantFile(f), 1500); }
  });
}

// A tone from `f0` to `f1` Hz over `dur` s, `at` s from now.
function tone(type, f0, f1, dur, gain, at = 0) {
  const t = ctx.currentTime + at;
  const o = ctx.createOscillator();
  const g = ctx.createGain();
  o.type = type;
  o.frequency.setValueAtTime(f0, t);
  if (f1 !== f0) o.frequency.exponentialRampToValueAtTime(f1, t + dur);
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + 0.008);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  o.connect(g).connect(sfxBus);
  o.start(t);
  o.stop(t + dur + 0.02);
}

let noiseBuf = null;
// White noise through a band-pass sweeping from `f0` to `f1` Hz.
function noise(f0, f1, dur, gain, at = 0) {
  if (!noiseBuf) {
    noiseBuf = ctx.createBuffer(1, ctx.sampleRate * 0.5, ctx.sampleRate);
    const d = noiseBuf.getChannelData(0);
    for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  }
  const t = ctx.currentTime + at;
  const s = ctx.createBufferSource();
  s.buffer = noiseBuf;
  const f = ctx.createBiquadFilter();
  f.type = "bandpass";
  f.Q.value = 1.2;
  f.frequency.setValueAtTime(f0, t);
  f.frequency.exponentialRampToValueAtTime(f1, t + dur);
  const g = ctx.createGain();
  g.gain.setValueAtTime(0.0001, t);
  g.gain.exponentialRampToValueAtTime(gain, t + 0.01);
  g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  s.connect(f).connect(g).connect(sfxBus);
  s.start(t);
  s.stop(t + dur + 0.02);
}

const SOUNDS = {
  // A card leaves the hand: a soft swish.
  play: () => noise(900, 3200, 0.14, 0.5),
  // An enemy is hit: a thump with a crack on top.
  hit: () => {
    tone("sine", 170, 55, 0.18, 0.9);
    noise(2400, 600, 0.08, 0.45);
  },
  // You are hit: lower and longer.
  hurt: () => {
    tone("triangle", 120, 40, 0.3, 0.9);
    noise(1200, 300, 0.16, 0.5);
  },
  // Block: a short metallic ring.
  block: () => {
    tone("triangle", 660, 640, 0.16, 0.35);
    tone("sine", 1320, 1300, 0.22, 0.2, 0.01);
  },
  // Gold: two bright coin notes.
  gold: () => {
    tone("square", 1568, 1568, 0.07, 0.12);
    tone("square", 2093, 2093, 0.16, 0.12, 0.07);
  },
  // Your turn again: a soft two-note chime.
  turn: () => {
    tone("sine", 784, 784, 0.18, 0.25);
    tone("sine", 1175, 1175, 0.3, 0.22, 0.1);
  },
  // A fight won: a rising major arpeggio.
  victory: () => [523, 659, 784, 1047].forEach((f, i) => tone("triangle", f, f, i === 3 ? 0.5 : 0.16, 0.4, i * 0.1)),
  // The run lost: a falling minor line.
  defeat: () => [392, 311, 262, 196].forEach((f, i) => tone("sawtooth", f, f * 0.98, i === 3 ? 0.9 : 0.26, 0.16, i * 0.22)),
};


export function play(name, o) {
  if (!level || !ctx || ctx.state !== "running") return;
  const at = performance.now();
  const note = (how) => {
    stats.log.push(`${name}:${how}`);
    if (how.startsWith("missing")) stats.missing.push(`${name}:${how}`);
  };
  if (manifest === false) {
    note("synth");
    SOUNDS[name]?.();
    return;
  }
  if (manifest === undefined || manifest === null) {
    // The manifest is still coming: the cue plays if it lands within 600 ms.
    setTimeout(() => performance.now() - at < 600 && play(name, o), 150);
    return;
  }
  const r = resolve(name, o);
  if (!r || !r.file) return note(r ? `missing:${r.key}` : "missing:cue");
  wantFile(r.file).then((buf) => {
    if (!buf) return note(`missing:${r.key}`);
    if (performance.now() - at > 600) return note(`late:${r.key}`);
    start(buf, sfxBus);
    note(r.key);
  });
}
