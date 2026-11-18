---
name: teardown-modding
description: Develop, debug and test Teardown (voxel game by Tuxedo Labs) mods and Lua scripts, with a live game driven through the `teardown` MCP server. Use this skill whenever the work touches a Teardown mod - writing or porting Lua scripts (`#version 2`, `server.*` / `client.*` callbacks), level or prefab XML, multiplayer behaviour, "why does this script stop / do nothing" bugs, reloading a level to see a change, checking what the game is doing, or any question about what a Teardown API function does or where it may be called - even if the user never says "MCP" or "skill". Also use it before calling any `mcp__teardown__*` tool.
---

# Teardown modding with a live game

You have two things a mod author normally lacks: a bridge into a running game (the `teardown`
MCP server plus its in-game "MCP Bridge" mod) and a set of measured facts about the engine that
its documentation does not state. Use both. Guessing at engine behaviour is expensive here: a
wrong guess often fails silently, and each look at the result can cost a level reload of minutes.

## The loop

1. **Get a game.** If none is running in a level, start the local multiplayer test:
   `mp_test_start` with `mod` (a mod id or a folder name under `Documents/Teardown/mods`) and,
   to skip a mod's own menu level, `level`. It opens two windows (host + one client) and
   returns when both players are in. It needs no main game window. Single-player works too
   (replies then go through the savegame), but a level must be loaded first: by the user, or
   by clicking Play > Mod manager > the mod > Play > One Player with `send_input`, with "MCP
   Bridge" enabled there.
2. **Edit the mod's files.**
3. **`reload_scene`.** Scripts, XML and assets are read at level load, so nothing you edit
   shows until then. It returns only when the new scene runs; heavy mods take minutes, so do
   not shorten the timeout and do not poll with other tools meanwhile.
4. **Look.** `look_at` puts the player in front of what you changed, then `screenshot` (red
   text on screen = a Lua error), `read_log` with `file: log_0.txt`
   in the multiplayer test (Lua errors appear there with file and line), `run_lua` to read
   state, `registry_get` for the mod's `level.*` keys.
   To exercise input paths (a tool's fire button, a clickable menu, driving), `send_input`
   presses keys and clicks in the game window like a player; it takes window focus from the
   user while it runs, so use it deliberately.
   For performance (what a script costs, in which callback), open the engine profiler with
   `send_input` (see bridge.md) and read it from a `screenshot`.
5. Repeat. Batch what you can: one `batch` call with several actions costs one round trip.

Prefer measuring to reasoning. `run_lua` takes a `file` (absolute path, forward slashes): write
a small Lua file that returns what you want to know and run it, instead of long inline code.

## Where code runs (read this before any `run_lua`)

A `#version 2` script is two programs sharing one file: a **server half** (`server.init/tick/
update`) and a **client half** (`client.init/tick/update/draw`). They are separate Lua states:
no shared globals, even on the host. Every bridge command names where it runs with `side`:

| side | Runs in | Use for |
|---|---|---|
| `server` (default) | host's server half | world changes: spawn, delete, move bodies, player health/tools, damage, environment |
| `client` | host's client half | camera, local player view, sounds, sprites, client-side reads |
| `draw` | host's `client.draw()` | anything `Ui*`: those functions do not exist anywhere else |
| `remote` | the joined client's game | what a non-host player's game sees (synced keys, replicated entities). Multiplayer test only |

**The trap that matters most:** calling a function from the wrong half does not raise an error.
The engine logs `<fn> is not available in client script` (or `server script`) and then stops
that half of the calling script for the rest of the scene. `pcall` reports success. The script
just goes quiet. So before you write or run a call, check its side in
`references/api-index.md`. The bridge blocks the known cases in its own code and returns a
normal error naming the right side, but your mod's scripts have no such guard.

If a half of the bridge itself stops answering, the error message quotes the log line. Recover
with `reload_scene` from the other half (`side: client` or `server`), or
`mp_test_start` with `restart: true`.

`run_lua` runs inside the bridge mod's script. It sees the whole engine API, the registry and
the scene, but not another script's Lua variables, and not its `shared` table either (`shared`
is per script). To read a mod's internal state, have the mod write it to the registry
(`level.<mod>.<key>`), or temporarily add that to its script. Registry flags work the other way
too: a script that checks `GetBool("level.<mod>.<flag>")` can be driven with `registry_set`.

## Things that will otherwise cost you a reload

- **Do not pause, quit to the menu, or load a level path you have not seen work.** In
  single-player, scripts do not run in the pause menu or main menu; only `screenshot` and
  `send_input` still work there (a click on Continue or a second `esc` gets out of the pause
  menu). The multiplayer test does not pause at all. `start_level` with
  a bad path drops to the main menu.
- **Registry sync:** `SetString(key, value)` on the server is visible to the host's own client
  half but not to joined clients. Pass `true` as the third argument to sync. A joined client's
  writes never reach the server, synced or not. `savegame.mod.*` is per machine.
- **`shared`** is server-to-clients. A client can assign to it without error; the value stays
  on that client and reaches nobody.
- **Clients cannot modify entities the server created** (`SetBodyTransform`, `SetTag`,
  `SetLightEnabled`, ...): that one does raise an error, "invalid attempt to access server
  authored entity from client script". A client can `Spawn` its own entities; they get negative
  handles and exist only on that machine.
- **Player ids:** host is 1, the first client 2. `0` or omitted means "host" in server code and
  "the local player" in client code.
- **Testing multiplayer:** the multiplayer test compares each mod's files between host and
  client at start. Anything that changes a mod folder while it starts gets the client kicked.

The full list, with how each was measured, is in `references/quirks.md`. Read it before
designing anything that crosses the server/client line, and add to it when you measure
something new (see "Extending this knowledge" there).

## Reference files

- `references/api-index.md` - every documented function with its signature, the side it may be
  called from, and its line in `script_defs.lua`. Search it (grep the function name) rather
  than reading it whole. For the meaning of parameters, read the doc comment at that line in
  `<Teardown>/data/script_defs.lua`; the game's own scripts in `<Teardown>/data/script/` are
  the best usage examples.
- `references/authoring.md` - patterns that were built and run: a level that loads, shapes and
  a wheeled vehicle from XML, server logic with client presentation, HUD and world-space UI,
  camera, weather, shooting, joints and triggers, a custom hand tool, a clickable menu. Read it before writing a
  new script, prefab or level.
- `references/quirks.md` - measured engine behaviour: script model, wrong-side calls, registry
  and `shared` propagation, entities and handles, files and the Lua sandbox, logs, the
  multiplayer test, what is still unverified.
- `references/bridge.md` - the MCP tools one by one, recipes (reload and check, inspect a
  mod's state, compare host and client, test UI code), failure messages and what to do.

## Reporting

Say what you measured and what you only inferred. If a claim about engine behaviour did not
come from this skill's references, the API docs, or a probe you ran, mark it as a guess. When a
test needed the user to act in the game (resume, load a level), ask for exactly that.
