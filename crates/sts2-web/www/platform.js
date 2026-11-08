// Phone-app behaviour (WEB31): swipe a sheet down, keep the screen on during a run, the update
// toast and the Install button. Loaded before app.js so `beforeinstallprompt`, which Chrome fires
// early, is not missed; app.js calls `attach` once the page is up and shows `settings` in its sheet.
const $ = (id) => document.getElementById(id);
const el = (tag, cls, ...kids) => {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  for (const k of kids) if (k != null) e.append(k);
  return e;
};
let S = null; // window.sts2, once attached
const reduced = () => S?.pref("motion") === "reduced" || (S?.pref("motion") !== "full" && matchMedia("(prefers-reduced-motion: reduce)").matches);

// ---- swipe a sheet --------------------------------------------------------------------------
// Drag the head (or the body, when it is scrolled to the top) down: the panel follows the finger
// and closes past a third of its height or on a fling, else springs back. A pinned choice sheet
// folds ("Look") instead of closing, and unfolds when dragged up.
const FLING = 0.6; // px per ms
let gest = null;

const panelOf = () => document.querySelector("#sheet:not([hidden]):not(.closing) .panel");
const backdropOf = (p) => p.previousElementSibling;

function begin(y, fromBody) {
  const panel = panelOf();
  if (!panel || gest) return false;
  const now = performance.now();
  gest = { panel, y0: y, y, t: now, v: 0, on: false, fromBody, pinned: $("sheet").classList.contains("pinned") };
  return true;
}

// True once the drag has taken over (the caller then stops the page from scrolling).
function track(y) {
  const g = gest;
  if (!g) return false;
  const now = performance.now();
  g.v = (y - g.y) / Math.max(1, now - g.t);
  g.y = y;
  g.t = now;
  const dy = y - g.y0;
  if (!g.on) {
    if (Math.abs(dy) < 8) return false;
    const folded = g.pinned && $("sheet").classList.contains("collapsed");
    const scrolled = g.panel.querySelector(".sheetbody").scrollTop > 0;
    // Down from anywhere at the top; up only to unfold a folded sheet. The body scrolls otherwise.
    if ((dy < 0 && !folded) || (g.fromBody && (dy < 0 || scrolled))) {
      gest = null;
      return false;
    }
    g.on = true;
    g.panel.style.transition = "none";
    g.panel.style.animation = "none";
  }
  if (!reduced()) {
    const down = Math.max(0, dy);
    g.panel.style.transform = `translateY(${down}px)`;
    const back = backdropOf(g.panel);
    if (back && !g.pinned) back.style.opacity = String(Math.max(0, 1 - down / g.panel.offsetHeight));
  }
  return true;
}

function finish() {
  const g = gest;
  gest = null;
  if (!g?.on) return;
  const dy = g.y - g.y0;
  const p = g.panel;
  const h = p.offsetHeight;
  const fling = Math.abs(g.v) > FLING && performance.now() - g.t < 120;
  const down = dy > h / (g.pinned ? 5 : 3) || (fling && g.v > 0 && dy > 8);
  const up = dy < -h / 5 || (fling && g.v < 0 && dy < -8);
  const settle = () => {
    p.style.transition = "";
    p.style.transform = "";
    p.style.animation = "";
    const back = backdropOf(p);
    if (back) back.style.opacity = "";
  };
  if (g.pinned) {
    const chip = p.querySelector(".sheethead .chipbtn");
    const want = down ? "Look" : up ? "Show" : null;
    if (want && chip?.textContent === want) {
      settle();
      chip.click(); // the sheet redraws folded or open
      return;
    }
  } else if (down) {
    if (reduced()) return S.closeSheet();
    p.style.transition = "transform .16s ease-in";
    p.style.transform = "translateY(100%)";
    setTimeout(() => S.closeSheet(), 160);
    return;
  }
  p.style.transition = reduced() ? "none" : "transform .22s cubic-bezier(.2,.8,.3,1)";
  p.style.transform = "";
  const back = backdropOf(p);
  if (back) back.style.opacity = "";
  setTimeout(settle, 240);
}

function initSwipe() {
  const sheet = $("sheet");
  // The head and grabber: pointer events (the head sets `touch-action: none`).
  sheet.addEventListener("pointerdown", (ev) => {
    if (ev.button > 0 || !ev.target.closest(".sheethead") || ev.target.closest("button")) return;
    if (!begin(ev.clientY, false)) return;
    const move = (m) => track(m.clientY);
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", end);
      finish();
    };
    window.addEventListener("pointermove", move, { passive: true });
    window.addEventListener("pointerup", end);
    window.addEventListener("pointercancel", end);
  });
  // The body, from the top of its scroll: touch events, because the browser takes the pointer
  // over (pointercancel) once it starts to scroll.
  sheet.addEventListener("touchstart", (ev) => {
    if (ev.touches.length === 1 && ev.target.closest(".sheetbody")) begin(ev.touches[0].clientY, true);
  }, { passive: true });
  sheet.addEventListener("touchmove", (ev) => {
    if (gest?.fromBody && track(ev.touches[0].clientY) && ev.cancelable) ev.preventDefault();
  }, { passive: false });
  const done = () => gest?.fromBody && finish();
  sheet.addEventListener("touchend", done);
  sheet.addEventListener("touchcancel", done);
}

// ---- keep the screen on -----------------------------------------------------------------------
let wantAwake = false;
let lock = null;
let asking = false;
const awake = () => wantAwake && S?.pref("keepAwake") !== false && document.visibilityState === "visible";
async function syncLock() {
  if (!("wakeLock" in navigator)) return;
  try {
    if (awake() && !lock && !asking) {
      asking = true;
      const l = await navigator.wakeLock.request("screen");
      asking = false;
      l.addEventListener("release", () => (lock = null));
      lock = l;
      if (!awake()) syncLock(); // the want changed while the request was out
    } else if (!awake() && lock) {
      const l = lock;
      lock = null;
      await l.release();
    }
  } catch {
    asking = false; // refused (low battery, not allowed): fail silently
  }
}
document.addEventListener("visibilitychange", syncLock);

// ---- the update toast -------------------------------------------------------------------------
function initUpdate() {
  if (!("serviceWorker" in navigator)) return;
  // A page that began with no worker is a first install: the worker taking control is not an update.
  const had = !!navigator.serviceWorker.controller;
  navigator.serviceWorker.addEventListener("controllerchange", () => {
    if (!had || $("updated")) return;
    const t = el("button", "updated", "Updated — tap to reload");
    t.id = "updated";
    t.addEventListener("click", () => {
      // Never mid-drag; a save follows every tap, so any other moment is safe.
      const go = () => (document.body.classList.contains("dragging") || gest?.on ? setTimeout(go, 200) : location.reload());
      go();
    });
    document.body.append(t);
  });
}

// ---- install ----------------------------------------------------------------------------------
let deferred = null;
const standalone = () => matchMedia("(display-mode: standalone)").matches || navigator.standalone === true;
const ios = () => /iPad|iPhone|iPod/.test(navigator.userAgent) || (navigator.platform === "MacIntel" && navigator.maxTouchPoints > 1);
function syncInstall() {
  const foot = document.querySelector("#start:not([hidden]) .foot");
  const have = foot?.querySelector(".install");
  if (standalone() || !(deferred || ios())) return have?.remove();
  if (!foot || have) return;
  foot.append(installButton("chipbtn install"));
}
const canInstall = () => !standalone() && !!(deferred || ios());
function installButton(cls) {
  const b = el("button", cls, "Install app");
  b.addEventListener("click", async (ev) => {
    ev.stopPropagation();
    if (deferred) {
      const d = deferred;
      deferred = null;
      d.prompt();
      await d.userChoice.catch(() => {});
      syncInstall();
    } else {
      S.showLocal({ title: "Install app", body: () => [el("p", "prompt", "Share → Add to Home Screen")] });
    }
  });
  return b;
}
window.addEventListener("beforeinstallprompt", (ev) => {
  ev.preventDefault();
  deferred = ev;
  syncInstall();
});
window.addEventListener("appinstalled", () => {
  deferred = null;
  syncInstall();
});
matchMedia("(display-mode: standalone)").addEventListener?.("change", syncInstall);

// ---- hooks for app.js -------------------------------------------------------------------------
window.sts2platform = {
  attach(sts2) {
    S = sts2;
    initSwipe();
    initUpdate();
    S.hook((kind, view) => {
      if (kind === "start") {
        wantAwake = false;
        syncInstall();
      } else if (kind !== "sheet") wantAwake = !!view && !view.over;
      syncLock();
    });
  },
  // The two settings this adds, for the settings sheet (`segment` is app.js's row of choices).
  settings(segment) {
    const on = [[true, "On"], [false, "Off"]];
    return [
      "wakeLock" in navigator ? [el("h3", null, "Keep screen on"), segment("keepAwake", on, syncLock)] : [],
      navigator.vibrate ? [el("h3", null, "Vibration"), segment("vibration", on, () => navigator.vibrate(8))] : [],
      // A short phone's start screen has no room for it: it is here instead.
      canInstall() ? [el("h3", null, "App"), installButton("chipbtn")] : [],
    ].flat(2);
  },
};
