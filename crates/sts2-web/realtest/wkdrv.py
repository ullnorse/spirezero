"""The realtest driver on Playwright WebKit (Linux WebKitGTK/WPE build, not iOS), with an iPhone 13 profile for
the phone: touch, 390x664, device scale 3. Same interface as drv.Page. Taps are real touch taps; WebKit on Linux
has no touch swipe, so drags and holds are touch-typed PointerEvents dispatched at the element under the finger
(they run the app's own drag code, not the engine's native panning)."""
import time
from playwright.sync_api import sync_playwright

_DISPATCH = """([type, x, y]) => {
  const t = document.elementFromPoint(x, y) || document.body;
  const o = {bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, pointerId: 77, pointerType: 'touch', isPrimary: true, button: 0, buttons: type === 'pointerup' ? 0 : 1, width: 1, height: 1};
  // A browser drops its capture when the pressed element is removed (the app re-renders when a card lifts) and goes on
  // hit-testing; so do the same, or the moves go to a detached node and never reach the page.
  const f = window.__finger;
  (f && f.isConnected ? f : t).dispatchEvent(new PointerEvent(type, o));
  if (type === 'pointerdown') window.__finger = t;
  if (type === 'pointerup') window.__finger = null;
}"""


class Page:
    def __init__(self, port, width, height, touch):
        self.pw = sync_playwright().start()
        self.browser = self.pw.webkit.launch()
        kw = dict(self.pw.devices["iPhone 13"]) if touch else {"viewport": {"width": width, "height": height}}
        if touch:
            kw["viewport"] = {"width": width, "height": height}
        self.ctx = self.browser.new_context(**kw)
        self.events = []
        self.page = self._tab()
        self.touch = touch
        self.size = (width, height)
        self.mx, self.my = width // 2, height // 2

    def _tab(self):
        page = self.ctx.new_page()
        page.on("pageerror", lambda e: self.events.append(str(e)[:200]))
        page.on("console", lambda m: m.type == "error" and self.events.append(m.text[:200]))
        return page

    def renew(self):
        """A fresh tab in the same context (same saves), as drv.Page.renew; console errors so far are kept. The
        caller reopens the page."""
        old = self.page
        self.page = self._tab()
        old.close()

    def call(self, method, **p):
        if method == "Input.dispatchMouseEvent" and p.get("type") == "mouseWheel":
            self._wheel(p["x"], p["y"], p["deltaY"])
            return {}
        raise RuntimeError(f"{method} is CDP only")

    def _wheel(self, x, y, dy):
        """Mobile WebKit has no mouse wheel: scroll the nearest scrollable ancestor under the point."""
        self.page.evaluate("""([x, y, dy]) => { let e = document.elementFromPoint(x, y);
          while (e && e !== document.body && !(/(auto|scroll)/.test(getComputedStyle(e).overflowY) && e.scrollHeight > e.clientHeight)) e = e.parentElement;
          (e && e !== document.body ? e : document.scrollingElement).scrollBy(0, dy); }""", [x, y, dy])

    def js(self, expr):
        return self.page.evaluate("s => (0, eval)(s)", expr)

    def open(self, url):
        self.page.goto(url, wait_until="load")
        time.sleep(0.5)

    def shot(self, path):
        self.page.screenshot(path=path)

    def errors(self):
        return list(self.events)

    def settle(self, secs=0.25):
        self.page.wait_for_timeout(int(secs * 1000))

    def _fire(self, kind, x, y):
        self.page.evaluate(_DISPATCH, [kind, x, y])

    def move(self, x, y, steps=1):
        for i in range(1, steps + 1):
            px, py = self.mx + (x - self.mx) * i / steps, self.my + (y - self.my) * i / steps
            if self.touch:
                self._fire("pointermove", px, py)
            else:
                self.page.mouse.move(px, py)
            if steps > 1:
                time.sleep(0.016)
        self.mx, self.my = x, y

    def down(self):
        if self.touch:
            self._fire("pointerdown", self.mx, self.my)
        else:
            self.page.mouse.down()

    def up(self):
        if self.touch:
            self._fire("pointerup", self.mx, self.my)
        else:
            self.page.mouse.up()

    def click(self, x, y, skip_reveal=True):
        # WEB67: a transform's reveal takes the tap that skips it (drv.py's click does the same).
        if skip_reveal and self.js("!!document.querySelector('.reveal:not(.done)')"):
            self.click(x, y, False)
            self.settle(0.5)
        if self.touch:
            self.mx, self.my = x, y
            self.page.touchscreen.tap(x, y)
        else:
            self.page.mouse.move(x, y)
            self.page.mouse.click(x, y)
        self.settle(0.12)

    def hover(self, x, y):
        if not self.touch:
            self.move(x, y, 4)
            self.settle(0.2)

    def drag(self, x1, y1, x2, y2, steps=14, release=True):
        self.mx, self.my = x1, y1
        if not self.touch:
            self.page.mouse.move(x1, y1)
        time.sleep(0.1); self.down(); time.sleep(0.1)
        self.move(x2, y2, steps); time.sleep(0.15)
        if release:
            self.up()
        self.settle(0.3)

    def wheel(self, x, y, dy, count, hz=60):
        for _ in range(count):
            self._wheel(x, y, dy)
            time.sleep(1 / hz)
        self.settle(0.3)

    def key(self, k):
        self.page.keyboard.press(k)
        self.settle(0.2)

    def type_text(self, s):
        self.page.keyboard.insert_text(s)
        self.settle(0.1)
