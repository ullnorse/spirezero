"""Static export of a web release for Cloudflare Pages (see export_static.sh). Stdlib only.
Usage: export_static.py [RELEASE_DIR] OUT_DIR | --placeholder OUT_DIR

Writes what serve.py computes at request time as plain files: `/` (index.html with the <base> and art
<meta>), `/v/<build>/...`, `/a/<art id>/...`, plus `_headers` and `_redirects`. <build> and <art id> come
from serve.py's own build() and art_id(), run with the release as the working directory."""
import os
import re
import shutil
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import serve  # noqa: E402  (its server only starts under __main__)

LIMIT = 25 * 1024 * 1024  # Pages' per-file limit
MAX_FILES = 20000
TEXTY = {".js", ".html", ".css", ".json", ".webmanifest", ".tsv", ".svg"}

PLACEHOLDER = """<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Private</title>
<style>html{background:#0e1015;color:#9aa0ad;font:16px system-ui,sans-serif}body{margin:0;min-height:100vh;display:grid;place-items:center}</style>
</head><body><p>Private.</p></body></html>
"""
NOT_FOUND = PLACEHOLDER.replace("<title>Private</title>", "<title>Not found</title>").replace("Private.", "Not found.")


def die(msg):
    print(f"export_static: {msg}", file=sys.stderr)
    sys.exit(1)


def walk(top):
    """(relative path, real path) of every visible file under top, links followed, like serve.py's build()."""
    for d, dirs, names in os.walk(top, followlinks=True):
        dirs[:] = sorted(x for x in dirs if not x.startswith("."))
        for n in sorted(names):
            if not n.startswith("."):
                p = os.path.join(d, n)
                yield os.path.relpath(p, top), p


def put(src, dst):
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(src, dst)  # copyfile follows links: the copy is a plain file


def write(path, text):
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w", encoding="utf-8") as f:
        f.write(text)


def placeholder(out):
    shutil.rmtree(out, ignore_errors=True)
    write(os.path.join(out, "index.html"), PLACEHOLDER)
    write(os.path.join(out, "404.html"), NOT_FOUND)
    write(os.path.join(out, "_headers"), "/*\n  Cache-Control: no-cache\n  X-Robots-Tag: noindex\n")
    print(f"placeholder: 3 files in {out}")


def export(rel, out):
    rel = os.path.realpath(rel)
    if not os.path.isfile(os.path.join(rel, "index.html")):
        die(f"{rel} has no index.html")
    out = os.path.abspath(out)
    if out == rel or rel.startswith(out + os.sep) or out.startswith(rel + os.sep):
        die("OUT_DIR and the release must not contain each other")
    os.chdir(rel)
    build, art = serve.build(), serve.art_id()
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)

    files, skipped = [], []  # (published path, source)
    for r, p in walk("."):
        r = r.replace(os.sep, "/")
        if r.startswith("art/"):
            pub = f"a/{art}/{r[4:]}"  # the page reads pictures from the <meta name="art">, never from /v/<build>/art/
        else:
            pub = f"v/{build}/{r}"
        (skipped if os.path.getsize(p) > LIMIT else files).append((pub, r, p))

    for pub, r, p in files:
        put(p, os.path.join(out, pub))

    # the root: index.html with the injected base, exactly as serve.py's entry branch writes it; the worker and
    # the manifest are registered by absolute address (app.js, index.html)
    with open("index.html", encoding="utf-8") as f:
        html = f.read().replace("<head>", f'<head>\n<base href="/v/{build}/">\n<meta name="art" content="/a/{art}/">', 1)
    if "<base href" not in html:
        die("index.html has no <head> to inject the base into")
    write(os.path.join(out, "index.html"), html)
    put("sw.js", os.path.join(out, "sw.js"))
    put("manifest.webmanifest", os.path.join(out, "manifest.webmanifest"))
    write(os.path.join(out, "404.html"), NOT_FOUND)  # without one Pages answers any unknown path with index.html

    # a skipped file that something still names is a broken page: fail
    names = {r: r for _, r, _ in skipped}
    bad = []
    if skipped:
        for r, p in walk("."):
            if os.path.splitext(r)[1] in TEXTY and os.path.getsize(p) < LIMIT:
                with open(p, encoding="utf-8", errors="replace") as f:
                    body = f.read()
                for s in names:
                    if s in body or re.search(r"(?<![\w.-])" + re.escape(os.path.basename(s)) + r"(?![\w.-])", body):
                        bad.append((r, s))
    for _, r, p in skipped:
        print(f"skipped (> 25 MiB): {r} ({os.path.getsize(p) / 1048576:.1f} MiB)")
    if bad:
        die("skipped files are referenced: " + ", ".join(f"{a} -> {b}" for a, b in bad))

    # what serve.py redirects, as Pages rules. Rules whose target pattern also matches the source would
    # loop (a stale build or art id, which serve.py sends to the current one), so those stay out: a new
    # deploy removes the old addresses.
    write(os.path.join(out, "_redirects"),
          f"/v/:build/art/* /a/{art}/:splat 302\n"  # serve.py answers these with the picture; nothing under /v/ holds art here
          f"/art/* /a/{art}/:splat 302\n"
          f"/text/* /v/{build}/text/:splat 302\n"
          f"/audio/* /v/{build}/audio/:splat 302\n")
    forever = "  Cache-Control: public, max-age=31536000, immutable\n"
    write(os.path.join(out, "_headers"),
          "/\n  Cache-Control: no-cache\n"
          "/index.html\n  Cache-Control: no-cache\n"
          "/sw.js\n  Cache-Control: no-cache\n"
          "/manifest.webmanifest\n  Cache-Control: no-cache\n"
          "/404.html\n  Cache-Control: no-cache\n"
          f"/v/*\n{forever}/a/*\n{forever}")

    total = 4 + len(files) + 2  # index, sw, manifest, 404, _headers, _redirects
    if total > MAX_FILES:
        die(f"{total} files; Pages allows {MAX_FILES}")
    size = sum(os.path.getsize(os.path.join(d, n)) for d, _, ns in os.walk(out) for n in ns)
    print(f"build {build}, art {art}: {total} files, {size / 1048576:.1f} MiB in {out}")


def main(argv):
    if len(argv) == 2 and argv[0] == "--placeholder":
        return placeholder(argv[1])
    if len(argv) == 1:
        argv = [os.path.expanduser("~/.cache/spirebot-data/current")] + argv
    if len(argv) != 2 or argv[0].startswith("-"):
        die("usage: export_static.sh [RELEASE_DIR] OUT_DIR | --placeholder OUT_DIR")
    export(*argv)


main(sys.argv[1:])
