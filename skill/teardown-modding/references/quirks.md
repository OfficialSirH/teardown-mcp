# Measured engine behaviour

Everything here was measured through the bridge on Teardown **2.2.0** (Windows, Steam),
2026-11-16 to 20, in built-in levels (Villa Gordon, the Lee sandbox) and test levels of our own,
single-player and the local multiplayer test (host + one client on one PC), plus findings
from porting a large mod (2026-11-21, builds 0x6ae468bd and 0x6afda5ed). The entries describe
the engine; the numbers in them come from those tests. Entries marked *observed* were seen
in logs, crash reports or play but not isolated; the rest were measured. "Not verified"
items are listed at the end. Re-measure after a game update: the probes are in
`tools/probe/` of the teardown-mcp repo.

## Contents

1. Script model
2. Wrong-side calls
3. Functions that cannot be used at all
4. Registry, `shared`, and what reaches whom
5. Entities and handles: spawn and prefabs, deleting things, shape editing, materials and
   .vox files, destruction and debris, bodies moved by script, client-side entities and
   animation, light and environment
6. Players
7. Lua sandbox and files
8. Savegame
9. Logs and errors
10. Local multiplayer test
11. Level loading, pause, menus
11b. Crashes, hangs, performance
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
- **`server.update` runs once per 1/60 s physics step**, so 2 or 3 times in one slow frame;
  `tick` runs once per frame. The engine profiler runs one physics update per frame, so its
  per-update numbers differ from normal play.
- **`server.postUpdate(dt)` and `client.postUpdate(dt)` exist** (undocumented in
  `script_defs.lua`; the game's own `script/playerbody.lua` uses `client.postUpdate`). Counted
  per second (2026-11-21, 2.2.0, multiplayer test host): tick 118, update 60, postUpdate 60
  on both halves, so postUpdate runs once per physics step like update. It runs **after** the
  step: for a walking body, the transform read in `client.postUpdate` differed from the one
  `client.tick` read earlier in the same frame in about 60 calls a second, so code that places
  things on a moving body sees its current position there. A callback assigned later from
  another chunk (`server.postUpdate = f` from the bridge's `run_lua`, after init) was never
  called: define it when the script loads.
- **Call `Constrain*` from `server.update`, never from `server.tick`:** from `tick` it crashed
  the engine on a worker thread within minutes (access violations in teardown.exe).
- `GetTimeStep()` is capped at 1/30 s on both halves (it read 33 ms while the game ran at
  4 FPS); `GetFps()` follows the real frame time (0.2 to 1 during multi-second freezes). A
  script cannot time a freeze with `GetTimeStep`.
- **Callbacks are plain table assignments** (`function server.tick() end` assigns
  `server.tick`), so a metatable `__newindex` on `server` / `client`, installed by an include
  before the callbacks are defined, can wrap every callback of a script (per-callback timing
  or logging; works in single-player and multiplayer). Two such includes must chain: the
  second calls the first's `__newindex`.
- **No API calls while a file loads** (top level, includes too): "Calling the <X> API function
  before init is not supported". Pure math (`Vec`, `Quat`) is fine.
- `#include` is resolved before Lua parses: an include inside a `--[[ ]]` comment still loads,
  and a missing file keeps the whole script from loading (section 7). An included file has no
  `#version` line of its own.
- A script without `#version 2` is silently disabled in multiplayer (it still runs in
  single-player; section 9).
- `<screen script="...">` scripts run as version 1 (with `#version 2` the `client` / `server`
  tables do not exist), and version 1 scripts are disabled in multiplayer, so screens are
  blank there. A `<screen>` with no script logs `Script not found: data` each time one spawns.
  A screen is deleted together with the shape it sits on.
- **`GetScriptId()`** is undocumented (not in `script_defs.lua`) and returns the calling
  script's own entity handle.
- **A script that deletes its own entity inside its own callback crashes the game** ("General
  crash", empty Lua stack, an exception on a worker thread). Deleting another script's entity
  is safe (31 at once worked). Pattern: the script puts its handle in the registry and a
  long-lived script deletes it on its next tick. This includes tools: a bridge `run_lua` that
  deletes every script entity must skip `GetScriptId()`.
- **Script entities are not linked to what they spawned:** `GetEntityChildren(script, ...)`,
  `GetEntityParent(e, "", "script")` and `GetProperty(script, "file")` return nothing, bodies
  have no parent entity (`GetEntityParent(body)` is 0), and there is no `GetScriptPath`. The
  only link is the list `Spawn` returns, which includes the script handles.
  `FindEntities("", true, "script")` lists script entities (handles only).
- **Deleting a script entity leaves the bodies it spawned; deleting the bodies leaves the
  script running** (the built-in robot: without its script the legs and head stay; without
  its body the script keeps ticking at ~0.1 ms). Delete every handle `Spawn` returned.
- `ClientCall` made by a script that is deleted in the same tick is lost.
- Cost of a script with an empty tick: about 5 µs per frame. Many small scripts add up: about
  90 client scripts at 0.1 to 0.3 ms each took 11 to 12 ms of a frame in the profiler.
- *Observed:* `GetTime()` differed between scripts in the same frame (a script spawned later
  read about 30 s less than the level's scripts), as if it counted from the script's start.
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

*Observed:* a one-frame registry pulse set in one script's `update` can be missed by another
script that reads the registry less often; such readers must latch the value.

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

### Spawn and prefabs

- **Only the first root element of a prefab or XML string spawns:** a `<prefab>` with three
  root groups spawned the first one only.
- The top group's `pos` is ignored when spawning at a transform (a top group at y = 5 still
  lands at the transform).
- **Joints and ropes attach only to shapes from the same `Spawn` call**, unless the fourth
  argument (`jointExisting`) is true, which lets them attach to shapes already in the scene. A
  prefab split over several `Spawn` calls loses the attachments across the split.
- `Spawn("builtin-<id>:path.xml", t)` spawns a built-in mod's prefab from any mod's script; an
  inline `<instance file="builtin-...">` inside an XML string spawns nothing.
- `GetRopePointPosition` returns nil in the frame the rope was spawned; shape bounds work.
- **A rope from an XML string** (measured 2026-11-21, 2.2.0, multiplayer test):
  `<rope slack="..." color="r g b"><location pos="a"/><location pos="b"/></rope>`, the two
  locations in the spawn transform's space (or inside a `<body>`, in its space, as the built-in
  cranes do). Spawned with `jointExisting` true and each location just inside a shape, both
  ends attached (`GetJointShapes` gave 2) and the joint's type is `rope`. The rope replicates:
  the joined client found it with `QueryRaycastRope` under the same joint handle. `Delete` on
  the joint handle removes it.
- **Rope sag:** over 9 m, `slack="0"` still sagged 1.0 m at the middle; `slack="-0.3"` (as in
  the built-in truck prefab) 0.11 m, `-1` 0.11 m, `-3` 0.17 m. Over 12 m `slack="0.5"` sagged
  1.75 m. The rope had 32 points at slack 0.5 and fewer when pulled tight (22 at -0.3).
- **`jointExisting` lets a joint pick the wrong pair of shapes.** A hinge on the boundary
  between two bodies of the same `Spawn` joined them when spawned without `jointExisting`.
  With it, and other shapes near the joint (rock behind a static bracket, or an overlapping
  copy), a hinge joined two static shapes (bracket to bracket) and the dynamic body it was meant
  to hold had no joint and fell. Two of four such spawns in a cave lost their hinges this way.
  Pass `jointExisting` only when a joint must reach a shape outside the spawn.
- **Hinges from XML:** `<joint pos="x y z" rot="0 90 0" type="hinge" size="0.15"
  collide="false"/>` as a child of a `<body>`, at a point on the boundary between that body's
  shape and a shape of another body in the same spawn, joins the two (`GetJointOtherShape`
  returns the other one). `DetachJointFromShape(joint, shape)` for each joint lets the body go.
- **`QuatEuler(x, y, z)` applies Y, then Z, then X** (checked numerically).
- Static shapes spawned with `allowStatic = true` and no `<body>` go into the world body
  (`GetShapeBody(s) == GetWorldBody()`); `GetBodyShapes(GetWorldBody())` lists them in
  ascending handle order.
- **Spawn cost follows the shape's (cropped) box of cells, empty air included:** hidden floors
  of 4 to 8 M cells froze the game the most; halving the voxel resolution helped, hollowing a
  model did not; a 1.6 M-cell voxbox and 1600 tiny bush shapes felt instant. Multi-second
  freezes came from a single `Spawn` of a big file. (Measured by feel and `GetFps`.)
- `SetShapeLocalTransform` on a world-body shape costs about 0.28 ms; a pass over ~10 000
  shapes froze the game for 2 to 5 s.
- A changed shape local transform is synced to clients; setting the same value again is not.

### Deleting things

Tested on host and joined client in the multiplayer test; none of these crashed:

- deleting a shape a rope, hinge or ball joint is attached to: the joint stays and
  `GetJointShapes` loses that shape (also for a joint attached with `jointExisting`);
- deleting a body whose shape holds a joint (the joint is left with no shapes), then the joint;
- deleting a vehicle's shapes, its joints a frame later, then the vehicle, also next to the
  joined client's player;
- moving 400 bodies 200 m with `SetBodyTransform` and deleting their shapes in the same frame
  or the next; the same with 300 world-body shapes moved by `SetShapeLocalTransform`;
- deleting script entities spawned with a prefab (6 and 4 at once);
- `Delete` on an invalid or already deleted handle.

The one deletion that crashes is a script deleting its own entity (section 1).

### Shape editing (measured 2026-11-20, 2.2.0, local multiplayer test, host and joined client)

- **Brush edits made by the server reach joined clients.** A static body spawned from XML
  (`<voxbox size="120 40 60" material="rock"/>`), then `SetBrush("sphere", 16, 0)` and
  `DrawShapeLine(shape, x0,y0,z0, x1,y1,z1)` in the shape's voxel coordinates: the joined
  client reported the same `GetShapeVoxelCount` right after. Brush material index 0 removes
  voxels; any other index adds that palette entry, or recolours with the `paint` flag.
- Brush `size` is the diameter in voxels and 16 is the maximum: a size-16 sphere line removes a
  round section about 1.6 m across at 0.1 m voxels. Wider cuts need several lines, or
  `DrawShapeBox` with `SetBrush("cube", 1, 0)`.
- `DrawShapeLine` and `DrawShapeBox` clip to the shape: a line starting outside cut only
  inside and did not grow the shape.
- Cost: a 256 × 128 × 256 voxel shape (8.4 M voxels) cut by 80 brush lines and 6 boxes in one
  server call gave no frame over 33 ms (but see the 33 ms dt cap in 11b), and two such shapes
  (13 M voxels) rendered at 70 FPS afterwards.
- `CreateShape(body, transform, "MOD/x.vox")` makes a 1 × 1 × 1 shape with that file's
  palette, so any of its entries can be drawn with `SetBrush`. `ResizeShape(s, 0,0,0,
  127,127,127)` then gave 128³ with a zero offset. One small "palette" .vox holding every
  material you need is enough to build multi-material shapes at runtime.
- **Brush edits never split a shape.** A block cut free on all sides with `DrawShapeBox` stayed
  part of the static shape, hanging in its slot; a `MakeHole` next to it did not split it
  either. `IsShapeDisconnected` reported true on large carved shapes, yet `SplitShape(shape,
  false)` then made only a few shapes of 1 to 12 voxels, and `SplitShape(shape, true)` removed
  such residual voxels.

### Materials and .vox files

- **Palette index → material** in a `.vox` file (all 255 entries read back with
  `GetShapeMaterial`): 1-8 glass, 9-24 foliage, 25-40 dirt, 41-56 rock, 57-72 wood,
  **73-104 masonry, 105-120 plaster, 121-136 metal, 137-152 heavymetal, 153-168 plastic,
  169-176 hardmetal, 177-184 hardmasonry, 185-192 ice**, 193-224 none, 225-240 unphysical,
  241-253 none, 254 unphysical, 255 none. (Older tables online with masonry 73-88 and
  hardmetal 153-168 are wrong for 2.2.0.) A MATL chunk's `_emit` shows as the emissive value.
- "unphysical" voxels (225-240) render normally and `GetShapeMaterial` reports them as
  `unphysical`. Whether a player can walk through them was not tested.
- `MakeHole(pos, r0, r1, r2)`: r1 ≤ r0 and r2 ≤ r1, as documented; a soft radius of 0 with a
  medium radius makes nothing. Voxels removed from a 1 m cube at radius 0.4:

  | Material | soft only | soft + medium | all three |
  |---|---|---|---|
  | dirt, plaster, wood, ice, plastic | cut | cut | cut |
  | masonry, metal | — | cut | cut |
  | hardmasonry, hardmetal | — | — | cut |
  | rock, heavymetal | — | — | — |

  `rock` and `heavymetal` are not removed by `MakeHole` at all (radius 1 tested), which makes
  them good for boundaries a player must not dig through; brush edits still cut them.
- Vox file paths: an absolute path fails (`File not found data/vox/C:/...` in the log). `MOD/`
  is the calling script's mod; `MOD/../<other mod folder>/x.vox` reaches another local mod.
- **A scaled `<vox>` makes a body static.** `<body dynamic="true"><vox file="x.vox"
  scale="0.5"/></body>` spawned with `IsBodyDynamic` false and mass 0, with or without
  `allowStatic`, and also as `<vox ... prop="true"/>`; `SetBodyDynamic(b, true)` changed
  nothing. The same file without `scale` spawned dynamic. A body holding a plain voxbox as
  well as the scaled vox was dynamic, with the voxbox's mass only. Use 0.1 m voxels for
  anything physical; scaled models are fine for static bodies (held tools, decoration,
  client-side visuals).
- `.vox` axes: vox (x, y, z-up) becomes Teardown (x, y-up, -z).
- **A `<vox>` without `pos` is centred on its body origin on all three axes**, the height
  included (measured 2026-11-21, 2.2.0, files with only SIZE and XYZI chunks):
  `GetShapeLocalTransform` put the voxel corner (floor(sx / 2), floor(sy / 2), floor(sz / 2))
  of the file's SIZE at the origin (SIZE 7 × 14 × 7: y -0.3; 12 × 25 × 12: y -0.6; 10 × 16 ×
  14: y -0.7). So a model built standing on z = 0 hangs half its height below the body origin,
  and a body spawned at floor level starts half inside the floor (it wedged there, asleep, and
  constraints could not move it). `<vox pos="0 h 0">` with h = floor(sz / 2) × voxel size puts
  the model's bottom at the origin. (An earlier note here, from a vehicle porting project, said
  the origin is the bottom centre; that is wrong for these files.)

### Destruction and debris

- A piece cut loose from a body becomes a **new dynamic body** with **no tags** (the tagged
  original keeps the rest); both report `IsBodyBroken` true. It appears on the next physics
  step, not within the call that cut it. Same handle on joined clients, and a server `Delete`
  removes it there too.
- **Handle numbers are reused** after bodies are deleted: a piece broken off after many
  deletions got a handle lower than bodies created earlier. To find "bodies created since X",
  keep the set of handles that existed at X; "handle greater than the last one" misses some.
- **Dynamic bodies below about y = -20 freeze.** Measured 2026-11-21 (2.2.0, multiplayer
  test, a custom level whose own geometry ends at y = -1): a body that falls or is spawned
  below about y -20.1 is put to sleep every step and hangs in mid-air. It stays
  `IsBodyDynamic` true; `SetBodyActive(b, true)` plus a velocity moves it 1 to 5 cm before it
  sleeps again. Boxes dropped from -15 stopped at -20.16 to -20.25 in a row, over a floor
  1.7 m lower that raycasts saw; boxes spawned at -21.3, -25 and -45 never moved; boxes at
  y 20 to 120 fell normally. Static geometry at y -60, in the level XML or spawned at runtime,
  did not move the limit. Bodies driven by `ConstrainVelocity` every physics step (with
  `SetBodyActive`) still moved below it. Not checked: whether built-in levels, with deeper
  terrain, have a different limit. Keep anything that must fall (debris, props, collapsing
  parts) above y -20; players have their own limit near -30 (section 6).
  (An earlier note here said a cut-free block "rested on a sliver" when it hung asleep 1.5 m
  above the floor; it was measured below y -20, so this freeze is the likelier cause.)
- **A cut-through post keeps holding.** A 0.3 m wooden post (dynamic) holding up a hinged
  slab, cut through 1 m up with `MakeHole`: the piece above the cut slid down onto the stump
  and the slab rested on it again. `MakeHole(p, 0.08, 0.05, 0)` (a rifle chip) only nicked it;
  eight hits did not cut it. To make a support fail, remove its pieces.
- `MakeHole` on a small dynamic body can split it, and **the original handle may keep the
  small piece**: a script that kept driving the handle moved a sliver while the bulk fell
  away. Do not cut holes into bodies whose handle you keep using.
- An `Explosion` also leaves **local debris in every game**: broken dynamic bodies with
  negative handles (47 on the host, 53 on the joined client after one size-2.5 explosion),
  besides the server's positive-handle pieces. A server-side cleanup never sees a joined
  client's; each client's own script can `Delete` its own. Unbroken negative-handle dynamic
  bodies near the players are their held tools: leave them alone.

### Bodies moved by script

- **Constraint-driven bodies fall asleep.** A 545 kg body driven each physics step with
  `ConstrainVelocity` stalled against a 0.3 m ledge, went inactive, and the constraints no
  longer moved it, for many seconds. Calling `SetBodyActive(b, true)` before constraining,
  plus an upward `ConstrainVelocity` while forward speed stays low, kept it walking. A 61 kg
  body with small feet did not get stuck the same way.
- **A soft-material body pushed along the ground breaks itself apart.** A 300 kg plastic body
  driven with `ConstrainVelocity` lost a third of its voxels in 20 s of walking over a rough
  cave floor (968 to 627; no explosion, no shooting), the pieces coming off as debris, until
  almost nothing was left under its handle. The same model in hardmetal (palette 169-176,
  about 5 times the mass) walked 27 m and fought in a full crowd with no voxel lost. Make
  script-driven walkers of hard material, scale the constraint impulses with
  `GetBodyMass`, and give `MakeHole` a hard radius where it must still cut them.
- Such an upward "step-up" push must test for a real ledge: lifting whenever something blocks
  at foot height but not at step height walked bodies up gently curved cave walls. Find the
  obstacle's top (a ray down from step height just ahead) and lift only when it is lower than
  one step.
- **Carrying a player along a line:** `SetPlayerVelocity(v, p)` from `server.update` each step,
  with v = the ride speed along the line plus 10 × (target point - player position), held the
  host and a joined client 2 m under a 9 m rope while they moved along it with W/S read through
  `InputDown("up", p)` / `InputDown("down", p)` on the server. The joined client's own game put
  it within 0.1 m of the server's position. Releasing (no more velocity set) dropped them
  normally. Smoothness on the joined client was not judged by eye.
- **Let script-driven walkers pass through each other:** `SetShapeCollisionFilter(shape, 2,
  255 - 2)` on each of their shapes (server side) keeps them colliding with the level (level
  shapes read layer 1, mask 255 from `GetShapeCollisionFilter`) and with players, but not with
  each other. Call `QueryRejectBody` for each of them before `PathPlannerQuery` too, or paths
  bend around walkers that have already moved on. Measured with 14 walkers: 11 reached their
  target in 20 s with this, against 3 while they collided. Steering them apart everywhere
  (separation) brought the jam back in narrow tunnels; apply it only near the target.

### Client-side entities and animation

- **Client code can animate local copies of parts over server bodies.** It cannot move a
  server body's shapes (see above), but it can `Spawn` its own bodies (negative handles, that
  game only) and `SetBodyTransform` them every frame, scaled vox models included
  (`<body><vox file=... scale="0.5"/></body>`, static). Each client reads the server body's
  transform (`GetBodyTransform` on the shared handle) and places its parts from it. About 200
  such bodies moved per frame (14 bodies with 14 parts each) ran at 67 FPS on the host and
  73 on the joined client. Make the parts from unphysical palette entries so they do not
  collide.
- Client code sees server tags: `FindBodies("tag", true)` and `GetTagValue(b, "key")` on the
  joined client found server bodies and their tag values.
- Hand-animated falling parts: before the landing raycast call `QueryRequire("static")`, or
  parts come to rest on dynamic bodies and hang in the air when those move away.

### Light and environment

- A dark area needs a fixed exposure: `SetEnvironmentProperty("exposure", 1, 1)`. With the
  default range, auto-exposure lifted an unlit cave (ambient 0.02, no sun) to an even grey.
- **The player's flashlight** is toggled with F (input `flashlight`;
  `SetPlayerParam("flashlightallowed", false, p)` takes it away). `GetFlashlight(p)` returns a
  client-side light (negative handle; intensity 30, colour 0.85 0.85 1). Client code can switch
  it on with `SetLightEnabled(GetFlashlight(GetLocalPlayer()), true)` (host and joined client).
  A `ClientCall` doing that in the same tick as a server teleport worked for the host but not
  for the joined client; doing it from the client's own tick a moment after it saw the change
  worked on both. It is a real shadowed cone, far better than a `PointLight` at the camera. In
  one measurement the two players' flashlights cost about 15 % of the frame rate.

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

- **Depth limit:** a player below about y = -30 is put back at the spawn point with full
  health (platform at -25 held, -33 and lower reset; +70 held). A static slab at y = -101 in
  the level XML did not lower the limit. Build underground areas above -30, or move the whole
  level up. Dynamic bodies already freeze below about -20 (section 5, "Destruction and
  debris"), so anything with physics belongs above -20.
- A player teleported by the server onto geometry the server built in the same tick fell
  through on the joined client (it had not received the shapes yet), dropped below the depth
  limit and respawned. Teleporting 1.5 s after the build worked; check positions afterwards
  and teleport again if needed.
- `ApplyPlayerDamage(p, amount, cause)` (server) lowers health and shows the game's hurt
  effect. At health 0 the player stays down until a script respawns them (no automatic
  respawn in a level without `sandbox.lua`). `RespawnPlayerAtTransform(t, p)` brought a downed
  player back there with full health and their tools; their flashlight was off again.
- **A custom tool whose engine ammo is 0 gets no `usetool` input.** Holding the mouse button
  with a `RegisterTool` gun in group 2 never made `InputDown("usetool", p)` true on the server;
  after `SetToolAmmo(id, 100, p)` it did. A tool in group 1 worked at ammo 0. Without
  `sandbox.lua` nothing sets ammo for you; keep `SetToolAmmo` above zero (the number shows in
  the game's tool bar, so it can mirror your own ammo count).
- A held tool sits exactly at the camera unless `SetToolTransform` is called every frame, in
  every game state where the player can hold it (authoring.md, section 10).
- **The engine resets the player's walking speed every frame:** call `SetPlayerWalkingSpeed`
  in every `server.tick`.
- `DisablePlayerInput(p)` plus `SetPlayerHidden(p)` in every `server.tick` freeze and hide a
  player; that player's own client can still read `InputDown` / `InputPressed`.
- Players in version 2 have a visible body, also in vehicle seats: version 1 tricks that faked
  a player model now show doubles.
- `PauseMenuButton(title, true)` is the single primary mod button below "Main menu", shared by
  all mods; `"main_top"` adds above it; `"main_bottom"` is documented but never drawn.
- The old version 1 trick of finding a script's own handle through a pause-menu button (look it
  up in `game.pausemenu.items`) does not work in a version 2 server half: use `GetScriptId()`.
- Signatures that changed from version 1: `GetPlayerTransform(true)` became
  `GetPlayerTransformWithPitch()`; `SetPlayerTransform(t, true)` became
  `SetPlayerTransformWithPitch(t)` (the second argument is now a player id);
  `Shoot(origin, dir, [type], [strength], [maxDist], [playerId])`; `QueryShot(...)` returns
  `hit, dist, shape, playerId, damageFactor, normal`.

## 7. Lua sandbox and files

- Lua 5.1 (not LuaJIT). Present: `loadstring`, `load`, `loadfile`, `dofile`, `setfenv`,
  `getfenv`, `coroutine`, `string`, `table`, `math`, `newproxy`, `_G` (wrapping API globals
  through `_G[name]` works). Absent: `io`, `os`, `debug`, `require`, `package`, `ffi`. A
  script cannot write a file and mods cannot load native code.
- `collectgarbage("count" / "stop" / "restart" / "setpause", n)` work, per Lua state (each
  script half has its own). Many small tables per frame on a small heap mean many full
  collection cycles: a pause of 500 instead of the default 200 cut a heavy script's tick time
  by about 25 %, for a bigger heap.
- `loadfile` / `dofile` read any file on disk, live, by **absolute path**. `MOD/...` and
  relative paths fail ("No such file or directory"); `MOD/` is only understood by engine
  functions and `#include`.
- `HasFile("MOD/x")` checks the disk live. Absolute paths fail there too, and
  `Spawn('<script file="C:/...">')` loads nothing. From another mod's script (the bridge's
  `run_lua` included) `MOD/` is that other mod; to make a mod spawn its own files on request,
  give it a hook such as a registry key it polls (`level.<mod>.request = "MOD/x.xml"`).
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
- Because keys are per mod id, a copy of a mod in another folder starts with its own empty
  settings. To try another version of a mod with the user's settings, point the mod folder (a
  junction) at the other checkout instead of copying it.

## 9. Logs and errors

- Normal game: `%LOCALAPPDATA%\Teardown\log.txt`, written out completely only at a normal
  exit (it stayed empty during single-player play; the Teardown-PhysX project says the same).
  The live log is in the engine profiler's Log window. Multiplayer test: `log_0.txt` (host),
  `log_1.txt` (client), written in 4 KB blocks while running, so the newest lines lag.
- `log_sentry.txt`: the log of the last session that crashed, written by the crash handler at
  the crash (with line numbers, session time, level and tags such as `[NoTag|Network]`).
- Logged with script path and line: wrong-side calls, missing prefabs / sounds / vox files,
  disabled scripts. Lua runtime errors also show as red text on screen: `screenshot`.
- A `#version`-less script in multiplayer is logged as `Single player script disabled`.
- **A mod script's `print()` never reaches the log**, and `DebugPrint` only shows on screen.
  The way to write text into the log is a `LoadSound` of a missing file (section 7); give each
  line a unique name (a counter) so every call is logged.
- `log_0.txt` / `log_1.txt` are overwritten when the multiplayer test restarts: copy them first
  after a crash.
- *Observed,* client log lines in multiplayer that point at a host / client mismatch:
  `shape is null in handlePaint` (paint for a shape the client does not have, such as one just
  deleted) and `Init for unknwown script` (sic; a script init for a script the client does not
  know, such as one spawned and deleted in the same tick; a spawn and delete in one tick also
  dropped a joined client once).
- *Observed:* a sound loop started by the server reached a client as the host's absolute
  path (`Couldn't find sound loop: C:/Program Files (x86)/...` on a client whose mod was on
  another drive). Clients with the mod in another Steam library may not hear such loops.
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
- The windows run `teardown_modtest.exe`, not `teardown.exe` (process watchers must look for
  that name), and **write no crash report when they die**: the window closes and the log stops.
- Two windows of a heavy mod used 10 to 12 GB of RAM together, often leaving 100 to 500 MB free;
  some test-window deaths may come from that setup rather than the mod.

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
- The full script list ("...") scrolls further than it looks; selecting a script and reading
  "Instances N" is the only way to see how many copies of a script are loaded.

## 11b. Crashes, hangs, performance (measured 2026-11-20, 2.2.0)

- **The game writes its own crash report** to `%LOCALAPPDATA%\Teardown\crash\<date-time>_Steam_LOCAL-BUILD\`:
  `error.yaml` (JSON despite the name: `message`, `callstack_lua`, `version`, `memory` with
  registry size and entry count, video memory, audio, shadow volume and voxel memory by source
  (scene XML / destruction / spawned), GPU capabilities, `active_mods`), `callstack.txt` (native
  stack, innermost first; bare hex numbers are offsets in teardown.exe), `minidump.dmp`,
  `dxdiag.txt`, a copy of the `teardown.exe` that crashed, and `crashData.zip` with the log up
  to the crash. `crash_report` reads all of it.
- The crash folder keeps only the newest 5 reports (a sixth drops the oldest).
- Five reports on this machine (Nov 17 to 19; each held a copy of `teardown.exe`, so they came
  from the normal game, not the multiplayer test, whose windows write none) all said "General
  crash" with an empty Lua stack. Three had the same stack, inside physics: GameTick >
  SceneUpdate > PhysicsUpdate > two unnamed functions > ShapeInterpTransform > crash (names
  from the Teardown-PhysX table, see below). `active_mods` listed no mod as active even with a
  workshop mod's level running: it describes globally enabled mods, not the level's own.
- The game was updated between Nov 17 and Nov 19 without the version string changing ("2.2.0"):
  teardown.exe's PE timestamp went from 0x6ae468bd to 0x6afda5ed and the crash offsets moved.
  Compare stacks only within one build (`exe_build_stamp` in `crash_report`), or by function
  name: names come from a table made for exactly that build (`server/names/`), ported to new
  builds with `tools/port_names.py`. Ported to 0x6afda5ed, the Nov 19 crash has the same chain
  as the Nov 17 ones (GameTick > SceneUpdate > PhysicsUpdate > ... > ShapeInterpTransform): the
  same crash, still there after the update.
- `teardown_modtest.exe` (the multiplayer test) is a separate build of its own (0x6afda615 next
  to teardown.exe's 0x6afda5ed) and has its own table.
- *Observed* in a large mod port, single-player and multiplayer: the same class of crash,
  an access violation inside the physics step (host: `PhysicsUpdate` > two functions >
  `ShapeInterpTransform` > ...; client: another path in `PhysicsUpdate`), with registers holding
  text from reused memory: a freed object read during physics. No single deletion or move
  pattern reproduced it (see "Deleting things"); in a long two-window run with constant
  spawning, a cleanup deleting up to 120 objects per frame and a 200 m world shift every few
  seconds, a window died at random after 30 s to 25 min whatever was switched off. One run per
  variant cannot tell a cause from luck: repeat runs for A/B tests of rare crashes.
- Reading a minidump's exception record by hand: stream type 6 holds the code, address and
  parameters, then the thread CONTEXT (RAX..R15 from offset 0x78, RIP at 0xF8); about 40 lines
  of Python. 0xC0000005 with parameter 0 is a read violation. Register values that decode as
  ASCII text mean the engine dereferenced freed memory that had been reused.
- Exit codes (`game_health` remembers each game process it has seen): 1 after the multiplayer
  test's stop (a forced kill); a native crash would show 0xC0000005 or similar.
- `dump_game` on the multiplayer test host: the main thread was in the NVIDIA driver
  (rendering); worker threads wait in `WaitForSingleObjectEx`. A 0.9 MB dump took 0.5 s.
- `perf_sample` in the playground (64 bodies): host 88 FPS (frame p50 10 ms, p99 27 ms), the
  joined client 74 FPS, on this laptop with both windows open. Game seconds matched wall
  seconds. `FindBodies("", true)` and the other `Find*("", true)` return every entity.
- **Frame dt as `client.tick` sees it is capped at 1/30 s (33.33 ms):** frames that took longer
  report exactly 33.33, so hitches longer than that are invisible in dt (and
  `frames_over_50ms` from `perf_sample` is always 0). Count the frames at the cap instead.
- Within one `batch` of bridge actions everything runs in the same game tick: waiting
  between them with `run_lua` does not let physics or timers move (bridge.md).

### From the Teardown-PhysX project (not verified here)

[Teardown-PhysX](https://github.com/slopkhayzo/Teardown-PhysX) is an MIT-licensed native
plugin that hooks the engine; its notes are from reverse engineering, mostly AI-generated by
its own account. Facts it relies on that matter for mod work:

- Game states: NONE, SPLASH, MENU, UI, LOADING, MENU_LOADING, PLAY, EDIT, QUIT. The log has
  "State switch: MENU -> LOADING" lines (seen in the profiler's Log window).
- The scene updates in 60 Hz ticks (several per frame when frames are slow).
- The game's debris cleanup deletes small loose pieces at rest unless they are within 5 m of
  the player, or within 30 m and in view. Small debris does not collide with other debris.
- Lua `math.random` uses the C runtime's `rand()`; the game seeds it from the clock.
- Starting a level natively takes mission, path, layers and mod: built-in levels by file name
  resolved under `data/level/` (mission `lee_sandbox`, file `lee.xml`, layers `sandbox`);
  mod levels as `<mod folder>/<file>` with the mod id, which is what `Command("mods.play", ...)`
  does. That fits our failed `StartLevel(..., "data/level/hub.xml")`, which was loaded as
  `data/level/data/level/hub.xml`: the file name alone is probably the right form.
- teardown.exe's code is encrypted on disk (SteamStub) and decrypted at start.
- A normal exit saves mod lists and options; killing the process does not.

## 12. Not verified

- Why a `Shoot` in the first second after a level load does nothing (seen twice; an
  `Explosion` 1.5 s after a load did work).
- Gamepad input: `send_input` sends keyboard and mouse only; a virtual pad would need a
  driver such as ViGEm.

- Whether sounds, particles and lights started by server code reach joined clients (weather
  set by the server does).
- A working `StartLevel` path form for built-in and mod levels (see the Teardown-PhysX notes
  in 11b for the likely one).
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
