# API probes

Scripts that measure engine behaviour through the bridge. Their results feed the skill's
references (`skill/teardown-modding/references/`). Run them in a light built-in level:

```
python tools/mcp_call.py mp_test_start '{"mod":"builtin-villagordon","restart":true}'
```

| File | What it measures | How to run |
|---|---|---|
| `gen_allcalls.py` → `allcalls.lua` | Which functions the engine refuses on a side, which are internal-only or missing. Calls every documented function once with no arguments. **Stops the half it runs on** (wrong-side calls do that), so restart the test afterwards | `run_lua` with `file` on `server`, then on `client`; then collect `is not available in <side> script` from `log_0.txt` into `unavailable_<side>.txt`, save the two results as `allcalls_<side>.json` |
| `gen_getters.py` → `getters.lua` | Every read function with plausible arguments: works / error message / return types | `run_lua` with `file` on `server`, `client`, `draw`; save as `getters_<side>.json` |
| `mutators.lua`, `run_mutators.py` | About 80 state-changing calls with valid arguments on server, host client and joined client, plus what each side sees of the others' registry writes | `python tools/probe/run_mutators.py` |
| `bisect.py` | Runs the mutator probes one at a time and reports the first after which the game stops answering | `python tools/probe/bisect.py client` |
| `sync1.lua`, `sync2.lua`, `sync3.lua` | Registry / `shared` / entity propagation between server, host client and joined client | `sync1` on server, `sync2` on client and remote with `WHO` set, `sync3` on server |
| `state.lua` | The bridge's own registry keys | `run_lua` with `file` |

`tools/gen_api_index.py` turns the results plus the game's `script_defs.lua` into
`api-index.md`. `tools/mcp_call.py` calls the MCP server from a script.

Since the bridge mod guards the known wrong-side functions in its own Lua state, `allcalls.lua`
run through a current bridge reports them as ordinary errors instead of reaching the engine. To
re-measure from scratch (after a game update), empty the `CLIENT_ONLY` / `SERVER_ONLY` lists in
`mod/main.lua` first.
