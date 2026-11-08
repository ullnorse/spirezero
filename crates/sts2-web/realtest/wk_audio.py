"""WebKit: audio unlock by a touch tap and the m4a path. Run 1 as is; run 2 with canPlayType('audio/ogg; opus')
answering "" like older iOS Safari, which must make the page fetch and decode the .m4a files. Usage: URL"""
import sys
from playwright.sync_api import sync_playwright
url = sys.argv[1]
with sync_playwright() as pw:
    br = pw.webkit.launch()
    for name, init in (("as is", ""), ("ogg refused", "const o=HTMLMediaElement.prototype.canPlayType; HTMLMediaElement.prototype.canPlayType=function(t){return /ogg/.test(t)?'':o.call(this,t)};")):
        ctx = br.new_context(**pw.devices["iPhone 13"])
        if init: ctx.add_init_script(init)
        pg = ctx.new_page(); errs = []; pg.on("pageerror", lambda e: errs.append(str(e)[:150]))
        reqs = []; pg.on("request", lambda r: "/audio/" in r.url and reqs.append(r.url.rsplit("/", 1)[1]))
        pg.goto(url); pg.evaluate("() => localStorage.clear()"); pg.goto(url); pg.wait_for_timeout(1500)
        pg.touchscreen.tap(195, 120); pg.wait_for_timeout(4000)
        st = pg.evaluate("() => ({format: __sound.format(), bytes: __sound.bytes, tracks: __sound.tracks, missing: __sound.missing})")
        ctxstate = pg.evaluate("() => { const a = new (window.AudioContext||webkitAudioContext)(); return a.state; }")
        print(name, st, "files:", sorted(set(reqs))[:6], "errors:", errs, flush=True)
        ctx.close()
    br.close()
