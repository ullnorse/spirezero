"""Headless proof driver (stdlib only): opens a URL in headless Chrome over the DevTools protocol
at a true <width>x<height> viewport (the window-size flag cannot go below ~500 px on macOS), then
saves a screenshot and prints the page's title.
Real time (default): waits until the title starts with `proof:` (or the timeout), for scenarios
that wait on the bot's Web Worker or time frames, which virtual time does not run.
Virtual time (`virtual_ms` > 0): runs the page for that much virtual time, deterministic and fast.
Usage: cdp.py <url> <out.png> <width>x<height> [timeout_s] [virtual_ms]"""
import base64
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request


class Socket:
    """A minimal websocket client: text frames out (masked), frames in."""

    def __init__(self, url):
        host, rest = url[len("ws://"):].split("/", 1)
        h, p = host.split(":")
        self.s = socket.create_connection((h, int(p)))
        key = base64.b64encode(os.urandom(16)).decode()
        self.s.sendall((f"GET /{rest} HTTP/1.1\r\nHost: {host}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                        f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        buf = b""
        while b"\r\n\r\n" not in buf:
            buf += self.s.recv(4096)
        self.rest = buf.split(b"\r\n\r\n", 1)[1]
        self.next = 0
        self.events = []

    def _read(self, n):
        while len(self.rest) < n:
            self.rest += self.s.recv(1 << 20)
        out, self.rest = self.rest[:n], self.rest[n:]
        return out

    def send(self, obj):
        data = json.dumps(obj).encode()
        head = bytes([0x81])
        n = len(data)
        if n < 126:
            head += bytes([0x80 | n])
        elif n < 65536:
            head += bytes([0x80 | 126]) + n.to_bytes(2, "big")
        else:
            head += bytes([0x80 | 127]) + n.to_bytes(8, "big")
        mask = os.urandom(4)
        self.s.sendall(head + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))

    def recv(self):
        msg = b""
        while True:
            b0, b1 = self._read(2)
            n = b1 & 0x7F
            if n == 126:
                n = int.from_bytes(self._read(2), "big")
            elif n == 127:
                n = int.from_bytes(self._read(8), "big")
            msg += self._read(n)
            if b0 & 0x80:
                return json.loads(msg)

    def call(self, method, **params):
        self.next += 1
        self.send({"id": self.next, "method": method, "params": params})
        while True:
            m = self.recv()
            if m.get("id") == self.next:
                return m.get("result", {})
            if "method" in m:
                self.events.append(m["method"])

    def wait_event(self, name, timeout):
        end = time.time() + timeout
        while name not in self.events and time.time() < end:
            self.s.settimeout(max(0.1, end - time.time()))
            try:
                m = self.recv()
            except socket.timeout:
                break
            if "method" in m:
                self.events.append(m["method"])
        self.s.settimeout(None)
        return name in self.events


def find_chrome():
    for name in ("google-chrome", "chromium", "chromium-browser"):
        path = shutil.which(name)
        if path:
            return path
    mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
    if os.path.exists(mac):
        return mac
    sys.exit("cdp.py: no Chrome found (google-chrome, chromium, or /Applications/Google Chrome.app)")


def main():
    url, out, size = sys.argv[1:4]
    timeout = float(sys.argv[4]) if len(sys.argv) > 4 else 120
    virtual = int(sys.argv[5]) if len(sys.argv) > 5 else 0
    w, h = (int(x) for x in size.split("x"))
    profile = tempfile.mkdtemp(prefix="cdp-", dir=os.path.dirname(os.path.abspath(out)))
    port = 9300 + os.getpid() % 500
    chrome = subprocess.Popen([find_chrome(), "--headless=new", "--mute-audio", "--no-sandbox", "--disable-gpu", "--hide-scrollbars",
                               f"--remote-debugging-port={port}", f"--user-data-dir={profile}", f"--window-size={w},{h}",
                               "about:blank"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        for _ in range(100):
            try:
                tabs = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json"))
                page = next(t for t in tabs if t["type"] == "page")
                break
            except Exception:
                time.sleep(0.1)
        ws = Socket(page["webSocketDebuggerUrl"])
        ws.call("Emulation.setDeviceMetricsOverride", width=w, height=h, deviceScaleFactor=3 if w < 600 else 1, mobile=w < 600)
        ws.call("Emulation.setTouchEmulationEnabled", enabled=w < 600, maxTouchPoints=5)
        title = ""
        if virtual:
            # Paused until the navigation has started, so the budget is spent on the page.
            ws.call("Emulation.setVirtualTimePolicy", policy="pause")
            ws.call("Page.navigate", url=url)
            ws.events.clear()
            ws.call("Emulation.setVirtualTimePolicy", policy="pauseIfNetworkFetchesPending", budget=virtual)
            ws.wait_event("Emulation.virtualTimeBudgetExpired", timeout)
            title = ws.call("Runtime.evaluate", expression="document.title", returnByValue=True)["result"].get("value", "")
            # A screenshot needs a frame, which a paused clock never produces (nor does an
            # unbudgeted "advance"): let a little virtual time pass.
            ws.call("Emulation.setVirtualTimePolicy", policy="advance", budget=200)
        else:
            ws.call("Page.navigate", url=url)
            end = time.time() + timeout
            while time.time() < end:
                title = ws.call("Runtime.evaluate", expression="document.title", returnByValue=True)["result"].get("value", "")
                if title.startswith("proof:"):
                    break
                time.sleep(0.25)
            time.sleep(0.4)
        shot = ws.call("Page.captureScreenshot", format="png")
        with open(out, "wb") as f:
            f.write(base64.b64decode(shot["data"]))
        print(title)
    finally:
        chrome.terminate()
        chrome.wait()
        # Chrome's helpers let go of the profile a moment after it exits.
        for _ in range(20):
            shutil.rmtree(profile, ignore_errors=True)
            if not os.path.exists(profile):
                break
            time.sleep(0.25)


main()
