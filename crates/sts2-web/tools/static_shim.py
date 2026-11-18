"""Serves an export_static.sh tree the way Cloudflare Pages would, minimally: `_redirects` (splat rules, status),
`_headers` (path globs, Cache-Control), a plain file or directory index otherwise, 404.html for the rest.
Usage: static_shim.py <dir> <port> <bind>. Stdlib only; for checking the export, not for serving it."""
import fnmatch
import http.server
import os
import re
import sys
import urllib.parse


def rules(path):
    out = []
    with open(path) as f:
        for line in f:
            p = line.split()
            if len(p) >= 2 and not line.startswith("#"):
                out.append((p[0], p[1], int(p[2]) if len(p) > 2 else 301))
    return out


def headers(path):
    out, cur = [], None
    with open(path) as f:
        for line in f:
            if line.strip() and not line[0].isspace():
                cur = (line.strip(), {})
                out.append(cur)
            elif cur and ":" in line:
                k, v = line.split(":", 1)
                cur[1][k.strip()] = v.strip()
    return out


class H(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm",
                      ".js": "text/javascript", ".tsv": "text/tab-separated-values", ".webmanifest": "application/manifest+json"}

    def __init__(self, *a, **k):
        super().__init__(*a, directory=ROOT, **k)

    def redirect(self, path):
        for src, dst, code in RULES:
            pat = re.escape(src).replace(":build", "[^/]+").replace(r"\*", "(.*)")
            m = re.fullmatch(pat, path)
            if m:
                return code, dst.replace(":splat", m.group(1) if m.groups() else "")
        return None

    def send_head(self):
        path = urllib.parse.urlsplit(self.path).path
        r = self.redirect(path)
        if r:
            self.send_response(r[0])
            self.send_header("Location", r[1])
            self.end_headers()
            return None
        fs = self.translate_path(path)
        if os.path.isdir(fs):
            fs = os.path.join(fs, "index.html")
        if not os.path.isfile(fs):
            self.send_response(404)
            body = open(os.path.join(ROOT, "404.html"), "rb").read()
            self.send_header("Content-Type", "text/html")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            if self.command == "GET":
                self.wfile.write(body)
            return None
        self.hdr = {}
        for pat, h in HEADERS:
            if fnmatch.fnmatchcase(path, pat):
                self.hdr.update(h)
        return super().send_head()

    def end_headers(self):
        for k, v in getattr(self, "hdr", {}).items():
            self.send_header(k, v)
        super().end_headers()

    def log_message(self, *a):
        if self.path.startswith("/proof-done?"):
            print(urllib.parse.unquote(self.path.split("?", 1)[1]), file=sys.stderr, flush=True)


ROOT = os.path.abspath(sys.argv[1])
RULES, HEADERS = rules(os.path.join(ROOT, "_redirects")), headers(os.path.join(ROOT, "_headers"))
http.server.ThreadingHTTPServer((sys.argv[3], int(sys.argv[2])), H).serve_forever()
