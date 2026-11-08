"""WebKit (iPhone 13 profile) check of End Turn: both lines inside the plate at 320x568 and 390x664, and the
"Ask if energy is left" setting: the first tap shows "Energy left / tap again" inside the plate, and after 3 s
it is "End Turn" with its HP line again. Also which sound format this WebKit picked. Usage: wk_endturn.py URL OUT"""
import sys, os, json
from playwright.sync_api import sync_playwright

url, out = sys.argv[1], sys.argv[2]
INFO = """() => { const b = document.querySelector('#dock .fightbar .primary'); if (!b) return null;
  const r = b.getBoundingClientRect();
  const kids = [...b.childNodes].map((n) => { const rg = document.createRange(); rg.selectNodeContents(n); const q = rg.getBoundingClientRect(); return {t: n.textContent.trim(), l: q.left, r: q.right, t0: q.top, b: q.bottom}; }).filter((k) => k.t);
  return {text: b.innerText.replace(/\\n/g, ' / '), box: [r.left, r.top, r.right, r.bottom], kids, inside: kids.every((k) => k.l >= r.left - 0.5 && k.r <= r.right + 0.5 && k.t0 >= r.top - 0.5 && k.b <= r.bottom + 0.5), scrollW: [b.scrollWidth, b.clientWidth]}; }"""
with sync_playwright() as pw:
    br = pw.webkit.launch()
    for w, h in ((320, 568), (390, 664)):
        ctx = br.new_context(**{**pw.devices["iPhone 13"], "viewport": {"width": w, "height": h}})
        pg = ctx.new_page()
        pg.goto(url); pg.evaluate("() => { localStorage.clear(); localStorage.setItem('sts2-prefs', JSON.stringify({tutored: true, motion: 'full', confirmEnd: true})); }")
        pg.goto(url); pg.wait_for_timeout(1500)
        pg.touchscreen.tap(w / 2, 100)  # unlock audio like the first tap
        print(w, "sound format:", pg.evaluate("() => (window.__sound.format ? window.__sound.format() : 'n/a')"), "canPlay ogg opus:", repr(pg.evaluate("() => document.createElement('audio').canPlayType('audio/ogg; codecs=\"opus\"')")), "m4a:", repr(pg.evaluate("() => document.createElement('audio').canPlayType('audio/mp4; codecs=\"mp4a.40.2\"')")))
        r = pg.query_selector("#start input").bounding_box(); pg.touchscreen.tap(r["x"] + 5, r["y"] + 5); pg.keyboard.insert_text("WEBTEST1")
        r = pg.query_selector("#start .primary").bounding_box(); pg.touchscreen.tap(r["x"] + r["width"] / 2, r["y"] + r["height"] / 2); pg.wait_for_timeout(1200)
        for _ in range(14):
            if pg.query_selector("#dock .fightbar .primary"): break
            n = pg.query_selector("#screen g.node.reach") or pg.query_selector("#screen [data-bid]:not([disabled]):not(.disabled)") or pg.query_selector("#dock .primary:not([disabled])")
            if not n: pg.wait_for_timeout(500); continue
            n.evaluate("e => e.scrollIntoView({block: \"nearest\"})"); b = n.bounding_box(); pg.touchscreen.tap(b["x"] + b["width"] / 2, b["y"] + b["height"] / 2); pg.wait_for_timeout(1200)
        pg.wait_for_timeout(2500)
        print(w, "decision", pg.evaluate("() => sts2.view()?.decision"), pg.evaluate("() => document.querySelectorAll('#screen g.node').length"), pg.evaluate("() => document.querySelectorAll('#screen g.node.reach').length")); pg.screenshot(path=f"{out}/endturn-{w}-pre.png")
        a = pg.evaluate(INFO)
        print(w, "end turn:", json.dumps(a))
        pg.screenshot(path=f"{out}/endturn-{w}-a.png")
        e = pg.evaluate("() => sts2.view()?.screen && JSON.stringify(sts2.view().screen.blocks.find((x) => x.You)?.You?.energy)")
        b0 = pg.query_selector("#dock .fightbar .primary").bounding_box()
        pg.touchscreen.tap(b0["x"] + b0["width"] / 2, b0["y"] + b0["height"] / 2); pg.wait_for_timeout(300)
        asked = pg.evaluate(INFO); pg.screenshot(path=f"{out}/endturn-{w}-asked.png")
        print(w, "energy", e, "after 1st tap:", json.dumps(asked))
        pg.wait_for_timeout(3300)
        back = pg.evaluate(INFO); print(w, "after 3.3 s:", json.dumps(back)); pg.screenshot(path=f"{out}/endturn-{w}-back.png")
        ctx.close()
    br.close()
