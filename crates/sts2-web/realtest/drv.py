"""CDP driver for realtest: real mouse, touch, wheel and key input into one Chrome tab (stdlib only).
Reuses the websocket client of ../cdp.py."""
import base64, json, os, time, urllib.request

_here = os.path.dirname(os.path.abspath(__file__))
_src = open(os.path.join(_here, "..", "cdp.py")).read().replace("\nmain()\n", "\n")
_ns = {"__name__": "cdp"}
exec(compile(_src, "cdp.py", "exec"), _ns)
Socket = _ns["Socket"]


class Page:
    def __init__(self, port, width, height, touch):
        # a browser context of its own (its own localStorage: the phone and the desktop run save separately), so
        # both can share one Chrome
        info = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json/version"))
        browser = Socket(info["webSocketDebuggerUrl"])
        self.port = port
        self.ctx = browser.call("Target.createBrowserContext")["browserContextId"]
        self.browser = browser
        self.target = None
        self.events = []
        self.touch = touch
        self.size = (width, height)
        self.attach()
        self.mx, self.my = width // 2, height // 2

    def attach(self):
        target = self.browser.call("Target.createTarget", url="about:blank", browserContextId=self.ctx)["targetId"]
        self.ws = Socket(f"ws://127.0.0.1:{self.port}/devtools/page/{target}")
        self.target = target
        self.call("Page.enable")
        self.call("Runtime.enable")
        self.emulate()

    def renew(self):
        """A fresh tab in the same context (same saves): a tab used for a whole run grows to Chrome's 4 GB
        cap and is killed. Keeps the old tab's console errors for `errors()`; the caller reopens the page."""
        old = self.target
        kept = [m for m in self.events if m.get("method") in ("Runtime.exceptionThrown", "Runtime.consoleAPICalled") and (m.get("method") == "Runtime.exceptionThrown" or m["params"]["type"] == "error")]
        try:
            self.ws.s.close()
        except Exception:
            pass
        self.events = kept
        self.attach()
        self.browser.call("Target.closeTarget", targetId=old)

    def emulate(self):
        w, h = self.size
        self.call("Emulation.setDeviceMetricsOverride", width=w, height=h, deviceScaleFactor=1, mobile=self.touch)
        self.call("Emulation.setFocusEmulationEnabled", enabled=True)
        self.call("Emulation.setTouchEmulationEnabled", enabled=self.touch, maxTouchPoints=5)

    def call(self, method, **p):
        self.ws.next += 1
        self.ws.send({"id": self.ws.next, "method": method, "params": p})
        while True:
            m = self.ws.recv()
            if m.get("id") == self.ws.next:
                if "error" in m:
                    raise RuntimeError(f"{method}: {m['error']}")
                return m.get("result", {})
            self.events.append(m)

    def js(self, expr):
        r = self.call("Runtime.evaluate", expression=expr, returnByValue=True, awaitPromise=True)
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"].get("exception", {}).get("description", "js error"))
        return r.get("result", {}).get("value")

    def open(self, url):
        self.call("Page.navigate", url=url)
        time.sleep(0.5)
        self.emulate()

    def shot(self, path):
        open(path, "wb").write(base64.b64decode(self.call("Page.captureScreenshot", format="png")["data"]))

    def errors(self):
        out = []
        for m in self.events:
            if m.get("method") == "Runtime.exceptionThrown":
                out.append(m["params"]["exceptionDetails"].get("exception", {}).get("description", "exception")[:200])
            elif m.get("method") == "Runtime.consoleAPICalled" and m["params"]["type"] == "error":
                out.append(" ".join(str(a.get("value", a.get("description", ""))) for a in m["params"]["args"])[:200])
        return out

    def settle(self, secs=0.25):
        end = time.time() + secs
        while time.time() < end:
            self.js("0")
            time.sleep(0.03)

    # -- pointer: a finger on a phone, a mouse on a desktop ------------------------------
    def _touch(self, kind, x, y):
        self.call("Input.dispatchTouchEvent", type=kind, touchPoints=[] if kind == "touchEnd" else [{"x": x, "y": y, "id": 1}])

    def move(self, x, y, steps=1):
        for i in range(1, steps + 1):
            px, py = self.mx + (x - self.mx) * i / steps, self.my + (y - self.my) * i / steps
            if self.touch:
                self._touch("touchMove", px, py)
            else:
                self.call("Input.dispatchMouseEvent", type="mouseMoved", x=px, y=py)
            if steps > 1:
                time.sleep(0.016)
        self.mx, self.my = x, y

    def down(self):
        if self.touch:
            self._touch("touchStart", self.mx, self.my)
        else:
            self.call("Input.dispatchMouseEvent", type="mousePressed", x=self.mx, y=self.my, button="left", clickCount=1)

    def up(self):
        if self.touch:
            self._touch("touchEnd", self.mx, self.my)
        else:
            self.call("Input.dispatchMouseEvent", type="mouseReleased", x=self.mx, y=self.my, button="left", clickCount=1)

    def click(self, x, y, skip_reveal=True):
        # WEB67: a transform's reveal covers the page and takes the tap that skips it, as a player's first tap would.
        if skip_reveal and self.js("!!document.querySelector('.reveal:not(.done)')"):
            self.click(x, y, False)
            self.settle(0.5)
        if self.touch:
            self.mx, self.my = x, y
        else:
            self.move(x, y, 3)
        time.sleep(0.04)
        self.down(); time.sleep(0.03); self.up()
        self.settle(0.12)

    def hover(self, x, y):
        if not self.touch:
            self.move(x, y, 4)
            self.settle(0.2)

    def drag(self, x1, y1, x2, y2, steps=14, release=True):
        if self.touch:
            self.mx, self.my = x1, y1
        else:
            self.move(x1, y1, 3)
        time.sleep(0.1); self.down(); time.sleep(0.1)
        self.move(x2, y2, steps); time.sleep(0.15)
        if release:
            self.up()
        self.settle(0.3)

    def wheel(self, x, y, dy, count, hz=60):
        """`count` wheel events of `dy` px (a touchpad: 4 px at 60 Hz; a mouse notch: 100 px)."""
        if self.touch:
            # a finger swipe has no wheel; the equivalent scroll gesture
            self.call("Input.synthesizeScrollGesture", x=x, y=y, yDistance=-dy * count, speed=4000, gestureSourceType="touch")
            return
        self.move(x, y, 2)
        for _ in range(count):
            self.call("Input.dispatchMouseEvent", type="mouseWheel", x=x, y=y, deltaX=0, deltaY=dy)
            time.sleep(1 / hz)
        self.settle(0.3)

    def key(self, k):
        code = {"Escape": 27, "Enter": 13, "ArrowUp": 38, "ArrowDown": 40, "ArrowLeft": 37, "ArrowRight": 39}.get(k, ord(k.upper()) if len(k) == 1 else 0)
        for t in ("keyDown", "keyUp"):
            self.call("Input.dispatchKeyEvent", type=t, key=k, windowsVirtualKeyCode=code, text=k if t == "keyDown" and len(k) == 1 else "")
        self.settle(0.2)

    def type_text(self, s):
        self.call("Input.insertText", text=s)
        self.settle(0.1)
