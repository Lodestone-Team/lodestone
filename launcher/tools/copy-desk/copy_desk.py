#!/usr/bin/env python3
"""Copy Desk: edit the English copy of the Launcher and the Host in a clickable mock.

    python3 launcher/tools/copy-desk/copy_desk.py serve      # open the desk in a browser
    python3 launcher/tools/copy-desk/copy_desk.py status     # list the saved edits
    python3 launcher/tools/copy-desk/copy_desk.py apply      # write them into en/*.rs

Edits are saved to `edits.json` next to this file. `apply` writes the English of plain
strings and plurals into the source and removes them from `edits.json`; it leaves edits
it can't write safely (emptied strings, per-platform strings, `wording::` constants, or
strings whose source changed since the edit) for a hand edit. Translations (fr, zh_hans,
zh_hant) are never written here: redo them by hand or with the `apply-copy-edits` skill.
README.md has the details.
"""
import argparse
import json
import re
import sys
import threading
import time
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import unquote, urlparse

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from extract_strings import REPO, extract  # noqa: E402

PAGE = HERE / "page"
EDITS = HERE / "edits.json"
FONTS = REPO / "launcher" / "assets" / "fonts"
LOCK = threading.Lock()


def texts(s):
    if "forms" in s:
        return s["forms"]
    if "variants" in s:
        return [v["value"] for v in s["variants"]]
    return [s.get("value", "")]


def load_edits():
    if not EDITS.exists():
        return {}
    return json.loads(EDITS.read_text(encoding="utf-8") or "{}")


def save_edits(edits):
    if edits:
        EDITS.write_text(json.dumps(edits, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    elif EDITS.exists():
        EDITS.unlink()


# ---- serve -------------------------------------------------------------------------------


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):  # quiet: only errors
        pass

    def send(self, code, body, kind="application/json; charset=utf-8"):
        if isinstance(body, str):
            body = body.encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = unquote(urlparse(self.path).path)
        if path in ("/", "/index.html"):
            html = (PAGE / "index.html").read_text(encoding="utf-8")
            scripts = "\n".join(f'<script src="mock/{n}"></script>' for n in ("runtime.js", "launcher.js", "host.js"))
            return self.send(200, html.replace("<!--MOCK_SCRIPTS-->", scripts), "text/html; charset=utf-8")
        if path == "/strings.json":
            return self.send(200, json.dumps(extract(), ensure_ascii=False))
        if path == "/api/edits":
            with LOCK:
                return self.send(200, json.dumps(load_edits(), ensure_ascii=False))
        if path.startswith("/mock/fonts/"):
            name = Path(path).name
            found = next((p for p in FONTS.rglob(name) if p.is_file()), None)
            if found:
                return self.send(200, found.read_bytes(), "font/otf" if name.endswith(".otf") else "font/ttf")
            return self.send(404, "{}")
        if path.startswith("/mock/") and path.endswith(".js"):
            f = PAGE / "mock" / Path(path).name
            if f.is_file():
                return self.send(200, f.read_text(encoding="utf-8"), "text/javascript; charset=utf-8")
        return self.send(404, "{}")

    def do_POST(self):
        # {id, texts, original}: save one edit; texts null removes it.
        if urlparse(self.path).path != "/api/edits":
            return self.send(404, "{}")
        try:
            body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", 0))) or b"{}")
            sid = body["id"]
            assert isinstance(sid, str) and sid.count("/") == 3
        except Exception:
            return self.send(400, '{"error":"bad edit"}')
        with LOCK:
            edits = load_edits()
            if body.get("texts") is None:
                edits.pop(sid, None)
            else:
                edits[sid] = {"texts": list(map(str, body["texts"])), "original": list(map(str, body.get("original") or [])),
                              "updatedAt": int(time.time() * 1000)}
            save_edits(edits)
        return self.send(200, '{"ok":true}')


def serve(args):
    server = ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
    url = f"http://127.0.0.1:{server.server_port}/"
    print(f"Copy Desk: {url}\nEdits save to {EDITS.relative_to(REPO)}. Press Ctrl+C to stop.")
    if not args.no_open:
        threading.Timer(0.5, lambda: webbrowser.open(url)).start()
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass


# ---- status and apply -------------------------------------------------------------------


def rust_str(v):
    return '"' + v.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n") + '"'


STR = r'"(?:[^"\\]|\\.)*"'


def why_manual(s, e):
    """Why an edit can't be written automatically, or None."""
    if s is None:
        return "the string is gone from the source"
    if e.get("original") and e["original"] != texts(s):
        return "the source changed since the edit"
    if any(not t.strip() for t in e["texts"]):
        return "emptied: remove the string and its use in the code"
    if "ref" in s:
        return f"defined in {s.get('defined', s['ref'])}"
    if "variants" in s:
        return "has per-platform variants"
    have = set(re.findall(r"\{[a-zA-Z_]\w*\}", " ".join(texts(s))))
    want = set(re.findall(r"\{[a-zA-Z_]\w*\}", " ".join(e["texts"])))
    if have != want:
        return f"placeholders differ ({', '.join(sorted(have ^ want))})"
    return None


def status(args):
    edits = load_edits()
    if not edits:
        print("No edits saved.")
        return
    by_id = {s["id"]: s for s in extract()}
    rows = []
    for sid, e in sorted(edits.items()):
        s = by_id.get(sid)
        rows.append({"id": sid, "path": s and s["path"], "key": s and f"{s['enum']}::{s['key']}",
                     "current": s and texts(s), "new": e["texts"], "manual": why_manual(s, e)})
    if args.json:
        print(json.dumps(rows, ensure_ascii=False, indent=2))
        return
    for r in rows:
        print(f"{r['id']}  ({r['path']})")
        print(f"  now: {r['current']}\n  new: {r['new']}")
        if r["manual"]:
            print(f"  needs a hand edit: {r['manual']}")
    print(f"\n{len(rows)} edits.")


def write_one(s, new):
    path = REPO / s["path"]
    src = path.read_text(encoding="utf-8")
    start = src.index(f"pub enum {s['enum']} ")
    nxt = src.find("pub enum ", start + 1)
    end = nxt if nxt != -1 else len(src)
    block = src[start:end]
    if "forms" in s:
        pat = re.compile(r"(\n\s*%s\s*=\s*)\(\s*%s\s*,\s*%s\s*,?\s*\)" % (s["key"], STR, STR))
        repl = f"({rust_str(new[0])}, {rust_str(new[1])})"
    else:
        pat = re.compile(r"(\n\s*%s\s*=\s*)%s" % (s["key"], STR))
        repl = rust_str(new[0])
    block, n = pat.subn(lambda m: m.group(1) + repl, block)
    if n != 1:
        return False
    path.write_text(src[:start] + block + src[end:], encoding="utf-8")
    return True


def apply(args):
    edits = load_edits()
    by_id = {s["id"]: s for s in extract()}
    done, left = [], []
    for sid, e in sorted(edits.items()):
        s = by_id.get(sid)
        reason = why_manual(s, e)
        if reason is None and e["texts"] == texts(s):
            done.append(sid)  # nothing to change
            continue
        if reason is None and write_one(s, e["texts"]):
            done.append(sid)
            print(f"wrote  {s['path']}  {s['enum']}::{s['key']}")
        else:
            left.append(sid)
            print(f"skip   {sid}: {reason or 'not found in the source as expected'}")
    for sid in done:
        edits.pop(sid)
    save_edits(edits)
    if done:
        print("\nRedo the fr, zh_hans and zh_hant translations of the strings written above, then run")
        print("`cargo fmt` and `cargo test -p lodestone-launcher -p lodestone-host -p lodestone-ui` in launcher/.")
    if left:
        print(f"{len(left)} edits stay in {EDITS.relative_to(REPO)} for a hand edit.")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("serve", help="serve the desk on localhost")
    p.add_argument("--port", type=int, default=8765)
    p.add_argument("--no-open", action="store_true", help="don't open a browser")
    p.set_defaults(fn=serve)
    p = sub.add_parser("status", help="list the saved edits")
    p.add_argument("--json", action="store_true")
    p.set_defaults(fn=status)
    sub.add_parser("apply", help="write the English of the saved edits into the source").set_defaults(fn=apply)
    args = ap.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
