# Measured engine behaviour

Everything here was measured through the bridge on Teardown **2.2.0** (Windows, Steam),
2026-11-16/17, in the built-in Villa Gordon level and the Lee sandbox, single-player and the
local multiplayer test (host + one client on one PC). "Not verified" items are listed at the
end. Re-measure after a game update: the probes are in `tools/probe/` of the teardown-mcp repo.

## Contents

1. Script model
2. Wrong-side calls
3. Functions that cannot be used at all
4. Registry, `shared`, and what reaches whom
5. Entities and handles
6. Players
7. Lua sandbox and files
8. Savegame
9. Logs and errors
10. Local multiplayer test
11. Level loading, pause, menus
12. Not verified
13. Extending this knowledge

## 1. Script model

- `#version 2` scripts have a server half and a client half. Each half is its own Lua state:
  the server state has a `server` table and no `client` table, the client state the reverse.
  Both have `shared`. Globals set in one half do not exist in the other, also on the host.
- `Ui*` functions (138 of them) are `nil` everywhere except inside `client.draw()`. Checking
  `type(UiText)` in `client.tick` gives `"nil"`. `UiWidth()`/`UiHeight()` in the multiplayer
  test's half-screen window returned 1920 x 2280.
- Every other documented function exists as a global in both halves. Existence says nothing
  about whether the call is allowed there (next section).
- In `client.draw()` the non-UI API is callable as in `client.tick`.

## 2. Wrong-side calls

Calling a function from the half it is not meant for:

- does **not** raise a Lua error; `pcall` returns true and the call returns nothing;
- writes `<path>:<line>: <Fn> is not available in client script` (or `server script`) to the
  game's log;
- lets the current callback finish, then **stops that half of that script until the level
  reloads**. No more `tick`, `update` or `draw` for it. The other half keeps running, and so
  do other scripts.

Measured by calling each function once per side and reading the log, then confirmed one call
at a time: the first wrong-side call was the last command that half answered.

Server-only (stop the **client** half when called from client code), 39:
`AddHeat AddSnow ApplyPlayerDamage DisablePlayerDamage DriveVehicle Explosion MakeHole Paint
PaintRGBA RegisterTool ReleasePlayerGrab RemoveAabbFires SetEnvironmentProperty SetGravity
SetPlayerCrouchSpeedScale SetPlayerGroundVelocity SetPlayerHealth SetPlayerHurtSpeedScale
SetPlayerParam SetPlayerRegenerationState SetPlayerScreen SetPlayerSpawnHealth
SetPlayerSpawnTool SetPlayerSpawnTransform SetPlayerTool SetPlayerTransform
SetPlayerTransformWithPitch SetPlayerVehicle SetPlayerVelocity SetPlayerWalkingSpeed
SetTimeScale SetToolAmmo SetToolAmmoPickupAmount SetToolEnabled SetVehicleEngineHealth
SetVehicleHealth SetVehicleParam Shoot SpawnFire`

Client-only (stop the **server** half when called from server code), 32:
`AddMapMarker AttachCameraTo CopyAnimatorState CreateHaptic DisableMotionBlur
GetCameraTransform HapticIsPlaying LoadHaptic PlayHaptic PlayHapticDirectional
PlaySoundForUser RequestFirstPerson RequestThirdPerson SaveCameraOverrideTransform
SelectedMapMarker SetCameraDof SetCameraFov SetCameraOffsetTransform SetCameraTransform
SetLowHealthBlurThreshold SetPivotClipBody SetPlayerCameraOffsetTransform SetPlayerRigTags
SetSoundLoopUser SetToolAllowedZoom SetToolHandPoseLocalTransform SetToolHaptic SetToolOffset
SetToolTransform SetToolTransformOverride ShakeCamera StopHaptic`

Notes:

- `GetCameraTransform` is the one read function in the list: server code that wants a camera
  must use `GetPlayerCameraTransform(playerId)`.
- These came from calls with no arguments plus a second pass with valid arguments; the two
  agreed. A function whose argument check runs before the side check could be missing.
- All read functions tested (`Get*`, `Is*`, `Has*`, `Find*`, `Query*`, `List*`; 245 of them)
  worked the same in both halves.
- Effects are fine from both halves: `PlaySound`, `PlayLoop`, `SpawnParticle`, `PointLight`,
  `DrawLine`, `DebugLine`, `DebugCross`, `DebugWatch`, `DrawSprite`, `DrawBodyOutline`,
  `DrawShapeOutline` all ran without complaint on server and client. Whether a server-side
  effect is seen or heard on joined clients was not checked.

## 3. Functions that cannot be used at all

- Raise `"<Fn> function is only for internal usage"` (a normal, catchable error), 41 in all,
  among them: `IsRunningOn*` (Steam, PC, consoles), `GetUserName`, `GetClipboardText`,
  `HasInputController`, `IsControllerButtonDown`, `IsAchievementCompleted`, `IsTestingMp`,
  `GetBodyHit`, `GetBodyHitsCount`, `IsBodyHitted`, `WatchBodyHit`, `GetShapeStrength`,
  `SetShapeStrength`, `SaveShape`, `SetRopeSlack`, `DisableCrouch`, `GetPlayerToolRecoil`,
  `GetLayoutActions`, `GetActionByButton`, `GetButtonsByAction`, `GetKeyByAction`,
  `GetDisplayCount`, `GetDisplayName`, `GetDisplayResolution`, `UiGetModsList`, `UiGetModById`,
  `UiIsMenuSyncEnabled`.
- Documented but not present: `AddAnimatorAnimations`, `EnableAnimatorPosAnim`,
  `SetAnimatorTransform2`.
- Wrong property names raise errors: `GetProperty` "Unavailable property x",
  `GetEnvironmentProperty` "Unknown environment property x", `GetPostProcessingProperty`,
  `GetPlayerParam` "Unavailable player param".

`api-index.md` marks each function.

## 4. Registry, `shared`, and what reaches whom

Measured by writing on one side and reading on the others (H-server = host's server half,
H-client = host's client half, Remote = the joined client's game):

| Written by | How | H-server | H-client | Remote |
|---|---|---|---|---|
| H-server | `SetString("level.x", v)` | yes | yes | **no** |
| H-server | `SetString("level.x", v, true)` | yes | yes | yes |
| H-server | `SetString("savegame.mod.x", v)` | yes | yes | no |
| H-client | `SetString("level.x", v)` | yes | yes | no |
| H-client | `SetString("level.x", v, true)` | yes | yes | yes |
| Remote | `SetString("level.x", v)` or with `true` | **no** | no | yes (itself only) |
| H-server | `shared.x = v` (nested tables too) | yes | yes | yes |
| any client | `shared.x = v` | **no** | only that client | only that client |

So: the host's two halves share one registry (single-player testing hides every sync bug,
because there the "client" always sees the server's keys). A joined client gets only keys
written with the sync flag, and can send nothing back through the registry or `shared`; it has
to use `ServerCall`. `SetString("game.x", v)` on an unknown `game.` key works on every side
(local to that game).

`shared` is **per script**: each script file has its own table. The bridge's `shared` is not
a mod's `shared`, so a mod's shared state cannot be read from outside; mirror it to the
registry. Within a script, nested writes (`shared.t.n = shared.t.n + 1`, `shared.t.list[1] =
"x"`) take effect at once and reach host client and joined client.

`SetEnvironmentProperty` called by the server changes the weather on joined clients too
(checked by screenshot of the client window).

`ServerCall("server.fn", ...)` from a client and `ClientCall(playerId, "client.fn", ...)` from
the server both ran. The function must be defined in the same script file.

## 5. Entities and handles

- Entities created by the server have the same handle number on every machine (a body spawned
  by the server was 6421 on host server, host client and the remote client). Tags set by the
  server are visible on clients.
- Client code cannot change server-created entities. `SetTag`, `SetDescription`,
  `SetBodyTransform`, `SetBodyDynamic`, `SetBodyVelocity`, `SetBodyAngularVelocity`,
  `SetBodyActive`, `ApplyBodyImpulse`, `SetShapeLocalTransform`, `SetShapeEmissiveScale`,
  `SetShapeDensity`, `SetShapeCollisionFilter`, `DrawShapeBox`, `CreateShape`,
  `SetLightEnabled`, `SetLightIntensity` all raised `invalid attempt to access server authored
  entity from client script`. This is a normal error and does not stop the script.
- Client code can `Spawn`. The handles are **negative** (-593), the entity exists only on that
  machine, and that client can `Delete` it. A body spawned by the host's client half is
  findable from the host's server half (same process); one spawned on the remote client is not.
- `Spawn(xml, transform)` accepts an XML string as well as a prefab path and returns every
  created handle (body, shapes, scripts).
- `MakeHole` on empty air returns 0.

## 6. Players

- Ids: host 1, first joined client 2. `GetAllPlayers()` returns `{1, 2}` on every side.
  `GetLocalPlayer()` is 1 on the host (both halves) and 2 on the client.
- A player argument of 0 or nil means the host in server code and the local player in client
  code.
- `IsPlayerHost(id)`, `GetPlayerName(id)` work on every side for any player ("Host",
  "Client1" in the multiplayer test).
- Server code can act on any player (`SetPlayerHealth(1, 2)`, `SetPlayerTransform(t, 2)`).
  Client code cannot set player state at all: every `SetPlayer*` and tool function is
  server-only (section 2).
- `InputDown("usetool", playerId)` is callable from server and client for any player.
- A standing player's `GetPlayerTransform(p).pos` is about 1 cm below the floor (y = -0.01 on
  a floor at 0): tests against volumes that start exactly at floor level miss the player.
- The held tool's body (`GetToolBody`) has a negative handle: it is a client-side entity.
- Mouse look: 400 pixels of relative mouse movement turned the camera 46 degrees at the
  default sensitivity. `InputPressed("m")` and other plain key names work as inputs in client
  code; `InputPressed("usetool", p)` works in server code for a real click.
- `UiTextInput(text, w, h, focus)` (2026-11-17, real input): a click on the field focuses it
  (second return value `active` true), typed letters, digits and Backspace edit the text,
  Enter does **not** release focus, a click elsewhere does. Keys typed into the field still
  reach `InputPressed`: typing "mom" counted two `InputPressed("m")`, so a key binding must
  be ignored while `active` is true or typing will trigger it. The function draws no frame:
  draw one yourself at the same corner.
- Profiler: see "The engine profiler" below.
- Input sent to the joined client's window (`window: "TD - Client1"`) moved and turned only
  that player and opened only that client's UI; a button there reached the server through
  `ServerCall` like a host click.
- `GetTime()` is per game instance: the host and the client report different values.
- `game.levelpath` is an absolute path on the host and a relative one
  (`mods/villagordon/main.xml`) on the joined client.

## 7. Lua sandbox and files

- Lua 5.1. Present: `loadstring`, `load`, `loadfile`, `dofile`, `setfenv`, `getfenv`,
  `coroutine`, `string`, `table`, `math`, `newproxy`. Absent: `io`, `os`, `debug`, `require`,
  `package`. A script cannot write a file.
- `loadfile` / `dofile` read any file on disk, live, by **absolute path**. `MOD/...` and
  relative paths fail ("No such file or directory"); `MOD/` is only understood by engine
  functions and `#include`.
- `HasFile("MOD/x")` checks the disk live.
- `#include` is resolved by the engine before Lua runs, relative to the script; a missing
  include keeps the whole script from loading.
- `LoadSound` on a missing file logs `Couldn't find sound bank <full path>` (paths of 600+
  characters survive). `LoadSound` called by the **server** half is replicated: every joined
  client loads the same sound, also players who join later.

## 8. Savegame

- `savegame.mod.*` keys of a local mod are stored under `<mod><local-<folder>>` in
  `%LOCALAPPDATA%\Teardown\savegame.xml`; mod id = `local-` + folder name in lower case with
  non-alphanumerics turned into `-`.
- The normal game writes the file within about 0.5 s of a change (the whole ~400 KB file).
- The game reads the file **once at startup**. An outside edit is never picked up, not on a
  level restart either, and is overwritten by the next save.
- The multiplayer test windows read the savegame at start and **never write it**.

## 9. Logs and errors

- Normal game: `%LOCALAPPDATA%\Teardown\log.txt`. Multiplayer test: `log_0.txt` (host),
  `log_1.txt` (client). Written in 4 KB blocks, so the newest lines lag.
- Logged with script path and line: wrong-side calls, missing prefabs / sounds / vox files,
  disabled scripts. Lua runtime errors also show as red text on screen: `screenshot`.
- A `#version`-less script in multiplayer is logged as `Single player script disabled`.
- Windows reports a stale size for a log another process is writing; open the file to get the
  real one.

## 10. Local multiplayer test

- Two processes of `teardown_modtest.exe`, windows "TD - Host" and "TD - Client1":
  `-half_screen 1 -mod_test_host "<mod id>" "<level xml>" "" "" "<global mod id>" -logfile
  <appdata>/log_0.txt` and `-half_screen 2 -mod_test_client -logfile <appdata>/log_1.txt`.
  They start without the main game running. The MCP server sets `SteamAppId=1167630` in their
  environment; whether that is required was not checked.
- The host hashes each mod's folder and compares with the client ("Calculated hash for mod").
  A file that changes in a mod folder during start-up gets the client kicked: host log
  `Detected mod mismatch, will kick player`.
- Client join states in the host log: CONNECTING, CONNECTED, MODS_SYNCING, MODS_SYNCED,
  LOADING, LOADED, DETERMINISTIC_SYNCING, INITED, REPLICATION_CAUGHT_UP, PLAYING.
- Villa Gordon: host in play after ~15 s, client in after ~25 s. A heavy mod: minutes.
- Started on a level other than the mod's `main.xml`, `game.levelid` is empty.

## 11. Level loading, pause, menus

- `Restart()` works from server code and from the host's client code. Scripts are reloaded
  from disk. `level.*` registry keys and spawned entities are gone afterwards; `savegame.*`
  keys stay.
- Single-player: no script callback runs in the pause menu or the main menu (not `draw`
  either), and `GetTime()` does not advance while paused, so nothing can be done from a script
  to leave them. A real click on Continue (or a second `esc`) through `send_input` resumed the
  game and the bridge answered again (2026-11-17, 2.2.0).
- Multiplayer test: the pause menu (Esc on the host) does **not** stop anything. The bridge
  kept answering on server and client sides and `GetTime()` kept running; the menu has a
  player list with Kick buttons instead. Continue or a second `esc` closed it.
- Playing a mod from the mod manager asks "Number of Players" (local split-screen: one, two
  or four) before loading. The bridge must be enabled in the mod manager for single-player
  (the multiplayer test loads it by itself).
- `StartLevel(mission, path)`: `data/level/hub.xml` was loaded as
  `data/level/data/level/hub.xml` and failed; the next load dropped to the main menu.

### The engine profiler

Explored 2026-11-18, Teardown 2.2.0, single-player (full-size) and the multiplayer test host,
driven with `send_input`.

- Shift+Alt+F3 opens it, F3 closes it. It is a panel down the left side of the window.
- Its own help text: "profiling mode" runs one update per frame and caps the frame rate at
  60 FPS, so the frame rate may differ from normal play but the timings are correct.
- The panel shows, live: CPU total (ms); counts of bodies, simulated bodies, shapes, visible
  shapes, visible lights by type, particles; CPU ms per engine system (Destruction, Particle,
  Path planning, Physics, Rendering, Screen, Script (server), Script, Water); and the top ten
  Lua scripts by CPU ms. "..." under that list opens every running script by full path
  (mods' scripts and the game's own `data/script/*.lua`, e.g. playerbody, sandbox,
  creativemode, achievements, characters, spawn).
- Selecting a script shows its path, mod name, instance count, memory use of the client and
  server halves, and last / average / peak ms graphs for each callback: Tick, Update and Post
  physics update for the server, then Tick, Update, Post physics update, Render and Draw for
  the client. This is the way to see what a mod's script costs and in which callback.
- The MCP Bridge itself: about 0.02 ms average per tick, with regular peaks of about 0.35 to
  0.4 ms in the ticks where it polls the command file; around 250 to 320 kB per half.
- Log and Registry buttons open windows over the game:
  - Log: the game log with auto-scroll, a tail line count, a level filter (debug and up), a
    text filter and per-tag toggles (Init, Render, Window, Loading, Script, VoxScript,
    LocalMod, StateSwitch, Saving, Network, MatchMaking, SplitServer, SplitClient, ModParity,
    Presence).
  - Registry: the whole registry as a tree (characters, game, hud, level, loading, mods,
    options, promo, promo_mp, savegame, spawn); each key shows its value and full name. It
    updates live (a key written through the bridge appeared), but values are **read-only**:
    a click, Ctrl+A, typing and Enter did not change one.
  - With the Registry window open the frame rate fell from 60 to about 12 FPS.
- The mouse reaches the panel and its windows **only while the pause menu is open**; while
  playing, a click on a tab did nothing. A window is closed by its x (while paused).
- **Input:** with only the panel open, game input works (walking, mouse look, a mod's key
  binding). With a Log or Registry window open, game input is blocked: `w` for 1 s and a
  mouse turn did nothing, and worked again once the windows were closed. Not verified:
  whether those windows stay open when the profiler is closed and reopened.
- At 12 FPS a 60 ms click is shorter than a frame and was missed; a 200 ms press worked.
- The bridge kept answering with the profiler and its windows open.

## 12. Not verified

- Why a `Shoot` in the first second after a level load does nothing (seen twice; an
  `Explosion` 1.5 s after a load did work).
- Gamepad input: `send_input` sends keyboard and mouse only; a virtual pad would need a
  driver such as ViGEm.

- Whether sounds, particles and lights started by server code reach joined clients (weather
  set by the server does).
- A working `StartLevel` path form for built-in and mod levels.
- More than one joined client; a real online session.
- `Restart`, `SetPaused`, `Command`, `Menu`, `StartLevel`, `Delete`, `ClearKey`, session and
  achievement functions, `SetPlayerHidden`, `DisablePlayer*`, `RespawnPlayer*` were left out
  of the side probe (marked `untested` in the index).
- Whether a wrong-side call inside a script's `init` behaves like one in `tick`.

## 13. Extending this knowledge

When the references do not answer a question about the engine, measure it:

1. Start a light level: `mp_test_start` with `mod: builtin-villagordon`.
2. Write a Lua file that performs the call inside `pcall` and returns what it observed; run it
   with `run_lua` (`file`, and the `side` in question). For anything about propagation, run a
   reader on the other sides afterwards (`client`, `remote`).
3. Read `log_0.txt` / `log_1.txt` for lines the call produced; a wrong-side call shows only
   there.
4. Write the result into this file with the date and game version, and say how it was
   measured. If it changes a side classification, add the name to the lists in the bridge
   mod (`CLIENT_ONLY` / `SERVER_ONLY` in `mod/main.lua`) and regenerate `api-index.md`.

A probe that calls an untested function can stop a script half or crash the game; that is
what the sandbox level is for. Restart the test with `mp_test_start` and `restart: true`.
