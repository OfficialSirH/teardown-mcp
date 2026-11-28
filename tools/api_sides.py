"""Which side each API function may be called from, as measured by the bridge probes in
tools/probe/ (see tools/probe/README.md). Shared by gen_api_index.py and fetch_official_docs.py.

    side = api_sides.load()
    side("SetBodyTransform")  # -> "server"

Values: "" (server and client), "server", "client", "draw" (only inside client.draw),
"internal" (mods cannot call it), "missing" (documented, not in the game), "untested".
"""
import json, os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROBE = os.path.join(ROOT, "tools", "probe")

# What each value means, for generated text.
MEANING = {
    "": "server and client",
    "server": "server only (from client code the engine stops the client half of the script)",
    "client": "client only (from server code the engine stops the server half of the script)",
    "draw": "only inside client.draw() (nil everywhere else)",
    "internal": "internal: raises \"function is only for internal usage\"; mods cannot use it",
    "missing": "documented but not present in the game",
    "untested": "not probed (destructive, session-level, or engine helper)",
}


def _words(name):
    path = os.path.join(PROBE, name)
    return set(open(path).read().split()) if os.path.exists(path) else set()


def _returns(name):
    path = os.path.join(PROBE, name)
    if not os.path.exists(path):
        return None
    return json.load(open(path))[0]["data"]["returns"][0]


def load():
    """Read the probe results once; returns side(name) -> one of the MEANING keys."""
    client_only = _words("unavailable_server.txt")
    server_only = _words("unavailable_client.txt") | {"SetGravity", "SetTimeScale"}
    internal, missing = set(), set()
    for half in ("server", "client"):
        d = _returns(f"allcalls_{half}.json")
        if d:
            internal |= set(d["internal"].split())
            missing |= set(d["missing"].split())
    draw = _returns("getters_draw.json")
    if draw:
        for row in draw.split("\n"):
            parts = row.split("|", 2)
            if len(parts) == 3 and "internal usage" in parts[2]:
                internal.add(parts[0])
    missing = {m for m in missing if not m.startswith(("Ui", "GetDisplay"))}
    path = os.path.join(PROBE, "allcalls_skipped.json")
    skipped = set(json.load(open(path))) if os.path.exists(path) else set()

    def side(name):
        if name in internal:
            return "internal"
        if name in missing:
            return "missing"
        if name.startswith("Ui"):
            return "draw"
        if name in client_only:
            return "client"
        if name in server_only:
            return "server"
        if name.endswith("CPP") or name in skipped:
            return "untested"
        return ""

    return side
