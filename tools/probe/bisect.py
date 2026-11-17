"""Run the mutator probes one at a time on one side and stop at the first after which the
game no longer answers."""
import os, sys, time
sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mcp_call import call
HERE = os.path.dirname(os.path.abspath(__file__)).replace("\\", "/")
side = sys.argv[1]
env = {"TEARDOWN_MCP_RESPONSE_TIMEOUT": "6"}
for i in range(1, 90):
    code = f'SIDE = "{side}" ONLY = {i} return dofile("{HERE}/mutators.lua")'
    r = call([("run_lua", {"side": side, "code": code})], env=env)[0]
    rows = (r["data"] or {}).get("returns", [""])[0].split("\n") if r["ok"] else []
    name = rows[-1].split("|")[0] if rows and not rows[-1].startswith("read") else "(none)"
    time.sleep(0.4)
    alive = call([("ping", {"side": side}), ("ping", {"side": "server"})], env=env)
    ok = [a["ok"] for a in alive]
    if not r["ok"] or not all(ok):
        print(f"probe {i} {name!r}: ran ok={r['ok']}, afterwards {side} answers={ok[0]}, server answers={ok[1]}")
        break
    if name == "(none)":
        print("all", i - 1, "probes passed with the game still answering")
        break
