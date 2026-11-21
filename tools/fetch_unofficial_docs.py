"""Download the data behind the Unofficial TD Docs site (https://githeadcrab.github.io/) and
list what it documents that references/api-index.md does not.

    python -I tools/fetch_unofficial_docs.py [out_dir]

The site is a React app: every page's content is compiled into one JavaScript bundle as
`JSON.parse('...')` literals (the functions/tags/registry data and the tag-group list). The
bundle is fetched from the site's GitHub repository (branch gh-pages, which is what the site
serves); nothing in it is executed, the string literals are decoded by hand.

Output (default %LOCALAPPDATA%\\Teardown\\unofficial_docs\\, outside the repository: the site has
no licence, so its text is not committed): docs.json (all sections), groups.json (tag
groups), summary.txt (per section and category: entry names, names missing from the index,
entries the site marks as undocumented).
"""
import json, os, re, sys, urllib.request

REPO = "GitHeadcrab/githeadcrab.github.io"
RAW = f"https://raw.githubusercontent.com/{REPO}/gh-pages/"
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
INDEX = os.path.join(ROOT, "skill", "teardown-modding", "references", "api-index.md")


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "teardown-mcp-docs-fetch"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def js_string_literals(src, key="JSON.parse('"):
    """Yield the decoded contents of every JSON.parse('...') literal in src."""
    simple = {"n": "\n", "t": "\t", "r": "\r", "b": "\b", "f": "\f", "0": "\0"}
    i = 0
    while True:
        i = src.find(key, i)
        if i < 0:
            return
        k = i + len(key)
        buf = []
        while True:
            ch = src[k]
            if ch == "\\":
                nx = src[k + 1]
                if nx == "x":
                    buf.append(chr(int(src[k + 2:k + 4], 16))); k += 4
                elif nx == "u":
                    buf.append(chr(int(src[k + 2:k + 6], 16))); k += 6
                elif nx in simple:
                    buf.append(simple[nx]); k += 2
                elif nx == "\n":
                    k += 2
                else:
                    buf.append(nx); k += 2
            elif ch == "'":
                break
            else:
                buf.append(ch); k += 1
        # 🔥-style pairs arrive as two surrogates: join them
        yield "".join(buf).encode("utf-16", "surrogatepass").decode("utf-16")
        i = k


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
        os.environ.get("LOCALAPPDATA", "."), "Teardown", "unofficial_docs")
    os.makedirs(out, exist_ok=True)

    manifest = json.loads(get(RAW + "asset-manifest.json"))
    bundle = manifest["files"]["main.js"].lstrip("/")
    try:
        commit = json.loads(get(f"https://api.github.com/repos/{REPO}/commits/gh-pages"))
        stamp = f"{commit['sha'][:8]} {commit['commit']['committer']['date']}"
    except Exception as e:  # rate limit or offline API: the raw files are enough
        stamp = f"(commit unknown: {e})"
    src = get(RAW + bundle).decode("utf-8")

    docs = groups = None
    for text in js_string_literals(src):
        d = json.loads(text)
        if isinstance(d, list) and d and isinstance(d[0], dict) and "categories" in d[0]:
            docs = d
        elif isinstance(d, list) and d and isinstance(d[0], dict) and "tags" in d[0]:
            groups = d
    if docs is None:
        sys.exit("no docs data found in the bundle: the site's build has changed, look at "
                 "the JSON.parse literals by hand")

    json.dump(docs, open(os.path.join(out, "docs.json"), "w", encoding="utf-8"), indent=1,
              ensure_ascii=False)
    if groups is not None:
        json.dump(groups, open(os.path.join(out, "groups.json"), "w", encoding="utf-8"),
                  indent=1, ensure_ascii=False)

    known = set()
    if os.path.exists(INDEX):
        known = set(re.findall(r"^\| `([A-Za-z_][\w.]*)\(", open(INDEX, encoding="utf-8").read(),
                               re.M))
    lines = [f"gh-pages {stamp}, bundle {bundle}", ""]
    for sec in docs:
        lines.append(f"## {sec['title']} (type {sec.get('type')}{', test page' if sec.get('test') else ''})")
        for cat in sec["categories"]:
            ch = cat.get("children", cat.get("articles", []))
            names = [e["name"] for e in ch]
            lines.append(f"- {cat['name']}: {len(ch)} entries")
            if sec.get("type") == "function":
                new = [n for n in names if n not in known]
                undoc = [e["name"] for e in ch if e.get("status") or e.get("removed")]
                if new:
                    lines.append(f"    not in api-index: {' '.join(new)}")
                if undoc:
                    lines.append(f"    status/removed noted: {' '.join(undoc)}")
            else:
                lines.append(f"    {' '.join(names)}")
        lines.append("")
    open(os.path.join(out, "summary.txt"), "w", encoding="utf-8").write("\n".join(lines))
    print(f"wrote {out}: docs.json, groups.json, summary.txt ({stamp})")


if __name__ == "__main__":
    main()
