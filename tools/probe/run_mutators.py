"""Run mutators.lua on the server half, the client half, the server half again (to read back what
the client wrote), and the joined client's game; print a comparison table."""
import json
import os
import re
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from mcp_call import call  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__)).replace("\\", "/")
RUNS = [("server", "server1"), ("client", "client"), ("server", "server2"), ("remote", "remote")]


def run(side):
    code = f'SIDE = "{"client" if side == "remote" else side}" return dofile("{HERE}/mutators.lua")'
    r = call([("run_lua", {"side": side, "code": code})])[0]
    if not r["ok"]:
        at = call([("run_lua", {"side": "client", "code": 'GetString("savegame.mod.mcpprobe_at")'})])
        return {"FAILED": r["text"][:300] + " | last probe started: " + (at[0]["text"][:120] if at else "?")}
    rows = {}
    for row in r["data"]["returns"][0].split("\n"):
        k, v = row.split("|", 1)
        rows[k] = re.sub(r"^err\|.*?: (Error: )?", "ERR ", v)
    return rows


res = {}
for side, label in RUNS:
    res[label] = run(side)
    print(label, "FAILED: " + res[label]["FAILED"] if "FAILED" in res[label] else f"{len(res[label])} rows")
json.dump(res, open(os.path.join(HERE, "mutators_result.json"), "w"), indent=1)
keys = [k for k in res["client"] if "FAILED" not in res["client"]] or list(res["server1"])
print(f"{'probe':30} | {'SERVER':44} | {'CLIENT (host)':44} | REMOTE CLIENT")
for k in keys:
    if k.startswith("read"):
        continue
    print(f"{k:30} | {res['server1'].get(k, '?')[:44]:44} | {res['client'].get(k, '?')[:44]:44} | {res['remote'].get(k, '?')[:60]}")
print("--- reads: server1 | client | server2 | remote")
for k in keys:
    if k.startswith("read"):
        print(f"{k:34} | " + " | ".join(f"{res[l].get(k, '?')[:22]:22}" for _, l in RUNS))
