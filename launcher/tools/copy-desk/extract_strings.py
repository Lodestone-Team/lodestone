"""Reads every English string of the Launcher, the Host and the shared UI.

Parses the `messages!` enums in `src/i18n/en/*.rs` of lodestone-launcher, lodestone-host
and lodestone-ui into a list of dicts: id ("app/file/Enum/Key"), the file it lives in,
its doc comment and section, and either `value`, `forms` (a plural: one, other) or
`variants` (per platform). Strings that name a `wording::` constant carry `defined`.
"""
import json, os, re, sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
ROOT = str(REPO / "launcher" / "crates")
APPS = [("launcher","lodestone-launcher","Lodestone Launcher"),("host","lodestone-host","Lodestone Host"),("shared","lodestone-ui","Shared (both apps)")]

def read_str(s, i):
    # s[i] == '"' ; returns (value, next_index)
    assert s[i] == '"'
    i += 1; out = []
    while True:
        c = s[i]
        if c == '"': return "".join(out), i+1
        if c == '\\':
            n = s[i+1]
            if n == 'n': out.append('\n'); i += 2
            elif n == 't': out.append('\t'); i += 2
            elif n == '"': out.append('"'); i += 2
            elif n == '\\': out.append('\\'); i += 2
            elif n == "'": out.append("'"); i += 2
            elif n == '0': out.append('\0'); i += 2
            elif n == 'u':
                j = s.index('}', i); out.append(chr(int(s[i+3:j],16))); i = j+1
            elif n == '\n':  # line continuation: skip newline and leading whitespace
                i += 2
                while s[i] in ' \t\n': i += 1
            else: raise ValueError(f"escape {n!r}")
        else: out.append(c); i += 1

def platform(cond):
    if not cond: return "Other systems"
    if "macos" in cond and "not" not in cond: return "macOS"
    if "windows" in cond: return "Windows"
    if "linux" in cond: return "Linux"
    return cond

def parse(path=None, text=None):
    s = text if text is not None else open(path, encoding="utf-8").read()
    i = 0; n = len(s); enum = None; section = ""; doc = []; items = []
    while i < n:
        if s.startswith("//!", i):
            i = s.index("\n", i) + 1; continue
        if s.startswith("///", i):
            j = s.index("\n", i); doc.append(s[i+3:j].strip()); i = j+1; continue
        if s.startswith("//", i):
            j = s.index("\n", i); line = s[i+2:j].strip()
            m = re.match(r"=+\s*(.*?)\s*=+$", line)
            if m: section = m.group(1)
            i = j+1; continue
        m = re.compile(r"pub enum (\w+)\s*\{").match(s, i)
        if m: enum = m.group(1); section = ""; doc = []; i = m.end(); continue
        m = re.compile(r"([A-Z]\w*)\s*=\s*").match(s, i)
        if m and enum:
            key = m.group(1); i = m.end()
            while s[i] in " \t\n": i += 1
            if s[i] == '"':
                v, i = read_str(s, i); items.append(dict(enum=enum, key=key, section=section, doc=" ".join(doc), value=v))
            elif s[i] == '(':
                i += 1; forms = []
                while True:
                    while s[i] in " \t\n,": i += 1
                    if s[i] == ')': i += 1; break
                    v, i = read_str(s, i); forms.append(v)
                items.append(dict(enum=enum, key=key, section=section, doc=" ".join(doc), forms=forms))
            elif s.startswith("if cfg!", i):
                variants = []
                while True:
                    cond = ""
                    if s.startswith("if cfg!", i):
                        j = i + len("if cfg!"); depth = 0; k = j
                        while True:
                            if s[k] == '(': depth += 1
                            elif s[k] == ')':
                                depth -= 1
                                if depth == 0: break
                            k += 1
                        cond = s[j+1:k]; i = k + 1
                    while s[i] in " \t\n{": i += 1
                    v, i = read_str(s, i)
                    while s[i] in " \t\n}": i += 1
                    variants.append({"when": platform(cond), "value": v})
                    if s.startswith("else", i):
                        i += 4
                        while s[i] in " \t\n": i += 1
                        continue
                    break
                items.append(dict(enum=enum, key=key, section=section, doc=" ".join(doc), variants=variants))
            else:
                m2 = re.compile(r"[\w:]+").match(s, i)
                ref = m2.group(0); i = m2.end()
                items.append(dict(enum=enum, key=key, section=section, doc=" ".join(doc), ref=ref))
            doc = []; continue
        if s[i] == '}' : enum_close = True
        if not s[i].isspace(): doc = [] if s[i] not in "#" else doc
        i += 1
    return items

def extract():
    """Returns every English string, freshly parsed from the source."""
    out = []
    for app, crate, label in APPS:
        d = f"{ROOT}/{crate}/src/i18n/en"
        for f in sorted(os.listdir(d)):
            if not f.endswith(".rs") or f == "mod.rs": continue
            path = f"{d}/{f}"
            head = [l[3:].strip() for l in open(path) if l.startswith("//!")]
            for it in parse(path):
                it.update(app=app, appLabel=label, file=f[:-3], fileDoc=" ".join(head), path=f"launcher/crates/{crate}/src/i18n/en/{f}")
                it["id"] = f"{app}/{f[:-3]}/{it['enum']}/{it['key']}"
                out.append(it)
    # constants from launcher-core's wording.rs, used by a few shared strings
    wsrc = open(f"{ROOT}/launcher-core/src/wording.rs", encoding="utf-8").read()
    for o in out:
        if "ref" in o and o["ref"].startswith("wording::"):
            name = o["ref"].split("::")[1]
            m = re.search(r"pub const " + name + r": &str = (.*?);\n", wsrc, re.S)
            body = re.sub(r"//[^\n]*", "", m.group(1))
            it = parse(text="pub enum W {\n " + name + " = " + body + ",\n}\n")[0]
            for k in ("value", "variants"):
                if k in it: o[k] = it[k]
            o["defined"] = "launcher/crates/launcher-core/src/wording.rs (" + name + ")"
    ids = [o["id"] for o in out]
    assert len(ids) == len(set(ids)), "duplicate ids"
    return out


if __name__ == "__main__":
    from collections import Counter
    out = extract()
    json.dump(out, open(sys.argv[1], "w") if len(sys.argv) > 1 else sys.stdout, ensure_ascii=False, indent=0)
    print(len(out), "strings;", sum(1 for o in out if "forms" in o), "plurals;",
          dict(Counter(o["app"] for o in out)), file=sys.stderr)
