// Line icons drawn for this page (no game art): 24×24, stroked with currentColor.
const P = {
  sword: '<path d="M20 4v3.5L10.5 17 7 13.5 16.5 4Z"/><path d="M5 12.5l6.5 6.5M8 16l-4 4"/>',
  skull: '<path d="M12 3c-4.4 0-8 3.1-8 7.5 0 2.6 1.3 4.4 3 5.5V20h10v-4c1.7-1.1 3-2.9 3-5.5C20 6.1 16.4 3 12 3Z"/><circle cx="9" cy="11" r="1.6"/><circle cx="15" cy="11" r="1.6"/><path d="M10 20v-2M14 20v-2"/>',
  unknown: '<path d="M9 9a3 3 0 1 1 4.5 2.6c-.9.5-1.5 1.2-1.5 2.4"/><circle cx="12" cy="18" r=".6" fill="currentColor"/>',
  shop: '<path d="M4 9h16l-1.5 11h-13Z"/><path d="M8.5 9V7a3.5 3.5 0 0 1 7 0v2"/>',
  chest: '<rect x="3.5" y="9" width="17" height="11" rx="1.5"/><path d="M3.5 13h17M5 9a7 5 0 0 1 14 0"/><rect x="10.5" y="11.5" width="3" height="3.5" rx=".6"/>',
  rest: '<path d="M12 3c1 3 4 4.5 4 8.5a4 4 0 0 1-8 0c0-1.6.8-2.7 1.6-3.6.2 1.3.9 2.1 1.9 2.4C11 8 11 5.5 12 3Z"/><path d="M5 21l14-3M5 18l14 3"/>',
  hammer: '<path d="M13.5 4.5 19 10l-2.5 2.5L11 7Z"/><path d="M12.5 9.5 4.5 17.5a1.4 1.4 0 0 0 2 2l8-8"/>',
  spire: '<path d="M12 2 9 9h6Z"/><path d="M9 9 7 22h10L15 9"/><path d="M10.5 14h3M10 18h4"/>',
  boss: '<path d="M4 18 3 7l5 4 4-6 4 6 5-4-1 11Z"/><path d="M4 21h16"/>',
  ancient: '<circle cx="12" cy="12" r="3"/><path d="M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Z"/>',
  heart: '<path d="M12 20s-7.5-4.6-7.5-10.2A4.3 4.3 0 0 1 12 7.3a4.3 4.3 0 0 1 7.5 2.5C19.5 15.4 12 20 12 20Z"/>',
  coin: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="4.5"/>',
  stairs: '<path d="M4 20h4v-4h4v-4h4V8h4V4"/>',
  shield: '<path d="M12 3 19.5 6v5.5c0 4.6-3.2 8-7.5 9.5-4.3-1.5-7.5-4.9-7.5-9.5V6Z"/>',
  deck: '<rect x="7" y="3.5" width="12" height="16" rx="2"/><path d="M5 6.5v12A2.5 2.5 0 0 0 7.5 21H15"/>',
  map: '<path d="M3.5 6.5 9 4l6 2.5L20.5 4v13.5L15 20l-6-2.5-5.5 2.5Z"/><path d="M9 4v13.5M15 6.5V20"/>',
  menu: '<path d="M4 7h16M4 12h16M4 17h16"/>',
  close: '<path d="M6 6l12 12M18 6 6 18"/>',
  energy: '<path d="M13 2 5 13.5h6L10 22l9-12h-6Z"/>',
  star: '<path d="m12 3 2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 16.8l-5.4 2.9 1.1-6.1-4.5-4.2 6.1-.8Z"/>',
  potion: '<path d="M10 3h4M10.5 3v4.2L6 15a5 5 0 0 0 4.4 6h3.2A5 5 0 0 0 18 15l-4.5-7.8V3"/><path d="M7.5 14h9"/>',
  draw: '<rect x="5" y="4" width="11" height="15" rx="2"/><path d="M19 8v11a2 2 0 0 1-2 2H9"/>',
  discard: '<rect x="8" y="5" width="11" height="15" rx="2" transform="rotate(12 13.5 12.5)"/><path d="M4 9l2 10"/>',
  exhaust: '<path d="M12 3c1 3 4 4.5 4 8.5a4 4 0 0 1-8 0c0-1.6.8-2.7 1.6-3.6.2 1.3.9 2.1 1.9 2.4C11 8 11 5.5 12 3Z"/>',
  back: '<path d="M15 5 8 12l7 7"/>',
  play: '<path d="M8 5v14l11-7Z"/>',
  sun: '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
  phone: '<rect x="7" y="2.5" width="10" height="19" rx="2.5"/><path d="M11 18.5h2"/>',
  sound: '<path d="M4 9v6h4l5 4V5L8 9H4Z"/><path d="M16.5 8.5a5 5 0 0 1 0 7M19 6a8.5 8.5 0 0 1 0 12"/>',
  mute: '<path d="M4 9v6h4l5 4V5L8 9H4Z"/><path d="m17 9 5 6M22 9l-5 6"/>',
  moon: '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z"/>',
  gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
  log: '<path d="M7 3.5h10a2 2 0 0 1 2 2V20l-3-1.5-3 1.5-3-1.5L7 20Z"/><path d="M10 8h6M10 11.5h6M10 15h3.5"/>',
  relic: '<path d="M12 3 20 12 12 21 4 12Z"/><path d="M12 7.5 16 12l-4 4.5L8 12Z"/>',
};

// Map room keys and glyphs to icons.
export const ROOM_ICON = {
  Monster: "sword", Elite: "skull", Unknown: "unknown", Event: "unknown", Shop: "shop",
  Treasure: "chest", RestSite: "rest", Rest: "rest", Boss: "boss", Ancient: "ancient",
};
export const GLYPH_ICON = { M: "sword", E: "skull", "?": "unknown", $: "shop", T: "chest", R: "rest", B: "boss", A: "ancient" };

export function icon(name, cls = "") {
  const span = document.createElement("span");
  span.className = `ic ${cls}`;
  span.innerHTML = `<svg viewBox="0 0 24 24" aria-hidden="true">${P[name] ?? P.unknown}</svg>`;
  return span;
}

export function iconSvg(name) {
  return P[name] ?? P.unknown;
}
