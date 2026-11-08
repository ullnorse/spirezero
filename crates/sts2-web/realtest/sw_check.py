"""Service worker check on a secure context (http://localhost): offline reload, a new build under an
open tab, cache pruning, the real clipboard, press feedback. Driven by sw-check.sh, which serves the
builds in turn. Prints one PASS/FAIL line per check. Env: CDP_PORT, OUT, SERVE (the script's argv[1] is
a command word: `step` below)."""
import json, os, subprocess, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drv import Page

PORT, CDP, OUT = os.environ["PORT"], int(os.environ["CDP_PORT"]), os.environ["OUT"]
URL = f"http://localhost:{PORT}/"
res = []
def check(name, ok, note=""):
    res.append(f"{'PASS' if ok else 'FAIL'} {name} {note}".rstrip())
    print(res[-1], flush=True)

def serve(which):  # restart the server on build `which` (www, www2, www3), or stop it
    subprocess.run(["systemctl", "--user", "stop", "web43-serve.service"], check=False)
    if which:
        subprocess.run(["systemd-run", "--user", "--unit=web43-serve", "--collect", "-p", "MemoryMax=1G", "env", f"PORT={PORT}", f"WWW={OUT}/{which}", os.environ["SERVE"]], check=True, capture_output=True)
        for _ in range(40):
            if subprocess.run(["curl", "-sf", "-o", "/dev/null", URL]).returncode == 0: break
            time.sleep(0.25)

def until(p, expr, secs=20):
    end = time.time() + secs
    while time.time() < end:
        if p.js(expr): return True
        time.sleep(0.25)
    return False

def build(p): return p.js("document.querySelector('base')?.href.match(/v\\/([0-9a-f]{12})/)?.[1]")
def caches(p): return sorted(p.js("caches.keys().then(k=>k.filter(x=>x.startsWith('sts2-b-')).map(x=>x.slice(7)))"))

p = Page(CDP, 390, 844, True)
for c in p.browser.call("Target.getBrowserContexts")["browserContextIds"]:
    p.browser.call("Browser.grantPermissions", permissions=["clipboardReadWrite", "clipboardSanitizedWrite"], origin=f"http://localhost:{PORT}", browserContextId=c)
serve("www")
p.open(URL)
check("secure context", p.js("isSecureContext") is True)
ok = until(p, "!!navigator.serviceWorker.controller") or (p.open(URL) or True) and until(p, "!!navigator.serviceWorker.controller")
check("worker controls the page", ok)
A = build(p)
check("shell cached once", until(p, f"caches.open('sts2-b-{A}').then(c=>c.match('/done')).then(Boolean)", 60), A)
until(p, "!!document.querySelector('#start .primary, #start button')", 30)
p.settle(1.5)
p.shot(f"{OUT}/sw-online-390x844.png")
n = p.js(f"caches.open('sts2-b-{A}').then(c=>c.keys()).then(k=>k.length)")
check("art/audio not cached wholesale", n < 120, f"{n} entries")

# Offline: the server is gone. A reload still gives the game.
serve(None)
p.open(URL); p.settle(1)
okoff = until(p, "!!document.querySelector('#start button, #start .primary')", 30)
check("offline reload plays", okoff and build(p) == A)
p.call("Emulation.setDeviceMetricsOverride", width=390, height=664, deviceScaleFactor=1, mobile=True)
p.settle(0.8)
p.shot(f"{OUT}/sw-offline-390x664.png")
p.call("Emulation.setDeviceMetricsOverride", width=390, height=844, deviceScaleFactor=1, mobile=True)

# Clipboard (secure context).
p.js("window.focus()")
r = p.js("import('./bugreport.js').then(m=>m.deliver('web43 clipboard test'))")
check("bug report copies via clipboard", r == "copied", str(r))

# Press feedback: a held touch dims the button and moves nothing.
btn = p.js("(()=>{const b=[...document.querySelectorAll('#start button')].find(b=>b.offsetParent&&b.disabled===false);if(!b)return null;const r=b.getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2,w:r.width,h:r.height,t:r.y}})()")
if btn:
    # headless Chrome does not set :active for emulated touches, so the press is the mouse button
    before = p.js("document.querySelector('#start').getBoundingClientRect().height")
    p.call("Input.dispatchMouseEvent", type="mousePressed", x=btn["x"], y=btn["y"], button="left", clickCount=1); time.sleep(0.3)
    after = p.js("document.querySelector('#start').getBoundingClientRect().height")
    f = p.js(f"getComputedStyle(document.elementFromPoint({btn['x']},{btn['y']}).closest('button')).filter")
    p.shot(f"{OUT}/sw-pressed-390x844.png")
    p.call("Input.dispatchMouseEvent", type="mouseMoved", x=1, y=1); p.call("Input.dispatchMouseEvent", type="mouseReleased", x=1, y=1, button="left", clickCount=1)
    check("press dims, layout still", f != "none" and before == after, f)
else:
    check("press dims, layout still", False, "no button")

# A new build while this tab is open.
serve("www2")
old_ok = p.js(f"fetch('/v/{A}/app.js').then(r=>r.ok)")
check("open tab keeps its own build (old files still served)", old_ok is True)
mod = p.js(f"import('/v/{A}/hover.js').then(()=>true).catch(()=>false)")
check("open tab lazy-loads its own build", mod is True)
p.open(URL); p.settle(1)
until(p, f"document.querySelector('base')&&!document.querySelector('base').href.includes('{A}')", 15)
B = build(p)
check("reload runs the new build", B and B != A, f"{A} -> {B}")
check("new shell cached; old one kept", until(p, f"caches.open('sts2-b-{B}').then(c=>c.match('/done')).then(Boolean)", 60) and set(caches(p)) == {A, B}, str(caches(p)))
serve("www3")
p.open(URL); p.settle(1)
until(p, f"document.querySelector('base')&&!document.querySelector('base').href.includes('{B}')", 15)
C = build(p)
until(p, f"caches.open('sts2-b-{C}').then(c=>c.match('/done')).then(Boolean)", 60); p.settle(1)
check("third build prunes the oldest", set(caches(p)) == {B, C}, str(caches(p)))
e = [x for x in p.errors() if "Failed to load resource" not in x]
check("no page errors", not e, "; ".join(e)[:200])
serve(None)
sys.exit(1 if any(r.startswith("FAIL") for r in res) else 0)
