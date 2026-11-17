"""Call the Teardown MCP server from a script: one server process, any number of tool calls.

    python tools/mcp_call.py ping '{}' run_lua '{"code":"GetVersion()"}'
    python tools/mcp_call.py --out result.json run_lua '{"code":"return dofile(\"C:/x.lua\")"}'

Arguments come in pairs: tool name, JSON arguments. Prints each result; with --out, writes the
list of results (text parsed as JSON where possible) to a file instead.
"""
import json
import os
import subprocess
import sys

EXE = os.path.join(os.path.dirname(__file__), "..", "server", "target", "release", "teardown-mcp.exe")


def call(calls, exe=EXE, env=None):
    """calls: list of (tool, args dict). Returns list of dicts {ok, text, data}."""
    lines = [json.dumps({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                         "params": {"protocolVersion": "2025-06-18", "capabilities": {},
                                    "clientInfo": {"name": "mcp_call", "version": "0"}}})]
    for i, (tool, args) in enumerate(calls, 1):
        lines.append(json.dumps({"jsonrpc": "2.0", "id": i, "method": "tools/call",
                                 "params": {"name": tool, "arguments": args}}))
    e = dict(os.environ)
    e.update(env or {})
    p = subprocess.run([exe], input="\n".join(lines) + "\n", capture_output=True, text=True,
                       encoding="utf-8", env=e)
    out = []
    for line in p.stdout.splitlines():
        if not line.strip():
            continue
        msg = json.loads(line)
        if msg.get("id") in (0, None):
            continue
        res = msg.get("result", {})
        text = "\n".join(c.get("text", "") for c in res.get("content", []) if c.get("type") == "text")
        try:
            data = json.loads(text)
        except ValueError:
            data = None
        out.append({"ok": not res.get("isError", False), "text": text, "data": data})
    return out


def main():
    argv = sys.argv[1:]
    out_path = None
    if argv[:1] == ["--out"]:
        out_path, argv = argv[1], argv[2:]
    calls = [(argv[i], json.loads(argv[i + 1])) for i in range(0, len(argv), 2)]
    results = call(calls)
    if out_path:
        with open(out_path, "w", encoding="utf-8") as f:
            json.dump(results, f, indent=1)
        print(f"{len(results)} results -> {out_path}; ok: {[r['ok'] for r in results]}")
    else:
        for r in results:
            print("ok" if r["ok"] else "ERROR", r["text"])


if __name__ == "__main__":
    main()
