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
`batch`, `registry_get`, `registry_set`, `registry_clear`, `player_get`, `player_set`, `spawn`,
`delete_entities`, `find_entities`, `describe_entity`, `raycast`, `debug_print`, `screenshot`,
`read_log`, `mp_test_start`, `mp_test_stop`.

`mp_test_start` launches the game's local multiplayer test (host + client windows,
`teardown_modtest.exe`) for a mod, with the bridge loaded, and waits until the host answers.
`mod` is a mod id or a folder name under `Documents\Teardown\mods`; `level` picks a level XML
other than the mod's `main.xml` (for example to skip a mod's own menu level).

Most take `side`: `server` (default) or `client`. The mod is a `#version 2` script, so the host's
server and client halves are separate Lua states; both poll the inbox and each runs the commands
addressed to it, in order.

## Limits

- Mod scripts do not run in the pause menu or the main menu, and nothing can be done from outside
  to leave them. If the game stops answering, someone has to resume it by hand.
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
