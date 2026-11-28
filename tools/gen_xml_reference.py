"""Build a level/prefab XML reference from the game's own files, for the skill.

    python -I tools/gen_xml_reference.py ["C:/path/to/Teardown"]

There is no published XML reference: the editor's help window and property tooltips are the
documentation. Their text is stored as plain strings in teardown.exe; this script reads those
strings (data only, no code) and combines them with an inventory of every element and attribute
used in the game's own XML (data/ and the built-in mods/).

Output: skill/teardown-modding/references/game/xml.md (gitignored, like references/official/:
the text is Tuxedo Labs'). Re-run after a game update.

  1. Editor help: what each node type is, and the tags it reads.
  2. Editor property descriptions with their defaults, in the editor's order. The attribute
     names are not stored next to them, so match each description to an attribute of the
     node it follows (section 3 lists the attributes the game actually uses).
  3. Per element: how often the game uses it, its attributes with counts and example values,
     example files.
"""
import collections, datetime, os, re, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAME = sys.argv[1] if len(sys.argv) > 1 else r"C:\Program Files (x86)\Steam\steamapps\common\Teardown"
OUT = os.path.join(ROOT, "skill", "teardown-modding", "references", "game", "xml.md")


def strings(d, a, b):
    """Printable strings (2+ chars, NUL-terminated) between file offsets a and b."""
    return [(a + m.start(), m.group().decode("latin-1")) for m in re.finditer(rb"[\x20-\x7e\t]{2,}(?=\0)", d[a:b])]


def node_help(d):
    """The editor help window: 'Title', 'title', paragraphs... per topic and node type."""
    a = d.find(b"Nodes and properties\0")
    b = d.find(b"Used for wheels in vehicles")
    if a < 0 or b < 0:
        return []
    b = d.find(b"\0", b)
    ss = strings(d, a - 64, b + 1)
    topics, cur = [], None
    for i, (_, s) in enumerate(ss):
        nxt = ss[i + 1][1] if i + 1 < len(ss) else ""
        # A topic starts with its title followed by its id: "Bodies and shapes", "bodiesshapes".
        if len(s) < 30 and s[:1].isupper() and re.fullmatch(r"[a-z]{3,24}", nxt) and nxt[0] == s[0].lower():
            cur = [s, []]
            topics.append(cur)
            continue
        if cur and not (re.fullmatch(r"[a-z]{3,24}", s) and s[0] == cur[0][0].lower()):
            cur[1].append(s)
    return topics


def property_help(d):
    """Property tooltips with defaults, in order, from 'Only used to more easily locate item'."""
    a = d.find(b"Only used to more easily locate item")
    b = d.find(b"#version 2\0", a)
    if a < 0 or b < 0:
        return []
    out = []
    for _, s in strings(d, a, b):
        if len(s) <= 2 or s.startswith("Editor::") or re.fullmatch(r"[A-Za-z0-9]{40,}", s):
            continue
        is_value = bool(re.fullmatch(r"[-0-9. ]+|[a-z_/.]+\.(ogg|png|jpg|dds)|ball|medium|axis|mirrory", s))
        is_header = bool(re.fullmatch(r"[A-Z][A-Za-z]+", s)) and len(s) < 16
        if is_value and out and out[-1][0] == "desc":
            out[-1] = ("desc", out[-1][1], s)
        elif is_header:
            out.append(("head", s, ""))
        else:
            out.append(("desc", s, ""))
    return out


def xml_inventory():
    elems = collections.defaultdict(lambda: {"n": 0, "files": set(), "attrs": collections.defaultdict(collections.Counter), "parents": collections.Counter()})
    files = 0
    for top in ("data", "mods"):
        for dirpath, _, names in os.walk(os.path.join(GAME, top)):
            for name in names:
                if not name.endswith(".xml"):
                    continue
                path = os.path.join(dirpath, name)
                rel = os.path.relpath(path, GAME).replace("\\", "/")
                try:
                    text = open(path, encoding="utf-8", errors="replace").read()
                except OSError:
                    continue
                files += 1
                text = re.sub(r"<!--.*?-->", "", text, flags=re.S)
                stack = []
                for m in re.finditer(r"<(/?)([A-Za-z_][\w]*)([^>]*?)(/?)>", text):
                    close, tag, attrs, selfclose = m.groups()
                    if close:
                        while stack and stack.pop() != tag:
                            pass
                        continue
                    e = elems[tag]
                    e["n"] += 1
                    e["files"].add(rel)
                    if stack:
                        e["parents"][stack[-1]] += 1
                    for k, v in re.findall(r"([\w:.-]+)\s*=\s*\"([^\"]*)\"", attrs):
                        e["attrs"][k][v] += 1
                    if not selfclose:
                        stack.append(tag)
    return files, elems


def main():
    exe = os.path.join(GAME, "teardown.exe")
    d = open(exe, "rb").read()
    topics, props = node_help(d), property_help(d)
    files, elems = xml_inventory()
    date = datetime.date.today().isoformat()
    out = ["# Level and prefab XML reference (generated)", "",
           f"> Generated {date} by tools/gen_xml_reference.py from the game's editor help (strings in "
           "teardown.exe) and every XML file in the game's data/ and built-in mods/ "
           f"({files} files). Not in git: the help text is Tuxedo Labs'. Measured behaviour that "
           "contradicts it is in quirks.md.", "",
           "Sections: 1. what each node is (editor help); 2. property descriptions and defaults "
           "(editor tooltips, in editor order); 3. every element and attribute the game's own XML uses.", "",
           "## 1. Node types (editor help)", ""]
    for title, paras in topics:
        out += [f"### {title}", ""]
        for p in paras:
            out.append(("- " + p) if re.match(r"^[\w ]+: ", p) and len(p) < 160 else p)
            out.append("")
    out += ["## 2. Property descriptions (editor tooltips)", "",
            "In the editor's order: the common entity properties (name, template, tags, pos, rot) come "
            "first, then each node type's own. Headers appear only where the game stores them, so a run "
            "of descriptions belongs to the node whose attributes they describe (section 3 lists them). "
            "`default` is the editor's default value where it has one.", ""]
    for kind, text, default in props:
        if kind == "head":
            out += ["", f"**{text}**", ""]
        else:
            out.append(f"- {text}" + (f" (default `{default}`)" if default else ""))
    out += ["", "## 3. Elements and attributes the game uses", "",
            "Per element: uses in the game's XML, typical parents, and each attribute with its count and up to "
            "four common values. Search this for an attribute name before inventing one.", ""]
    for tag, e in sorted(elems.items(), key=lambda kv: -kv[1]["n"]):
        if e["n"] < 2 and not e["attrs"]:
            continue
        parents = ", ".join(f"{p} {c}" for p, c in e["parents"].most_common(4))
        ex = ", ".join(sorted(e["files"], key=len)[:2])
        out += [f"### `<{tag}>`", "", f"{e['n']} uses in {len(e['files'])} files; parents: {parents or '-'}; e.g. {ex}", ""]
        for k, vals in sorted(e["attrs"].items(), key=lambda kv: -sum(kv[1].values())):
            common = ", ".join(f"`{v[:40]}`" for v, _ in vals.most_common(4))
            out.append(f"- `{k}` ({sum(vals.values())}): {common}")
        out.append("")
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(out) + "\n")
    print(f"{len(topics)} help topics, {sum(1 for p in props if p[0] == 'desc')} property descriptions, "
          f"{len(elems)} elements from {files} XML files -> {OUT}")


if __name__ == "__main__":
    main()
