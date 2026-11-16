# Teardown MCP

An MCP server (Rust) plus an in-game mod ("MCP Bridge") that lets an MCP client drive a running
Teardown 2.x game: run Lua, query the scene, reload the level after editing another mod and wait
until it is back, take screenshots.

## How it talks to the game

Teardown reads `savegame.xml` once at startup and never again, so the save file only works in one
direction (tested in-game, 2026-11-16: outside edits are ignored, even across a level restart, and
overwritten on the game's next save).

| Direction | Channel |
|---|---|
| server → game | `mod/mcp_inbox.lua`, a Lua table the mod polls with `loadfile` every 0.1 s |
| game → server | `savegame.mod.mcp_*` registry keys; the game writes them to `%LOCALAPPDATA%\Teardown\savegame.xml` within ~0.5 s and the server reads them there |

Save keys (under `<mod><local-mcp-bridge>` in the file):

| Key | Meaning |
|---|---|
| `mcp_session` | id of the server session the mod accepted |
| `mcp_ack` | id of the last executed command |
| `mcp_r<id>_n`, `mcp_r<id>_<i>` | result of command `<id>`: chunk count and chunks (percent-encoded JSON) |
| `mcp_waiting` | 1 from a scene-reloading command until the new scene runs |
| `mcp_halt` | 1 while a scene load is pending (nothing runs in the dying scene) |
| `mcp_scene` | number of scenes started; the server waits for it to grow after a reload |
| `mcp_level` | level path of the running scene |

A reload command carries `wait_scene=true` in the inbox. The mod acknowledges it, sets
`mcp_waiting`, restarts the level, and the freshly loaded script clears the flag and bumps
`mcp_scene` on its third server tick. The server waits for that (default 600 s, `timeout_secs`
per call, up to 7200).

## Setup

1. Build: `cargo build --release` in `server/`.
2. The mod folder `mod/` must be visible to the game as `Documents\Teardown\mods\mcp-bridge`
   (here: a directory junction). Enable "MCP Bridge" in the game's mod manager.
3. Register the server with your MCP client (`.mcp.json` here is an example).

The server writes `mod/mcp_config.lua` (the mod folder's absolute path, which `loadfile` needs) at
startup. The file is not in the repository, and the mod's script does not load without it, so
start the server once before loading a level.

Environment variables: `TEARDOWN_MCP_MOD_DIR`, `TEARDOWN_SAVEGAME`,
`TEARDOWN_MCP_RESPONSE_TIMEOUT` (seconds without any answer before giving up, default 20).

## Tools

`ping`, `bridge_status`, `run_lua`, `call_api`, `reload_scene`, `start_level`, `game_command`,
`batch`, `registry_get`, `registry_set`, `registry_clear`, `player_get`, `player_set`, `spawn`,
`delete_entities`, `find_entities`, `describe_entity`, `raycast`, `debug_print`, `screenshot`,
`read_log`.

Most take `side`: `server` (default) or `client`. The mod is a `#version 2` script, so the host's
server and client halves are separate Lua states; both poll the inbox and each runs the commands
addressed to it, in order.

## Limits

* Mod scripts do not run in the pause menu or the main menu, and nothing can be done from outside
  to leave them. If the game stops answering, someone has to resume it by hand.
* `start_level` with a path the engine cannot load drops the game to the main menu. A successful
  `start_level` has not been verified yet.
* `run_lua` runs inside the bridge's script: it has the whole engine API and the registry, but
  not other scripts' Lua globals.
* Only the host's game answers; a joined client's game ignores the inbox. Multiplayer sessions
  are untested.
* Every result is a write to `savegame.xml` (the game rewrites the whole file).

## Testing by hand

`server/tests/call.ps1 '{"name":"ping","arguments":{}}'` sends tool calls over stdio.
