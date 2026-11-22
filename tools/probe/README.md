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
| `tags_a.lua`, `tags_b.lua`, `tags_c.lua`, `tags_interact.lua` | What engine tags do (`unbreakable`, `inherittags`, `explosive`, `bomb`, `bounded`, `nonav`, `interact`, `customhealth`, `nomerge`) against untagged copies | spawn `probeevents.lua` (below) first; `run_lua` `file` `tags_a.lua` (server), wait 3 s, `tags_interact.lua` (client), `tags_b.lua` then `tags_c.lua` (server) |
| `tags2_a.lua`, `tags2_b.lua`, `playground/script/probedrive.lua` | Round 2 of tags: `passive`, `noroll`, `brokenthreshold`, `nodrive`, `invisible`, `booster`, `smoke`, `impactexplode`, `night` (the playground level also holds a `nomerge` pair, tags `mcpmerge` / `mcpnomerge`) | spawn `probeevents.lua` and `probedrive.lua`, `run_lua` `file` `tags2_a.lua` (server), wait 5 s, `tags2_b.lua`; `night` reads right one tick later |
| `playground/script/probeevents.lua`, `events_trigger.lua` | Every engine event each half sees, with its arguments; results under `level.probeev.<half>` | spawn it like `probecallbacks.lua`; `run_lua` `STEP = 1` then `file` `events_trigger.lua` (steps 1-3), read with `registry_get` on server, client and remote |
| `probemode/` (a global mod, linked into the game's mods folder as `mcp-probe-mode`) | A global game mode "MCP global probe" whose `init` / `destroy` write lines to the game log (`mcpglobalprobe_<what>_<ms>`) | `mp_test_start` with `game_mode: "local-mcp-probe-mode.mcp-global-probe"`, `extra_mods: ["local-mcp-probe-mode"]`; end it with `SetString("game.gamemode.next", "")` on the client side (no reload: no `wait_scene`); `grep mcpglobalprobe_` in `log_0.txt` / `log_1.txt` |
| `playground/script/probeswitches.lua` | Holds the registry switches listed in `level.probeswitch.on` (comma-separated) true every tick, like the game's own scripts; for `game.disablepause`, `hud.hide` etc. | spawn it like `probecallbacks.lua`, `SetString("level.probeswitch.on", "game.disablepause")` on the client side, `send_input` / `screenshot`, then `""` to stop |
| `playground/script/probeplayers.lua` | Ticks in which `GetAddedPlayers` / `GetRemovedPlayers` (and the include's `PlayersAdded` / `PlayersRemoved`) report players; results under `level.probeplayers.<half>` | `mp_test_start` with `players: 1`, spawn it, `mp_test_join`, `mp_test_stop` with `client_only: true`, `mp_test_join` again; `registry_get level.probeplayers` (depth 3) |
| `playground/voxscript/probe.lua`, `tools/lua/voxscript_probe.lua` | What a voxscript builds: shapes per `Vox`, `Box` bounds, units, XML parameters, materials, tags | the playground level places it twice (default and `<parameters size="12" mat="metal"/>`); `run_lua` `file` `voxscript_probe.lua` on server and remote. Runtime: `Spawn('<voxscript file="MOD/../mcp-playground/voxscript/probe.lua" ...>', t, true)` |
| `functions_docs.lua` | Community-docs claims about single functions: existence of removed names, `GetAddedPlayers`, `GetVehicleParams`, `GetBodyClosestPoint` with query filters | `run_lua` `file` on server, then client |
| `playground/script/probeuialpha.lua` | What `UiImageAlpha` / `UiImageOverlayAlpha` draw and return; results under `level.probeuialpha.returns`, effect in a screenshot | spawn it like `probecallbacks.lua`, `screenshot`, then `Delete` its handle |
| `playground/gamemodes.txt`, `playground/script/probemode.lua`, `tools/lua/gamemode_state.lua` | The playground's content game mode "MCP probe" (layer `mcpprobe`), recording its own init / destroy (`game.mcpprobemode.*` and log lines); the reader lists registered modes, the running one and the marks | `mp_test_start` with `game_mode: "local-mcp-playground.mcp-probe"` (layers come from `gamemodes.txt`; the level's `layer="-mcpprobe"` marker box, tag `mcpnomode`, is absent while the mode runs), or switch a running level with `SetString("game.gamemode.next", "local-mcp-playground.mcp-probe")` on the client side (`""` = no mode); `run_lua` `file` `gamemode_state.lua`, and `grep probemode_` in `log_0.txt` |
| `playground/script/probesprite.lua` | DrawSprite in the world: from which callback, how many per frame, depth test; settings and results under `level.probesprite` | spawn it like `probecallbacks.lua`, set `level.probesprite.*` with `registry_set` `side: client`, `screenshot`, `perf_sample` |
| `playground/script/probeuiwall.lua` | Ui* content mapped onto a world rectangle (UiRotate / UiScale fit to `UiWorldToPixel` corners, UiClipRect), against a DrawLine outline | spawn it like `probecallbacks.lua`, set `level.probeuiwall.*` on the client side, `screenshot` from several angles |
| `playground/script/probenet.lua` | What `ServerCall` tells the server about its caller (arguments only), spoofing an id, `ClientCall` to one player or all, server-side particles / lights / lines on joined clients; results under `level.probenet` | spawn it like `probecallbacks.lua`, set the triggers in its header with `registry_set` (`side: remote` for the joined client), read `level.probenet.who` (server) and `.got` (each client) |
| `playground/screen/probescreen.lua` | A version 1 `<screen>` script: where it runs in multiplayer (once, on the host), its canvas, input | spawn a screen with it (command in its header), `screenshot` host and client |
| `playground/script/probecallbacks.lua` | Which callbacks run and how often (`render` while paused), what `handleCommand` and `destroy` receive; results under `level.probecb.<half>` | spawn it from the bridge (server): `Spawn('<script file="MOD/../mcp-playground/script/probecallbacks.lua"/>', Transform())`, then trigger events and `registry_get level.probecb` (also with `side: remote`) |

`tools/gen_api_index.py` turns the results plus the game's `script_defs.lua` into
`api-index.md`. `tools/mcp_call.py` calls the MCP server from a script.

Since the bridge mod guards the known wrong-side functions in its own Lua state, `allcalls.lua`
run through a current bridge reports them as ordinary errors instead of reaching the engine. To
re-measure from scratch (after a game update), empty the `CLIENT_ONLY` / `SERVER_ONLY` lists in
`mod/main.lua` first.
