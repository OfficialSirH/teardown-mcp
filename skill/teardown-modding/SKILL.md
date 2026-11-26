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
   returns when both players are in. `players: 1` opens only the host: still a multiplayer
   session, lighter and quicker, for work that does not need a second player; `mp_test_join`
   adds the client later (a late join) and `mp_test_stop` with `client_only` makes it leave. It needs no main game window. Single-player works too
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
   For performance, `perf_sample` measures frame times, CPU and memory; the engine profiler
   (opened with `send_input`, see bridge.md) shows what each script costs per callback.
   When the game crashes, hangs or stops answering: `game_health`, then `crash_report` or
   `dump_game` (bridge.md, "It crashed, hung or got slow").
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

- **Do not quit to the menu or load a level path you have not seen work.** In the main menu
  no script runs; only `screenshot` and `send_input` still work. In the single-player pause
  menu only `client.render` runs: the bridge answers `side: client` commands there, gives
  every other side a "paused" error at once, and `run_lua` with `side: client` and
  `SetPaused(false)` resumes the game (so does a click on Continue). The multiplayer test does
  not pause at all. `start_level` with
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
- **Two ways to crash the game outright:** a script deleting its own entity inside its own
  callback, and `Constrain*` called from `server.tick` instead of `server.update`.
- **No API calls at a script's top level** (while the file loads): only inside callbacks.
- **`Spawn` spawns only the first root element** of a prefab or XML string, and joints only
  attach to shapes from the same `Spawn` call unless `jointExisting` is true (which can also
  make them pick the wrong pair of shapes).
- **Dynamic bodies below about y = -20 freeze in mid-air** (put to sleep every step). Build
  anything with physics above that.
- **A `<vox>` without `pos` is centred on its body origin**, height included: a model built
  standing on z = 0 hangs half its height below the origin.
- **`server.postUpdate` / `client.postUpdate`** run once per physics step, after it.
- **`<screen>` works in multiplayer**: its version 1 script runs once, on the host, and every
  player sees that picture (in sync, operated by any player). For displays made of `DrawSprite` quads instead
  (authoring.md section 5): no two sprites may overlap, and keep 10 cm or more of air between
  a sprite and any voxel face behind it.
- **`ServerCall` does not say who called.** The player id is an ordinary argument any client
  can fake, so `IsPlayerHost(p)` on it proves nothing. For host-only actions let the host's
  client half write a registry key its server half reads; for per-player rights use a token
  sent with a targeted `ClientCall` (quirks.md section 4).
- **Effects the server calls are sent to every client, call by call.** Send one `ClientCall`
  per event and let clients make the particles, sounds and lights (authoring.md section 4).
  For a player's own action (firing a tool), play their feedback from their own client's
  input check at once, not after the server's call comes back (authoring.md section 10).
- **Events do not all reach joined clients:** `playerhurt` and `playerdied` do; `explosion`
  and events a server script posts with `PostEvent` reach only the host. `SetPlayerHealth(0)`
  posts no `playerdied` (damage through `ApplyPlayerDamage` does). Arguments: quirks.md,
  "Engine events".

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
  camera, weather, shooting, joints and triggers, a custom hand tool, a clickable menu, screens that work in multiplayer, finding and fixing what costs frame time. Read it before writing a
  new script, prefab or level.
- `references/quirks.md` - measured engine behaviour: script model, wrong-side calls, registry
  and `shared` propagation, entities and handles, files and the Lua sandbox, logs, the
  multiplayer test, what is still unverified.
- `references/bridge.md` - the MCP tools one by one, recipes (reload and check, inspect a
  mod's state, compare host and client, test UI code), failure messages and what to do.
- `references/unofficial-docs.md` - not verified, from the community "Unofficial TD Docs":
  the callbacks `script_defs.lua` leaves out (`render`, `handleCommand`, `destroy`), entity
  tags the engine reads (`unbreakable`, `interact`, `nocull`, vehicle tags...), registry
  keys, Voxscript, and the game's multiplayer game-mode library (mplib). Look there before
  guessing at a tag or registry key; measure before relying on it.

## Reporting

Say what you measured and what you only inferred. If a claim about engine behaviour did not
come from this skill's references, the API docs, or a probe you ran, mark it as a guess. When a
test needed the user to act in the game (resume, load a level), ask for exactly that.
