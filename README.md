# Teardown MCP

An MCP server plus an in-game mod ("MCP Bridge") that lets an MCP client drive a running
Teardown 2.x game: run Lua, query the scene, reload the level after editing another mod and wait
until it is back, take screenshots.

## How it talks to the game

Teardown reads `savegame.xml` once at startup and never again, so the save file only works in one
direction (tested in-game, 2026-11-16: outside edits are ignored, even across a level restart, and
overwritten on the game's next save).

| Direction                             | Channel                                                                                                                                                                                                                                                                                        |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| server → game                         | `%LOCALAPPDATA%\Teardown\mcp_inbox.lua`, a Lua table the mod polls with `loadfile` every 0.1 s                                                                                                                                                                                                 |
| game → server, normal game            | `savegame.mod.mcp_*` registry keys; the game writes them to `%LOCALAPPDATA%\Teardown\savegame.xml` within ~0.5 s and the server reads them there                                                                                                                                               |
| game → server, local multiplayer test | the test windows (`teardown_modtest.exe`) never write `savegame.xml`, so the mod puts each result into the window's log file (`log_0.txt`): a failed `LoadSound("MOD/<data>.ogg")` logs its path, and the path carries the result as hex. Padding lines push it through the log's write buffer |

The server picks the reply channel per call: the log when a window titled "TD - Host" exists,
otherwise the savegame. `TEARDOWN_MCP_REPLY=log` or `=save` forces one.

Save keys (under `<mod><local-mcp-bridge>` in the file):

| Key                            | Meaning                                                                  |
| ------------------------------ | ------------------------------------------------------------------------ |
| `mcp_session`                  | id of the server session the mod accepted                                |
| `mcp_ack`                      | id of the last executed command                                          |
| `mcp_r<id>_n`, `mcp_r<id>_<i>` | result of command `<id>`: chunk count and chunks (percent-encoded JSON)  |
| `mcp_waiting`                  | 1 from a scene-reloading command until the new scene runs                |
| `mcp_halt`                     | 1 while a scene load is pending (nothing runs in the dying scene)        |
| `mcp_scene`                    | number of scenes started; the server waits for it to grow after a reload |
| `mcp_level`                    | level path of the running scene                                          |

A reload command carries `wait_scene=true` in the inbox. The mod acknowledges it, sets
`mcp_waiting`, restarts the level, and the freshly loaded script clears the flag and bumps
`mcp_scene` on its third server tick. The server waits for that (default 1800 s, `timeout_secs`
per call, up to 7200). In the multiplayer test the new scene announces itself in the log instead.

## Setup

1. Build: `cargo build --release` in `server/`.
2. The mod folder `mod/` must be visible to the game as `Documents\Teardown\mods\mcp-bridge`
   (here: a directory junction). Enable "MCP Bridge" in the game's mod manager.
3. Register the server with your MCP client (`.mcp.json` here is an example).

The server writes `mod/mcp_config.lua` (the absolute path of the command file, which `loadfile`
needs). The file is not in the repository, and the mod's script does not load without it, so
start the server once before loading a level. The command file is kept outside the mod folder on
purpose: a multiplayer session hashes each mod on host and client, and a file changing inside the
folder gets the client kicked for a mod mismatch.

Environment variables: `TEARDOWN_DIR` (game folder), `TEARDOWN_MCP_MOD_DIR`, `TEARDOWN_SAVEGAME`,
`TEARDOWN_MCP_RESPONSE_TIMEOUT` (seconds without any answer before giving up, default 20).

## Tools

`ping`, `bridge_status`, `run_lua`, `call_api`, `reload_scene`, `start_level`, `game_command`,
`batch`, `registry_get`, `registry_set`, `registry_clear`, `player_get`, `player_set`, `look_at`,
`spawn`,
`delete_entities`, `find_entities`, `describe_entity`, `raycast`, `debug_print`, `screenshot`,
`read_log`, `send_input`, `mp_test_start`, `mp_test_stop`, `wait_for_players`. Each is described in
`skill/teardown-modding/references/bridge.md`.

Most take `side`, where the command runs:

| side | Runs in |
|---|---|
| `server` (default) | the host's server half |
| `client` | the host's client half |
| `draw` | the host's `client.draw()`, the only place `Ui*` functions exist |
| `remote` | the joined client's game (local multiplayer test only) |

The mod is a `#version 2` script, so the two halves are separate Lua states; each runs the
commands addressed to it, in order. `run_lua` takes `code` or a Lua `file`.

Calling an API function from the wrong half makes the engine stop that half of the script
instead of raising an error. The bridge replaces the 71 functions known to do this (measured,
lists in `mod/main.lua`) with ones that raise a normal error, so a wrong-side call through the
bridge fails cleanly.

`mp_test_start` launches the game's local multiplayer test (host + client windows,
`teardown_modtest.exe`) for a mod, with the bridge loaded, and waits until both players are in
the level. `mod` is a mod id or a folder name under `Documents\Teardown\mods`; `level` picks a
level XML other than the mod's `main.xml`.

## The skill

`skill/teardown-modding/` is a Claude skill: how to work with the bridge, an index of every API
function with the side it may be called from, and the engine behaviour measured through the
bridge. To install it for Claude Code, link or copy the folder to
`~/.claude/skills/teardown-modding`. `tools/probe/` holds the probes behind it.

## The playground

`playground/` is a small level mod used to try the API in real use: a hand-built buggy with a
server autopilot and a speed HUD, a target range with a turret, synced score and effects, a
"director" with an orbit camera, a weather fade and an overlay panel, a gate on a motorised
hinge opened by a trigger zone, a custom hand tool (the Blaster), and a clickable menu (M). Link it to
`Documents\Teardown\mods\mcp-playground` and start it with
`mp_test_start { "mod": "mcp-playground" }`. Each script is switched on through a registry
flag (`level.buggy.autopilot`, `level.range.fire`, `level.director.orbit`,
`level.director.dusk`, `level.gate.force`, `level.blaster.fire`). `tools/lua/` has small reader scripts for `run_lua`.

## Limits

- In single-player, mod scripts do not run in the pause menu or the main menu. `send_input` can
  click the game out of the pause menu (verified); everything else waits until a level runs
  again. The multiplayer test does not pause at all.
- `send_input` sends real keyboard and mouse events: it brings the game window to the front and
  the user loses keyboard focus while it runs.
- `start_level` with a path the engine cannot load drops the game to the main menu. A successful
  `start_level` has not been verified yet.
- `run_lua` runs inside the bridge's script: it has the whole engine API and the registry, but
  not other scripts' Lua globals.
- Only the host's game answers; a joined client's game ignores the inbox. Tested in the local
  multiplayer test (host + client windows), not in a real online session.
- In the multiplayer test every result adds about 6 KB of "Couldn't find sound bank" lines to
  the host window's log.
- `screenshot` takes `window`: "TD - Host", "TD - Client1" or "Teardown".
- Every result is a write to `savegame.xml` (the game rewrites the whole file).

## Testing by hand

`server/tests/call.ps1 '{"name":"ping","arguments":{}}'` sends tool calls over stdio.
