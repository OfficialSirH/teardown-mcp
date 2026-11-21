# The `teardown` MCP tools

The server drives a running game through the "MCP Bridge" mod. Commands go to the game through
a file the mod polls; answers come back through the mod's savegame data (normal game) or the
game's log file (multiplayer test). You do not need to care which: the server picks.

## Contents

1. Tools
2. Recipes
3. When a call fails
4. Limits

## 1. Tools

Most tools take `side` (`server` default, `client`, `draw`, `remote`): see SKILL.md.

| Tool | What it does |
|---|---|
| `mp_test_start` | Start the two-window multiplayer test for `mod` (id or folder name), optional `level` (XML relative to the mod folder), `restart: true` to replace a running test, `players: 1` for the host window alone (still a multiplayer session: two script halves, multiplayer rules; normal-size window, starts in about 13 s). Returns when the players are in the level |
| `mp_test_stop` | Close the test windows |
| `wait_for_players` | Wait until `count` players are in with their scripts running. Use it after anything that makes a client rejoin or reload, instead of reading logs |
| `ping` | Is the game answering; level path, players (server side: `ready` per player) |
| `bridge_status` | File paths and saved bridge state, without talking to the game |
| `reload_scene` | Restart the level and wait for the new scene. `side: client` if the server half is dead. `timeout_secs` default 1800 |
| `run_lua` | Run `code`, or a Lua `file` by absolute path. Returns return values and `print` output. An expression works: `GetPlayerHealth()` |
| `call_api` | Call one API function by name with JSON arguments |
| `batch` | Several actions in one round trip; commands after a `restart` run in the new scene. All other actions in a batch run in the **same game tick**: a `run_lua` "wait" between them lets no time pass, so to see physics or timers move, use separate calls |
| `registry_get` / `registry_set` / `registry_clear` | Read a key with its children (`depth`), write (`type`, `sync`), delete |
| `player_get` / `player_set` | Transform, camera, velocity, health, vehicle, tool; teleport / velocity / health |
| `look_at` | Teleport a player next to a `target` (position or entity handle) facing it, offset `from` (default `[-5, 2, 5]`). Use before `screenshot`. Server side |
| `send_input` | Real key presses, clicks, drags and mouse movement in the game window: play as a player would. Takes window focus while it runs |
| `find_entities` | By `tag` and/or `type` (body, shape, vehicle, light, location, joint, trigger, screen) |
| `describe_entity` | Type, tags with values, transform, type-specific data |
| `spawn` / `delete_entities` | Prefab path or XML string at a position; delete by handle |
| `raycast` | From `origin` along `dir`, or from the player's camera |
| `debug_print` | A line in the game's on-screen console |
| `screenshot` | A game window: `window` is "TD - Host" (default when present), "TD - Client1" or "Teardown" |
| `read_log` | End of a log file; `file` is `log.txt` (normal game, complete only after a normal exit), `log_0.txt` (test host), `log_1.txt` (test client), `log_sentry.txt` (the last crashed session); `level: error`, `contains` |
| `game_health` | From outside the game: each game window's process (responding or hung, CPU, memory, threads), processes that ended since the last check with their exit code, the game's recent crash reports, GPU load. Works when the game is hung, paused, in a menu or gone |
| `crash_report` | One of the game's own crash reports (`index` 0 = newest): message, Lua stack, native stack with function names where known, memory, the log up to the crash |
| `dump_game` | Call stacks of a hung or slow game's main thread and busiest threads; `write_dump: true` adds a minidump file |
| `save_game_code` | The running exe's decrypted code to `%LOCALAPPDATA%/Teardown/mcp_code/` (for `tools/port_names.py` and disassemblers; stays on this machine). Save one per build of each exe |
| `perf_sample` | Over `seconds` (default 5): frame times from inside the game (avg, p50/p95/p99, frames over 33 / 50 ms), CPU and memory of the process, entity counts, GPU load |
| `game_command` | `Command(...)`, e.g. `game.quicksave` |
| `start_level` | `StartLevel`. Risky: a path the engine cannot load drops to the main menu |

## 2. Recipes

**Edit, reload, check.** Edit files, then:

```
reload_scene {}
read_log   { "file": "log_0.txt", "level": "error", "lines": 40 }
look_at    { "target": [0, 1, -22], "from": [6, 0.2, 10] }
screenshot {}
```

Lua errors appear in the log with file and line and as red text on screen. In the multiplayer
test the host log also fills with `Couldn't find sound bank ...mcp-bridge/mcp...` lines: those
are the bridge's own replies, ignore them (`contains` a word from the mod's path to filter).

A moving target has moved on by the time the screenshot is taken: pass the entity handle as
`target` and take the screenshot in the same turn, or look at where it is going.

**Read a mod's state.** A mod's Lua variables and its `shared` table are not reachable. Its
registry keys are:

```
registry_get { "key": "level", "depth": 1 }
```

For anything else, add a temporary `SetString("level.debug.x", tostring(x))` (or
`DebugWatch`) to the mod, reload, read.

**Host versus joined client.** Run the same reader on both and compare:

```
batch { "commands": [
  { "action": "run_lua", "side": "client", "args": { "file": "C:/.../read.lua" } },
  { "action": "run_lua", "side": "remote", "args": { "file": "C:/.../read.lua" } } ] }
```

A value present on the host and missing on `remote` is the classic unsynced registry key.
`screenshot` with `window: "TD - Client1"` shows the client's view.

**Test UI code.** `Ui*` only exists in draw:

```
run_lua { "side": "draw", "code": "UiFont('regular.ttf', 24) return UiGetTextSize('hello')" }
```

It runs for one frame, so it can measure, not show: for something to stay on screen it must be
in a script's own `client.draw`.

**Play it like a player.** `send_input` takes `actions`, run in order:

```
send_input { "actions": [
  { "key": "w", "hold_ms": 800 },                       walk forward
  { "move": [400, 0] },                                 turn right about 46 degrees
  { "key": "space" },                                   jump
  { "click": "left" },                                  use the held tool at the crosshair
  { "key": "m" }, { "wait_ms": 300 },                   a mod's own key binding
  { "click": "left", "x": 0.5, "y": 0.287 },            click a UI button
  { "drag": [0.5, 0.385], "to": [0.6, 0.385], "hold_ms": 300 }   drag a slider
] }
```

- `x`, `y` are fractions of the window: take a `screenshot`, read the pixel position of the
  control, divide by the image's width and height. That was accurate enough to hit 70-pixel
  buttons every time.
- Read the result from the mod's state afterwards (registry keys, `player_get`), not from the
  fact that the input was sent.
- It brings the game window to the front and the user loses keyboard focus meanwhile; it sends
  nothing if the window does not get focus. Pass `restore_focus: false` between consecutive
  calls so the window does not flip back and forth, and leave it at the default on the last.
- **The first click after the window comes to the front could be lost** (2026-11-20, the
  multiplayer test's host window, several times, intermittent): a 1-second hold sent as the
  first action of a call never reached `InputDown("usetool")`, while the same hold after a
  throwaway `{ "click": "left", "hold_ms": 40 }, { "wait_ms": 300 }` always did. Likely cause:
  the game resets input when it notices the focus change, wiping a press that came first.
  The server now waits 400 ms after taking focus and nudges the mouse one pixel before the
  first button press; 5 of 5 single first clicks fired after that (the fault was not
  reproducible on demand, so this is not proof). If a first click still goes missing, the
  throwaway click above is the fallback.
- In a vehicle: `w` / `s` drive, `a` / `d` steer, `e` gets in and out.
- `esc` opens the pause menu. In single-player scripts stop there; in the multiplayer test
  they keep running. A second `esc` or a click on Continue closes it; `screenshot` shows which.
- Key combos: join names with `+`, pressed in order and released in reverse.
- **The engine profiler** (CPU per system and per Lua script, per-callback graphs, a log
  viewer and a live registry viewer): `{ "key": "shift+alt+f3" }` opens, `{ "key": "f3" }`
  closes. Its mouse works only with the pause menu open (`esc` first, `esc` again to resume).
  An open Log or Registry window blocks all game input: close it by its x (while paused)
  before sending game input. Use `hold_ms: 200` for clicks when the frame rate is low.
  Details in quirks.md, "The engine profiler".
- `window: "TD - Client1"` drives the joined client instead of the host.
- Text fields: click the field, then one `key` action per character (`backspace` deletes);
  click outside to release it, Enter does not.
- Use registry switches (see authoring.md) when the point is the mod's logic, and
  `send_input` when the point is the input path itself (`InputPressed`, UI hit-testing,
  driving feel).

**It crashed, hung or got slow.**

```
game_health  {}                         alive? responding? exit code? new crash reports?
crash_report {}                         the newest crash: message, Lua stack, native stack, log tail
dump_game    {}                         hung: where the main thread is stuck
perf_sample  { "seconds": 5 }           slow: frame times, CPU, memory, entity counts
```

- A Lua error never crashes the game (it shows as red text). A crash with `lua_callstack`
  entries points at a script; an empty one is native. Native frames named
  `PhysicsUpdate`, `Destr*`, `Split*`, `Vox*` mean physics or destruction: look at what the mod
  spawns, breaks or moves in that moment (too many bodies, huge shapes, joints).
- `crash_report` and `dump_game` name engine functions only when there is a table for exactly
  that exe and build (`names_table` says where it came from); otherwise they give function
  starts, still enough to see whether two crashes of one build are the same. "name ported to
  changed code" means the function changed in that update: the name is likely, the offset in it
  is not comparable to other builds.
- To find which script costs the frame time, open the engine profiler (below) and read the
  per-script graphs; `perf_sample` before and after a change tells whether it helped.
- `dump_game` pauses each thread it reads for a moment. A main thread in the graphics driver
  is rendering; one waiting (`NtWaitFor...`) is waiting for worker threads; one in teardown.exe
  frames is computing (physics, scripts).

**Longer experiments.** Write a `.lua` file that returns a string or table and pass it as
`file`. Results over about 240 000 characters are refused; return less.

**Starting in a mod's game level.** `mp_test_start { "mod": "My Mod", "level": "levels/x.xml" }`
skips the mod's `main.xml` (often a menu). Whatever the menu would have written to the savegame
or registry is then missing; write it with `registry_set` first if the level needs it.

## 3. When a call fails

| Message | Meaning | Do |
|---|---|---|
| `X is not available in client script. The bridge blocked the call...` | Wrong side; nothing ran | Repeat with the side it names |
| `no answer from the game within 20 s` | No level running, paused, in a menu, mod not enabled, or that bridge half was stopped | `screenshot`. If a log line is quoted, a wrong-side call stopped that half: `reload_scene` with the other `side`, or `mp_test_start` with `restart: true`. If the screenshot shows the pause menu, `send_input` a click on Continue; in the main menu, ask the user |
| `the game stopped answering ...` | Same, after some commands had run | Same |
| `the command ended the script callback without returning` | The call ended the callback in a way `pcall` cannot catch; it was not retried | Split the code to find the call; note it in quirks.md |
| `the scene did not report back within N s` | The reload did not finish in time or the level failed to load | `screenshot`, `read_log`; raise `timeout_secs` for heavy mods |
| `a multiplayer test is already running` | | `restart: true`, or use the running one |
| `the host is up ... but the client did not join` | Client kicked or stuck | `read_log` with `file: log_0.txt`, look for "mismatch" or the client state lines |
| `result too large` | | Return less |

## 4. Limits

- The bridge's Lua side runs only inside a level. In the single-player pause menu or the main
  menu only `screenshot` and `send_input` still work (they do not go through the game's scripts).
- One MCP session at a time should send commands; two at once can interfere.
- `remote` exists only in the local multiplayer test (the client's game reads the same file
  on the same PC).
- In the multiplayer test each answer adds about 6 KB of lines to the test window's log.
