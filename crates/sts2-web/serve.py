"""Static server for www/. Stdlib only. Usage: serve.py <dir> <port> <bind>

Repeat loads are free because the page's files have versioned addresses: `/` (index.html, never
cached beyond a revalidation) gets `<base href="/v/<build>/">`, so every address the page and its
modules, worker, wasm, stylesheets, art and text build from it lies under `/v/<build>/` and is sent
`immutable`. <build> is a hash of every file in www/ plus the identity of the linked-in art and text,
so it changes exactly when something does; a request for another build's address is a 404 (an old
tab never gets new files under old names, so no page ever mixes two builds). The plain addresses
(`/app.js`, `/art/...`) still work, revalidating by ETag.
Also: gzips wasm, js, css, json, html, svg, tsv and ttf for clients that accept it, and answers
If-None-Match with 304."""
import gzip
import hashlib
import http.server
import os
import re
import sys
import urllib.parse

PACKED = {".wasm", ".js", ".css", ".json", ".html", ".svg", ".webmanifest", ".tsv", ".ttf"}
LINKED = {"art", "text", "audio"}  # linked in from ~/.cache/spirezero-data; they change under the same names on an import
cache = {}  # path -> (mtime, gzipped bytes)
hashes = {}  # path -> (mtime_ns, size, sha1)
VERSIONED = re.compile(r"^/v/([0-9a-f]{12})/(.*)$")
ARTWISE = re.compile(r"^/a/([0-9a-f]{12})/(.*)$")  # the art's own addresses, named by art_id()


def file_hash(path):
    st = os.stat(path)
    hit = hashes.get(path)
    if not hit or hit[:2] != (st.st_mtime_ns, st.st_size):
        with open(path, "rb") as f:
            hit = hashes[path] = (st.st_mtime_ns, st.st_size, hashlib.sha1(f.read()).hexdigest())
    return hit[2]


def build():
    """Hash of everything the page can fetch: www/ itself, the art index and bundle, the text tables."""
    h = hashlib.sha1()
    for top, dirs, names in os.walk(".", followlinks=True):
        dirs[:] = sorted(d for d in dirs if not d.startswith(".") and not (top == "." and d in ("art", "audio")))
        for n in sorted(names):
            if not n.startswith("."):
                p = os.path.join(top, n)
                h.update(f"{p}\0{file_hash(p)}\0".encode())
    # Art: 1700 pictures. The index (name, file, size of each) and the bundle's stamp stand for them.
    if os.path.isfile("art/index.tsv"):
        h.update(file_hash("art/index.tsv").encode())
        for p in ("art/art.bundle",):
            if os.path.isfile(p):
                st = os.stat(p)
                h.update(f"{st.st_mtime_ns}{st.st_size}".encode())
    if os.path.isfile("audio/audio.json"):  # the sounds: their manifest stands for the files
        h.update(file_hash("audio/audio.json").encode())
    return h.hexdigest()[:12]


def art_id():
    """The art's identity: its index and its bundle's stamp. Pictures live at `/a/<art id>/...`,
    which survive deploys (a phone keeps them until the next art import)."""
    h = hashlib.sha1()
    if os.path.isfile("art/index.tsv"):
        h.update(file_hash("art/index.tsv").encode())
        if os.path.isfile("art/art.bundle"):
            st = os.stat("art/art.bundle")
            h.update(f"{st.st_mtime_ns}{st.st_size}".encode())
    return h.hexdigest()[:12]


class Handler(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map,
                      ".wasm": "application/wasm", ".js": "text/javascript", ".tsv": "text/tab-separated-values",
                      ".webmanifest": "application/manifest+json"}
    etag = None
    immutable = False

    def end_headers(self):
        self.send_header("Cache-Control", "public, max-age=31536000, immutable" if self.immutable else "no-cache")
        if self.etag:
            self.send_header("ETag", self.etag)
        super().end_headers()

    def send_bytes(self, body, kind, gz):
        if gz:
            body = gzip.compress(body, 6)
        self.send_response(200)
        self.send_header("Content-Type", kind)
        if gz:
            self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Vary", "Accept-Encoding")
        self.end_headers()
        if self.command == "GET":
            self.wfile.write(body)
        return None

    def send_head(self):
        url = urllib.parse.urlsplit(self.path)
        now = build()
        a = ARTWISE.match(url.path)
        if a:
            art = art_id()
            if a[1] != art:
                self.send_response(302)
                self.send_header("Location", f"/a/{art}/{a[2]}")
                self.end_headers()
                return None
            self.immutable = True
            self.path = "/art/" + a[2]
            url = urllib.parse.urlsplit(self.path)
        m = VERSIONED.match(url.path)
        if m:
            if m[1] != now:
                # A tab left open across a deploy still asks for pictures and text as it plays.
                if m[2].startswith(("art/", "text/", "audio/")):
                    self.send_response(302)
                    self.send_header("Location", f"/v/{now}/{m[2]}")
                    self.end_headers()
                    return None
                self.send_error(404, "another build")
                return None
            self.immutable = True
            self.path = "/" + m[2]
            url = urllib.parse.urlsplit(self.path)
        gz = "gzip" in self.headers.get("Accept-Encoding", "")
        path = self.translate_path(url.path)
        if os.path.isdir(path):
            path = os.path.join(path, "index.html")
        if not os.path.isfile(path):
            return super().send_head()
        st = os.stat(path)
        entry = os.path.basename(path) == "index.html" and not m
        self.etag = f'"{now}"' if entry else f'"{st.st_mtime_ns:x}-{st.st_size:x}"'
        if self.headers.get("If-None-Match") == self.etag:
            self.send_response(304)
            self.end_headers()
            return None
        ext = os.path.splitext(path)[1]
        if entry:
            with open(path, encoding="utf-8") as f:
                html = f.read().replace("<head>", f'<head>\n<base href="/v/{now}/">\n<meta name="art" content="/a/{art_id()}/">', 1)
            return self.send_bytes(html.encode(), "text/html; charset=utf-8", gz)
        if ext not in PACKED or not gz:
            return super().send_head()
        hit = cache.get(path)
        if not hit or hit[0] != st.st_mtime_ns:
            with open(path, "rb") as f:
                hit = cache[path] = (st.st_mtime_ns, gzip.compress(f.read(), 9))
        body = hit[1]
        self.send_response(200)
        self.send_header("Content-Type", self.guess_type(path))
        self.send_header("Content-Encoding", "gzip")
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Vary", "Accept-Encoding")
        self.end_headers()
        if self.command == "GET":
            self.wfile.write(body)
        return None

    def log_message(self, *args):
        # The headless proof's real-time scenarios report here (`/proof-done?<title>`).
        if self.path.startswith("/proof-done?"):
            print(urllib.parse.unquote(self.path.split("?", 1)[1]), file=sys.stderr, flush=True)


os.chdir(sys.argv[1])
http.server.ThreadingHTTPServer((sys.argv[3], int(sys.argv[2])), Handler).serve_forever()
