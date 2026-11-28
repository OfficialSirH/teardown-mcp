"""Download Teardown's official modding documentation and convert it to Markdown the skill can
read and grep.

    python -I tools/fetch_official_docs.py

Sources (Tuxedo Labs):
  https://www.teardowngame.com/modding/            modding guide (Markdeep: Markdown in a page)
  https://www.teardowngame.com/modding/api.html    scripting API reference
  https://www.teardowngame.com/modding/api.xml     the same functions as data (checks the parse)
  https://www.teardowngame.com/modding/voxscript.html  Voxscript API (Lua that builds voxels in level XML)
  https://www.teardowngame.com/modding-mp/         multiplayer modding guide (Markdeep)
  https://www.teardowngame.com/animation.html      links four animation guides (Google Docs, exported as Markdown)

Output: skill/teardown-modding/references/official/ (gitignored: the text is Tuxedo Labs' and
this repository is public, so each machine generates its own copy). Re-run after a game update.

  README.md          what is where, version, date
  api/README.md      the API intro (Lua 5.1, server / client / shared, every callback) and the
                     categories with their functions
  api/<category>.md  every function of one category: signature, arguments, return values,
                     description, example, the official SERVER ONLY / CLIENT ONLY tag and the
                     side the bridge probes measured (tools/api_sides.py)
  voxscript/         the Voxscript API in the same form (no side lines)
  modding.md, multiplayer.md   the two guides; images/ their pictures (the material palette
                     chart among them)
"""
import datetime, html, os, re, shutil, sys, urllib.parse, urllib.request
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))  # -I leaves the script dir off sys.path
import api_sides

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "skill", "teardown-modding", "references", "official")
SITE = "https://www.teardowngame.com/"
GUIDES = [  # (url, output file, image folder)
    (SITE + "modding/", "modding.md", "modding"),
    (SITE + "modding-mp/index.html", "multiplayer.md", "multiplayer"),
]
# The animation guides linked from https://www.teardowngame.com/animation.html: Google Docs,
# exported as Markdown. (document id, output file under animation/)
ANIMATION = [
    ("1UbCJN4-jwWsd2-s39SwEqNpZAo_fiYKf74Pqse2CP0g", "guide.md"),
    ("16bJGONb6ll6BPtft0q3ndm-9J7KRnRMq2EpfnMvpE04", "vehicle-ik-rigging.md"),
    ("1IYQa9k6N8n5F4TH6DJstEmUTBsUOXXDXeS-Vrbp_Vus", "custom-tools.md"),
    ("16jm-WF1kRDjzVlHXSW4qYDWM2plZARwMqtsdOC7vckM", "character-mod.md"),
]


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "teardown-mcp-docs-fetch"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return r.read()


def slug(s):
    return re.sub(r"[^a-z0-9]+", "-", s.lower()).strip("-")


def write(rel, text):
    path = os.path.join(OUT, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(re.sub(r"\n{3,}", "\n\n", text).strip() + "\n")


# ---------------------------------------------------------------- HTML of api.html to Markdown

def inline(h):
    """Inline HTML (links, bold, line breaks) to Markdown text."""
    h = re.sub(r"<a\s+href=['\"]#(\w+)['\"][^>]*>(.*?)</a>", r"`\2`", h, flags=re.S)
    h = re.sub(r"<a\s+href=['\"]([^'\"]+)['\"][^>]*>(.*?)</a>", r"[\2](\1)", h, flags=re.S)
    h = re.sub(r"</?b>", "**", h)
    h = re.sub(r"<br\s*/?>", "\n", h)
    h = re.sub(r"<li>", "\n- ", h)
    h = re.sub(r"</?p>", "\n\n", h)
    h = re.sub(r"<[^>]+>", "", h)
    return html.unescape(h).replace("\xa0", " ")


def table(h):
    rows = []
    for tr in re.findall(r"<tr>(.*?)</tr>", h, re.S):
        cells = [re.sub(r"\s+", " ", inline(td)).strip().replace("|", "\\|")
                 for td in re.findall(r"<td[^>]*>(.*?)</td>", tr, re.S)]
        rows.append(cells)
    if not rows:
        return ""
    width = max(len(r) for r in rows)
    rows = [r + [""] * (width - len(r)) for r in rows]
    out = ["| " + " | ".join(rows[0]) + " |", "|" + "---|" * width]
    out += ["| " + " | ".join(r) + " |" for r in rows[1:]]
    return "\n\n" + "\n".join(out) + "\n\n"


def to_md(h):
    """A piece of api.html (paragraphs, tables, code, lists) to Markdown."""
    keep = []

    def stash(text):
        keep.append(text)
        return f"\x00{len(keep) - 1}\x00"

    h = re.sub(r"<pre[^>]*>(.*?)</pre>",
               lambda m: stash("\n\n```lua\n" + html.unescape(m.group(1)).strip("\n") + "\n```\n\n"), h, flags=re.S)
    h = re.sub(r"<table[^>]*>(.*?)</table>", lambda m: stash(table(m.group(1))), h, flags=re.S)
    t = inline(h)
    t = re.sub(r"\x00(\d+)\x00", lambda m: keep[int(m.group(1))], t)
    t = "\n".join(line.rstrip() for line in t.split("\n"))
    return re.sub(r"\n{3,}", "\n\n", t).strip()


# ---------------------------------------------------------------- api.html

def params(section):
    """Argument or return-value lines: [(name, type, description)]; [] for "none"."""
    out = []
    for m in re.finditer(r"<span class='(?:argname|retname)'>(.*?)</span>(.*?)(?:<br/>|$)", section, re.S):
        name = inline(m.group(1)).strip()
        rest = m.group(2)
        t = re.search(r"<span class='argtype'>\((.*?)\)</span>", rest, re.S)
        desc = re.sub(r"<span class='argtype'>.*?</span>", "", rest, flags=re.S)
        desc = re.sub(r"^\s*&ndash;\s*", "", desc.strip())
        if name == "none" and not t:
            continue
        out.append((name, html.unescape(t.group(1)) if t else "", re.sub(r"\s+", " ", inline(desc)).strip()))
    return out


def parse_api(src):
    version = re.search(r"<h1>.*?\(([\d.]+)\)</h1>", src).group(1)
    first = src.find("<a name='")
    heads = list(re.finditer(r"<h2>(.*?)</h2>", src[:first]))
    intro, cats = "", []
    for k, m in enumerate(heads):
        end = heads[k + 1].start() if k + 1 < len(heads) else first
        body = src[m.end():end]
        if k == 0 and "<a href='#" not in body:  # api.html opens with an intro, voxscript.html does not
            intro = to_md(body)
            continue
        names = re.findall(r"<a href='#(\w+)'>", body)
        cut = body.find("<a href='#")
        cats.append({"name": html.unescape(m.group(1)), "intro": to_md(body[:cut] if cut >= 0 else body), "names": names})

    funcs = {}
    parts = re.split(r"<a name='(\w+)'></a><h3 class='function'>", src[first:])
    for name, chunk in zip(parts[1::2], parts[2::2]):
        chunk = chunk.split("<hr/>")[0]
        head, _, body = chunk.partition("</h3>")
        tags = re.findall(r"<span class='functag'>(.*?)</span>", head)
        fd = re.search(r"<pre class='funcdef'>(.*?)</pre>", body, re.S)
        sig = html.unescape(re.sub(r"<[^>]+>", "", fd.group(1))).strip() if fd else name + "()"
        args = re.search(r"<p>Arguments<br/>(.*?)(?=<p>Return value<br/>)", body, re.S)
        rets = re.search(r"<p>Return value<br/>(.*?)(?=<p>|$)", body, re.S)
        rest = body[rets.end():] if rets else body[fd.end():] if fd else body
        examples = [html.unescape(e).strip("\n") for e in re.findall(r"<pre class='example'>(.*?)</pre>", rest, re.S)]
        rest = re.sub(r"<pre class='example'>.*?</pre>", "", rest, flags=re.S)
        funcs[name] = {
            "tags": tags, "sig": sig,
            "args": params(args.group(1)) if args else [],
            "rets": params(rets.group(1)) if rets else [],
            "text": to_md(rest), "examples": [e for e in examples if e.strip()],
        }
    return version, intro, cats, funcs


def side_line(name, tags, side):
    official = " / ".join(tags) if tags else "none (server and client)"
    s = side(name)
    measured = api_sides.MEANING[s]
    return f"**Side:** official tag: {official}. Measured by the bridge probes: {measured}."


def check_xml(xml_src, funcs):
    """Compare against api.xml: same functions, same number of inputs and outputs."""
    problems = []
    root = ET.fromstring(xml_src)
    seen = set()
    for f in root.iter("function"):
        n = f.get("name")
        seen.add(n)
        if n not in funcs:
            problems.append(f"{n}: in api.xml, not in api.html")
            continue
        ni, no = len(f.findall("input")), len(f.findall("output"))
        if ni != len(funcs[n]["args"]) or no != len(funcs[n]["rets"]):
            problems.append(f"{n}: api.xml {ni} in / {no} out, parsed {len(funcs[n]['args'])} / {len(funcs[n]['rets'])}")
    problems += [f"{n}: in api.html, not in api.xml" for n in funcs if n not in seen]
    return len(seen), problems


def write_api(version, intro, cats, funcs, side, date, folder="api", page="api.html", what="scripting API"):
    """One Markdown file per category in `folder`, plus folder/README.md. side=None: no side lines."""
    rows = []
    for c in cats:
        fname = slug(c["name"]) + ".md"
        rows.append(f"| {c['name']} | [{folder}/{fname}]({fname}) | {', '.join(c['names'])} |")
        out = [f"# {c['name']}", "",
               f"> Official Teardown {what} {version}, category \"{c['name']}\" "
               f"(from {SITE}modding/{page}, fetched {date}). Generated by tools/fetch_official_docs.py.",
               "", c["intro"], ""]
        for n in c["names"]:
            f = funcs.get(n)
            if not f:
                out += [f"### {n}", "", "(listed in the category, no entry on the page)", ""]
                continue
            out += [f"### {n}", "", "```lua", f["sig"], "```", ""]
            if side:
                out += [side_line(n, f["tags"], side), ""]
            for title, items in (("Arguments", f["args"]), ("Returns", f["rets"])):
                if items:
                    out.append(f"{title}:")
                    out += [f"- `{a}`" + (f" ({t})" if t else "") + (f": {d}" if d else "") for a, t, d in items]
                    out.append("")
                else:
                    out += [f"{title}: none", ""]
            if f["text"]:
                out += [f["text"], ""]
            for e in f["examples"]:
                out += ["Example:", "", "```lua", e, "```", ""]
        write(os.path.join(folder, fname), "\n".join(out))
    head = [f"# Teardown {what} {version} (official)", "",
            f"> From {SITE}modding/{page}, fetched {date}. Generated by tools/fetch_official_docs.py;",
            "> one file per category below." + (" Each function also says which side the bridge probes measured." if side else ""), ""]
    if intro:
        head += ["## Teardown scripting", "", intro, ""]
    write(os.path.join(folder, "README.md"), "\n".join(
        head + ["## Categories", "", "| Category | File | Functions |", "|---|---|---|", *rows, ""]))


# ---------------------------------------------------------------- the Markdeep guides

def guide(url, outname, imgdir, cat_of, date):
    src = get(url).decode("utf-8", errors="replace")
    base = url if url.endswith("/") else url.rsplit("/", 1)[0] + "/"
    src = re.sub(r"^\s*<meta[^>]*>", "", src)
    src = re.sub(r"<!--.*?-->|<script.*?</script>|<style.*?</style>|<link[^>]*>", "", src, flags=re.S)
    images = []

    def local_image(path):
        images.append(path)
        return f"images/{imgdir}/{os.path.basename(path)}"

    def link(target):
        if re.match(r"\w+://|#|mailto:", target):
            return target
        m = re.match(r"(?:\.\./modding/)?api\.html(?:#(\w+))?$", target)
        if m:
            fn = m.group(1)
            return f"api/{cat_of[fn]}.md#{fn.lower()}" if fn and fn in cat_of else "api/README.md"
        return urllib.parse.urljoin(base, target)

    # Markdown images first, then <img> tags (whose rewritten form must not be matched again).
    src = re.sub(r"!\[([^\]]*)\]\(((?!\w+://)[^)\s]+)\)", lambda m: f"![{m.group(1)}]({local_image(m.group(2))})", src)
    src = re.sub(r"<img\s+src=\"([^\"]+)\"(?:\s+alt=\"([^\"]*)\")?[^>]*>",
                 lambda m: f"![{m.group(2) or ''}]({local_image(m.group(1))})", src)
    src = re.sub(r"<a\s+href=\"([^\"]+)\"[^>]*>(.*?)</a>", lambda m: f"[{m.group(2).strip()}]({link(m.group(1))})", src, flags=re.S)
    src = re.sub(r"(?<!!)\[([^\]]*)\]\(((?!\w+://|images/)[^)\s]+)\)", lambda m: f"[{m.group(1)}]({link(m.group(2))})", src)
    src = "\n".join(line.rstrip() for line in src.split("\n"))
    title = "Teardown modding guide" if imgdir == "modding" else "Teardown multiplayer modding guide"
    write(outname, f"# {title} (official)\n\n> From {url}, fetched {date}. Generated by "
                   f"tools/fetch_official_docs.py; pictures are in images/{imgdir}/.\n\n{src}")
    got = 0
    for p in sorted(set(images)):
        dst = os.path.join(OUT, "images", imgdir, os.path.basename(p))
        try:
            data = get(urllib.parse.urljoin(base, p))
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            open(dst, "wb").write(data)
            got += 1
        except Exception as e:
            print(f"  image {p}: {e}")
    return got, len(set(images))


def animation_doc(doc_id, outname, date):
    """One animation guide: Markdown export of the Google Doc, its embedded images saved apart."""
    import base64
    url = f"https://docs.google.com/document/d/{doc_id}"
    src = get(url + "/export?format=md").decode("utf-8", errors="replace")
    stem = outname[:-3]
    images = {}
    for m in re.finditer(r"^\[(image\d+)\]: <data:image/(\w+);base64,([^>]*)>", src, re.M):
        name = f"{stem}-{m.group(1)[5:]}.{'jpg' if m.group(2) == 'jpeg' else m.group(2)}"
        path = os.path.join(OUT, "images", "animation", name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        open(path, "wb").write(base64.b64decode(m.group(3)))
        images[m.group(1)] = name
    src = re.sub(r"^\[image\d+\]: <data:.*$", "", src, flags=re.M)
    src = re.sub(r"!\[([^\]]*)\]\[(image\d+)\]",
                 lambda m: f"![{m.group(1)}](../images/animation/{images.get(m.group(2), m.group(2))})", src)
    write(os.path.join("animation", outname),
          f"> From {url} (linked from {SITE}animation.html), fetched {date}. Generated by "
          f"tools/fetch_official_docs.py; pictures in images/animation/.\n\n{src}")
    return len(images)


def main():
    date = datetime.date.today().isoformat()
    print("downloading api.html, api.xml")
    api_src = get(SITE + "modding/api.html").decode("utf-8", errors="replace")
    xml_src = get(SITE + "modding/api.xml")
    version, intro, cats, funcs = parse_api(api_src)
    side = api_sides.load()

    # Generated content only; empty the folder but keep it (a shell may have it open).
    for entry in os.listdir(OUT) if os.path.isdir(OUT) else []:
        path = os.path.join(OUT, entry)
        shutil.rmtree(path) if os.path.isdir(path) else os.remove(path)
    write_api(version, intro, cats, funcs, side, date)
    cat_of = {n: slug(c["name"]) for c in cats for n in c["names"]}
    listed = sum(len(c["names"]) for c in cats)
    n_xml, problems = check_xml(xml_src, funcs)
    print(f"API {version}: {len(cats)} categories, {listed} listed, {len(funcs)} entries parsed, {n_xml} in api.xml")
    for p in problems:
        print("  mismatch:", p)

    # Official side tags against the probes' measurements.
    disagree = []
    for n, f in funcs.items():
        s = side(n)
        if "SERVER ONLY" in f["tags"] and s not in ("server", "internal", "missing", "untested"):
            disagree.append(f"{n}: official SERVER ONLY, measured {s or 'both'}")
        if "CLIENT ONLY" in f["tags"] and s not in ("client", "draw", "internal", "missing", "untested"):
            disagree.append(f"{n}: official CLIENT ONLY, measured {s or 'both'}")
        if not f["tags"] and s in ("server", "client"):
            disagree.append(f"{n}: no official tag, measured {s} only")
    print(f"side tags: {sum(1 for f in funcs.values() if f['tags'])} tagged; {len(disagree)} differ from the probes")
    for d in disagree:
        print("  side:", d)

    files = ["api/README.md"] + [f"api/{slug(c['name'])}.md" for c in cats]
    print("downloading voxscript.html")
    v_version, _, v_cats, v_funcs = parse_api(get(SITE + "modding/voxscript.html").decode("utf-8", errors="replace"))
    write_api(v_version, "", v_cats, v_funcs, None, date, "voxscript", "voxscript.html", "voxscript API")
    files += ["voxscript/README.md"] + [f"voxscript/{slug(c['name'])}.md" for c in v_cats]
    print(f"voxscript {v_version}: {len(v_cats)} categories, {len(v_funcs)} functions")
    for url, outname, imgdir in GUIDES:
        print("downloading", url)
        got, total = guide(url, outname, imgdir, cat_of, date)
        files.append(outname)
        print(f"  {outname}: {got} of {total} images")
    for doc_id, outname in ANIMATION:
        print("downloading animation guide", outname)
        n = animation_doc(doc_id, outname, date)
        files.append("animation/" + outname)
        print(f"  animation/{outname}: {n} images")
    for name in ("sven_lagom_bonemap.xml", "nodes_template.xml"):  # example files the guide links
        dst = os.path.join(OUT, "animation", "guide-files", name)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        open(dst, "wb").write(get(SITE + "guide-files/" + name))
        files.append("animation/guide-files/" + name)

    write("README.md", "\n".join([
        "# Official Teardown documentation (generated copy)", "",
        f"Tuxedo Labs' modding documentation for Teardown {version}, fetched {date} by "
        "`tools/fetch_official_docs.py` and converted to Markdown. Not in git (the repository is "
        "public); run the script again to refresh it after a game update.", "",
        "- `api/README.md`: the API intro (Lua 5.1, the server / client / shared tables, every "
        "callback) and the categories with their functions.",
        "- `api/<category>.md`: every function in full, with the side the bridge probes measured. "
        "Grep `### Name` to find one.",
        "- `voxscript/`: the Voxscript API (Lua that builds voxels at level build time, "
        "`<voxscript>` in level XML), one file per category.",
        "- `modding.md`: the modding guide (mod types, info.txt, main.xml, options.lua, spawn.txt, "
        "editor, tags, layers, MagicaVoxel, palette and materials, skybox, scripting basics, limits).",
        "- `multiplayer.md`: the multiplayer guide (version 2 mods, game modes, mplib, standard "
        "level markup).",
        "- `animation/`: the animation guides linked from teardowngame.com/animation.html: "
        "`guide.md` (animated characters and objects: skeletons, clips, animator prefabs), "
        "`character-mod.md` (playable characters), `custom-tools.md` (tool prefabs and hand "
        "placement for third person), `vehicle-ik-rigging.md` (seating the player in vehicles).",
        "- `images/`: the guides' pictures (`images/modding/teardown_palette.png` is the material "
        "palette chart).", "",
        f"Files: {len(files)}. Functions: {len(funcs)}.", ""]))
    print("written to", OUT)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
