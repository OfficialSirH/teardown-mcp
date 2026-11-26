# What the Unofficial TD Docs add (not verified)

Source: [Unofficial TD Docs](https://githeadcrab.github.io/) (community site, "BETA"; data read
2026-11-21 from its `gh-pages` branch, commit b871b7fd of 2026-07-04). Most of its Script
section repeats the official API (`script_defs.lua`, already in `api-index.md`). This file
holds only what the skill did not have, summarised here, with what the game's own files confirm.
**Nothing here was measured in game unless it says so.** Where it contradicts `quirks.md`,
the measurement in `quirks.md` wins. When you rely on an entry, measure it and move it into
`quirks.md`.

Refresh: `python -I tools/fetch_unofficial_docs.py [out_dir]` downloads the site's data
(default `%LOCALAPPDATA%\Teardown\unofficial_docs\`) and writes `summary.txt`, which lists
per section the names that are not in `api-index.md`. Compare with the commit noted above. The
site's text is not stored in this repository (it has no licence).

The site links its Tags page to "dennispedia" (`x4fx77x4f.github.io/dennispedia/teardown/tags`);
that site returned 404 on 2026-11-21.

## Contents

1. Callbacks
2. Function notes beyond `script_defs.lua`
3. Entity tags the engine and the game scripts read
4. Registry keys
5. Voxscript
6. mplib (the game's multiplayer game-mode library)
7. What the site does not have

## 1. Callbacks

`script_defs.lua` documents no callbacks. The site lists eight, each as `server.X` / `client.X`
in version 2 and a plain global `X` in version 1:

| Callback | Halves | What the site says | What we know |
|---|---|---|---|
| `init()` | both | once when the script starts | measured |
| `tick(dt)` | both | once per frame, dt between 0 and 1/30 s | measured (dt capped at 1/30 s, quirks 1 and 11b) |
| `update(dt)` | both | fixed 60 Hz, dt always 1/60 s, **at most twice per frame**, sometimes not at all in a frame | we measured 2 **or 3** in one slow frame (quirks 1); trust that |
| `postUpdate(dt)` | both | after the game's own scripts, so a mod can set animator animations there; **formerly called `postPhysicsUpdate`** | measured: once per physics step, after it (quirks 1). The profiler's callback name "Post physics update" fits the old name |
| `draw(dt)` | client | 2D overlay, after the scene and before the game's HUD; the only place `Ui*` works | measured (`Ui*` nil elsewhere) |
| `render(dt)` | client | "perfect sync with player camera"; **also called while the game is paused** | measured: once per frame in play, **and it does keep running in the single-player pause menu** while `tick` and `draw` stop; `server.render` is never called (quirks 1 and 11). Many built-in level scripts define `client.render(dt)` (`data/level/*/script/`) |
| `handleCommand(cmd)` | both | called on certain events, see below | measured for `explosion` and `shot`: one string (quirks 1) |
| `destroy()` | both | game-mode scripts: called when the game mode stops | measured: called when a **global** game mode ends (both halves, joined clients too, no level reload); **not** called when a script entity is deleted (quirks 1), nor when a content game mode is switched off (authoring.md 13) |

`handleCommand` events, per the site: `shot` (a gunshot hits a surface; strength, hit point,
direction), `explosion` (strength, point), `resolutionchanged` (`game.applydisplay` ran),
`quicksave`, `quickload` (a quicksave is loaded, instead of `init`). Measured (quirks 1): the
event arrives as one space-separated string such as `"explosion 0.5 30.0 0.5 30.0"`; the
joined client got `explosion` but not `shot`; `quicksave` and `quickload` arrive as just
those words (single-player). `resolutionchanged` not tested.

Note: the site's table for `draw` names `client.render`; that is a slip on the site, `draw` is
`client.draw`.

## 2. Function notes beyond `script_defs.lua`

Notes the official text does not have. None measured.

- `GetVehicleParams(v)`: the keys are `spring`, `damping`, `topspeed`, `acceleration`,
  `strength`, `antispin`, `antiroll`, `difflock`, `steerassist`, `friction` (**measured**,
  quirks 5; the same names as the `<vehicle>` XML attributes; `SetVehicleParam` presumably
  takes them).
- `GetBodyClosestPoint(body, origin)`: the site says it respects `QueryRequire` but not
  `QueryRejectShape`. **Measured otherwise** (quirks 5): `QueryRequire("dynamic")`,
  `QueryRejectShape` and `QueryRejectBody` all left it hitting a static body.
- `ParticleOrientation(flags)`: one of `normalup`, `up`, `fixed` (exclusive, checked in that
  order), optionally plus `flat`. `script_defs.lua` marks it deprecated.
- `UiImageAlpha(path, alphaPath)` and `UiImageOverlayAlpha(path, alpha1, alpha2)`: draw an
  image with a mask image; dark parts of the mask turn the image transparent. The site has
  only these two forms; the 10-argument form with source rectangles and the return values
  (drawn width and height) are measured in quirks.md section 6.
- `GetAddedPlayers()` / `GetRemovedPlayers()`: the site guesses "since the last frame".
  **Measured** (quirks 6): each join or leave is reported once per script (a script's first
  tick lists everyone present); player ids are not reused. `PlayersAdded()` /
  `PlayersRemoved()` from `script/include/player.lua` wrap them.
- `GetPlayerInteractShape` / `GetPlayerInteractBody` only return entities tagged `interact`
  (also in `script_defs.lua`); see the tag below.
- `SetVehicleHealth` works only on vehicles tagged `customhealth` (also in `script_defs.lua`).
- `SpawnLayer`: spawns one layer of a vox file and applies colour to its `$TINT` layer.
- Names the site has that 2.2.0's `script_defs.lua` lacks: `InputLastPressedGamepadButton()`,
  `GetPlayerRigTransform()`, `SetPlayerZoom(zoom, time)`. **All three are nil in 2.2.0**
  (measured, quirks 3).
- Renames: `IsStaticShapeDetached` was probably `IsStaticShapeDisconnected`;
  `SetToolHandPoseLocalTransform` was once `SetToolHandPos`.

## 3. Entity tags the engine and the game scripts read

Tags set in XML (`tags="..."`) or with `SetTag`. "Script:" means a game Lua script reads the
tag, not the engine, so the tag only works when that script runs in the level. Short
descriptions in our words; "?" marks what the site itself is unsure of. **Measured since
(quirks.md section 5, "Tags the engine reads"):** `unbreakable`, `inherittags`, `explosive`,
`bomb`, `bounded`, `nonav`, `interact`, `customhealth`, `passive`, `nodrive`,
`brokenthreshold`, `impactexplode`, `booster`, `invisible`, `night` behave as described below;
`nomerge`, `noroll` and `smoke` showed no effect in our tests.

**Vehicles** (on the `<vehicle>` unless noted):

| Tag | Effect |
|---|---|
| `boat` | acts as a boat: uses a `propeller` location, no wheels |
| `crane`, `cranebottom`, `cranetop`, `dumptruck`, `frontloader`, `skylift` | hinge joints of the vehicle are driven by `vehicle_raise` / `vehicle_lower`; each shows its own control hints. `crane` also drives the hook |
| `forklift` | the same for prismatic joints |
| `tank` | tank controls. Script: `data/script/tank.lua`, `data/ui/hud.lua` |
| `nodrive` | no "Drive" prompt; players cannot drive it |
| `passive` | ignores input: idles, max 3 km/h forward, 2 km/h back |
| `reversebeep` | beeps when reversing |
| `noroll` | cannot be flipped |
| `customhealth` | breaking it does not lower its health; use `SetVehicleHealth` |
| `brokenthreshold=f` | undrivable once fewer than f of its original voxels remain |
| `player_standing` | the driver stands |
| `player_nogears`, `player_nosteeringwheel` | no gear / steering-wheel hand motion ? |
| joint `extend`, `turn` | joint driven by up/down or left/right in crane vehicles ? |
| joint `level` | skylift joints keep equal rotation ? |
| body / shape `hook` | the vehicle's hook, worked with `vehicle_action` |
| body `autobreak` | breaks at smaller impacts; the engine sets it on vehicle bodies |
| shape `turbo` | blue flame booster on a vehicle shape, by throttle; set by the engine |

Vehicle **locations**: `player` (entry point and first-person camera), `exit` (where the
player gets out; default: left of `player`), `camera` (third-person camera), `exhaust=amount`
(exhaust position and direction), `vital` (where engine smoke comes from below 50 % health; the
site says it does **not** kill the vehicle when broken, checked on 1.4.0), `propeller` (boats).
`name=<id>` names a location: in a tool prefab the name used by
`GetToolLocationLocalTransform` / `GetToolLocationWorldTransform` (we measured that, authoring.md
section 10); in a vehicle the player IK targets `player_steeringwheel`, `player_stand`,
`player_seat`, `ik_head`, `ik_hand_l`, `ik_hand_r`, `ik_foot_l`, `ik_foot_r`. With
`relative_parent` the IK location follows its parent body.

**Effects** (script: `data/script/fx.lua`):

- shape `fxbreak=<l|g><1-9><colour>`: liquid or gas particles for 1 to 9 s when broken
  (liquid: yellow, black, blue, green, brown; gas: yellow, blue, green). Example `fxbreak=g5blue`.
- light `fxflicker=<1-9>`: flicker strength.
- location `fxsmoke=<type><1-9><colour>`: smoke from a nearby shape; type `p` makes particles
  grow; colours `brown`, `subtle`.

**Game modes** (locations read by mplib, section 6): `playerspawn`, `teamspawn=<team>`,
`ammospawn` with `rarity=low|medium|high`, `pointofinterest=<team>`.

**Other:**

| Tag | On | Effect |
|---|---|---|
| `unbreakable` | body, shape, rope | cannot break; still takes paint and burn marks |
| `explosive=size` | body, shape | explodes when broken (size 0.5 to 4?, default 1); a thrown one explodes on contact |
| `impactexplode` | body | explodes on impact if a shape of it is `explosive` |
| `bomb=seconds`, `bombstrength=size` | shape | counts down, then explodes; set by the engine for bombs |
| `booster=t` | shape | rocket flame and smoke along +Z while 0 < t < 7; t rises 1/s, tag removed at 10 |
| `smoke` | shape | emits smoke (pipe bombs) |
| `breakall` | shape | breaks shapes it touches (robots; thrown explosives ?) |
| `inherittags` | body, shape | debris keeps all tags, recursively. Without it debris has no tags (measured, quirks 5) |
| `interact="text"` | body, shape | shows an interaction prompt; needed for `GetPlayerInteract*` |
| `invisible` | shape | not rendered. Overlapping shapes loaded together still overwrite each other's voxels unless the body is `nomerge` |
| `nomerge`, `smartmerge` | body | the engine does not merge / merges selectively the body's child shapes at load |
| `nocull` | shape | rendered at any distance |
| `noshadow` | shape | casts no volume shadow (still receives shadows) |
| `nosnow` | shape | no snow at level load |
| `nonav` | shape | ignored by path planning |
| `bounded` | body | kept inside the level boundary |
| `night` | light | off unless the `nightlight` environment property is on |
| `awake` | body, shape | not asleep at level load. Script: `data/script/awake.lua` (a body that loads at rest is asleep, authoring.md section 9) |
| `map` | body | marked on the map. Script: `data/ui/map.lua` |
| `nocursor`, `noidlecursor`, `nocam` | screen | cursor hidden when idle; `nocam` undescribed |
| `plank`, `wire` | joint | removed from robots by the robot code ? |
| `attachment`, `noproxyshape`, `bone[=name]`, `hand_l`, `hand_r` | animator parts | animator rig parts; meaning mostly unknown on the site |
| `ik_hand` | location | editor display aid for hand orientation in tool and vehicle prefabs |

## 4. Registry keys

The site's registry tree has the same top-level nodes we saw in the profiler's Registry window
(characters, game, hud, level, loading, mods, options, promo, savegame, spawn; quirks 11).
Keys a mod is likely to use, with the site's meaning (types as the site gives them):

- **Set every tick to block something:** `game.disablepause`, `game.disablemap`,
  `game.disableinteract` (bool), `game.input.locktool` (keeps the tool selection locked).
  **Measured** (quirks 4, "Registry switches the game reads"): pause and map work as
  described; `game.disableinteract` did nothing in the multiplayer test.
- **HUD:** `hud.hide`, `hud.disable`, `hud.aimdot` (bool). Measured: both hide the game's
  HUD when set every frame; `hud.hide` also hides an open pause menu.
- **Level flags:** `level.sandbox`, `level.campaign`, `level.spawn` (spawn menu available),
  `level.unlimitedammo`, `level.sandbox.disabled` (switches `sandbox.lua` off, also at
  runtime). Measured (quirks 4): `level.unlimitedammo` does not stop ammo use or refills by
  itself, and any game mode switches sandbox off.
- **State:** `game.state` ("PLAY" in a level), `game.paused`, `game.menu.active`,
  `game.map.enabled`, `game.thirdperson`, `game.version`, `game.levelid`, `game.levelpath`
  (measured: absolute on the host, relative on a joined client, quirks 6), `game.mod`
  (with `.title`, `.description`), `game.savegamepath`, `game.canquickload`,
  `game.brokenvoxels`, `game.fire.maxcount`, `game.fire.spread`, `game.hascontroller`,
  `game.input.curdevice`, `game.input.<action>` (key bound to an action: `jump`, `crouch`,
  `interact`, `flashlight`, `grab`, `handbrake`, ...).
- **Local player:** `game.player.health` (0 to 1), `.tool` (with `.group`, `.scope`,
  `.zoom`, `.targetzoom`), `.grabbing`, `.cangrab`, `.caninteract`, `.canusetool`,
  `.interactive`, `.picking`, `.idling`, `.hit`, `.throwing`, `.steroid`, `.character`,
  `.usevehicle`, `.usescreen`; `game.vehicle.health`, `game.vehicle.interactive`.
  These are per game, so on a joined client they presumably describe that client's player
  (not checked).
- **Tools:** `game.tool.<id>.name` (measured, authoring.md 10), `.enabled`, `.group` (1 to 6),
  `.index` (order in the group), `.ammo`, `.ammo.max` (9999 means unlimited; we saw 9999 with
  `sandbox.lua`, authoring.md 10). mplib treats a tool with `custom=true` as a mod tool.
- **Mods:** `mods.available.<mod-id>.` `name`, `listname`, `author`, `path`, `active`, `local`,
  `playable` (has `main.xml`), `options` (has `options.lua`), `character`, `tags`,
  `previewimage`, `steamtime`, `subscribetime`. Mod ids: `builtin-<name>`,
  `steam-<workshop id>`, `local-<folder>`.
- **Spawn menu:** `spawn.<mod-id>.<n>` = "CATEGORY/ITEM" path from the mod's `spawn.txt`,
  `.path` = the prefab, prefixed by source: `builtin-<pack>:`, `dlc-<pack>:`, `local-<mod>:`,
  `steam-<id>:` (and `BUILT-IN/` for the game's built-in folder). This is the same
  `builtin-<id>:path.xml` form `Spawn` accepts (measured, quirks 5).
- **Characters:** `characters.<mod-id>.<character-id>.` `name`, `description`, `height` (m),
  `anim_speed`, `preview`, `tool_offset`.
- **Options (read-only use):** `options.gfx.fov` (60 to 120), `.quality` (1 to 3),
  `.renderscale`, `.motionblur`, `.dof`, `.vsync`, `.gfxapi` (0 OpenGL, 1 D3D12);
  `options.audio.soundvolume` / `musicvolume` / `ambiencevolume` (0 to 100).
- **Savegame:** `savegame.stats.brokenvoxels`, `savegame.stats.totalplaytime`,
  `savegame.tool.<id>.enabled` (campaign unlocks).

## 5. Voxscript

Voxscripts generate voxel shapes when a level loads. `api-index.md` does not cover them: their
own definitions are `data/voxscript_defs.lua` (and `.luau`) in the game folder, read that for
signatures. From the game's files (checked 2026-11-21):

- Used from XML as `<voxscript pos="..." rot="..." file="MOD/voxscript/x.lua" .../>`; the
  game's own are in `data/built-in/voxscript/` (`ground.lua`, `tree.lua`, `pine.lua`, `roof.lua`,
  `floor.lua`, `foundation.lua`, ...), referenced as `BUILT-IN/voxscript/<file>`.
- A voxscript has no `#version` line and defines a plain `function init()` (the site's
  examples use `client.init`; the built-in ones do not). Parameters are read at file level
  with `GetFloat`, `GetInt`, `GetBool`, `GetString(name, default, hint)`, `GetColor`; per the
  site, parameters outside `init` show up in the editor. `ground.lua` reads `file`, `grass`,
  `scale`, `tilesize`, `hollow`.

**Measured since** (quirks.md section 5, "Voxscript"): XML parameters, `Vox` / `Box` in
voxels with an exclusive end, one static world-body shape per `Vox` carrying the element's
tags, materials from `CreateMaterial`, the same result on a joined client, and runtime
`Spawn` of a `<voxscript>`.

Functions (`voxscript_defs.lua`): random numbers (`Randomize`, `RandomFloat`, `RandomInt`),
materials and brushes (`CreateMaterial(type, r, g, b, a, reflect, shiny, metal, emissive)`,
`CreateBrush("file.vox:object", local)`, `FlipBrush`, `RotateBrush`, `TranslateBrush`,
`GetBrushSize`, `Material(handle)` to pick the current one), images (`LoadImage(png,
grassMap)`, `GetImageSize`, `GetImagePixel`), and drawing: `Vox(x, y, z, rx, ry, rz)` starts a
new shape (needed before drawing), then `Box`, `Sphere`, `Line`, `Set`, `Get`, `Heightmap`
(the site spells it `HeightMap`). Site notes, not verified: a local-coordinate brush is
offset from the `Box` / `Sphere` being drawn, a global one from the `Vox` origin; keep empty
space in each `Vox` shape small and split large hollow things into several shapes.

## 6. mplib (the game's multiplayer game-mode library)

Lua helpers the game's own multiplayer modes are built on. The game ships copies in
`data/script/mp/lib/`, `mods/mpclassics/mplib/` and `mods/splitfieldestate/mplib/` (countdown,
eventlog, hud, inputactions, mp, navigation, spawn, spectate, stats, teams, tools, ui, util).
The built-in modes include them by relative path (`#include "mplib/tools.lua"`, `"../mplib/..."`
or `"script/mp/lib/tools.lua"`). An example mode with a `gamemodes.txt` is
`mods/contentgamemodeexample`. The site documents each function from those files' comments;
read the files themselves for exact behaviour.

The official tutorial (part 9) adds: Tuxedo Labs' own modes all use it, it is open source
(a GitHub repository linked under that video), each module can be used without the others,
and the way to use it is to put a copy in your own mod folder. `#include
"script/mp/lib/..."` reaches the game's copy instead (path rules: quirks.md section 7).

| Module | Server | Client |
|---|---|---|
| countdown | `countdownInit(s)`, `countdownTick(dt)` (players locked while it runs) | `countdownDraw()`; `countdownDone()`, `countdownGetTime()` |
| eventlog | `eventlogPostMessage(items, time)`, `eventlogTick(dt)` (posts player deaths) | `eventlogDraw(dt, playerColors)` |
| hud | `hudAddUnstuckButton()` | `hudTick`, timers, banners, scoreboards and results, team scores, rounds, world markers, damage indicators, respawn timer, host setup screen (`hudDrawGameSetup`), player list, help text, fades |
| spawn | `spawnInit`, `spawnSetSpawnTransforms(list, group)`, `spawnSetRespawnTime`, `spawnSetDefaultLoadout({{tool, ammo}, ...})`, `spawnSpawnPlayer`, `spawnRespawnPlayer`, `spawnRespawnAllPlayers`, `spawnTick(dt, playerGroups)` | `spawnGetPlayerRespawnTimeLeft(p)` |
| spectate | | `spectateTick(playerList)` (the site lists it as a second `spectateDraw`), `spectateDraw()`, `spectateRender(dt)` (spectator camera, from `client.render`) |
| stats | `statsInit`, `statsTick` (counts from `playerdied` events) | `statsGetKills(p)`, `statsGetDeaths(p)` |
| teams | `teamsInit(n)`, `teamsSetNames`, `teamsSetColors`, `teamsSetTeams`, `teamsStart(skipCountdown)`, `teamsTick(dt)` | `teamsDraw(dt)` (team pick screen), `teamsGetLocalTeam`; `teamsGetTeamId(p)`, `teamsGetColor`, `teamsGetName` |
| tools | `toolsInit`, `toolsAddLootTier(transforms, lootTable)`, `toolsSetRespawnTime`, `toolsSetDropToolsOnDeath`, `toolsPreventToolDrop`, `toolsCleanup`, `toolsTick(dt)`; `toolsAddModToolsToLootTable` | |
| ui | | styled buttons, panels, text fitting with ellipsis, player avatar rows |
| util | `utilLoadLevelPlayerSpawns(team)`, `utilLoadLevelToolSpawns(rarity)`, `utilLoadLevelPoi(team)` (read the game-mode location tags, section 3), `utilGenerateSpawnPoints(count, existing)` and density variants | |

The engine events and their arguments are measured in quirks.md section 4, "Engine events".
Engine events the library reads (from the game files): `playerdied`, read as
`for i = 1, GetEventCount("playerdied") do local victim, attacker = GetEvent("playerdied", i)
... end` (`stats.lua`), and `playerhurt` (`hud.lua`).

## 7. What the site does not have

- No XML reference: nothing on the attributes of `<joint>`, `<rope>`, `<body>`, `<vox>`,
  `<voxbox>` or `<vehicle>` beyond the tags above. The measured XML facts are in `quirks.md`
  section 5 and `authoring.md`.
- No side (server / client) per function beyond the official text; `api-index.md` has the
  measured sides.
- Its "Articles", "Custom", "Automnatic" and "Tester" sections are placeholders.
