# Authoring patterns that were built and run

Everything here was written, loaded and watched working through the bridge (Teardown 2.2.0,
2026-11-17, local multiplayer test). The full sources are the playground mod in the
teardown-mcp repo (`playground/`: `main.xml`, `prefab/buggy.xml`, `prefab/blaster.xml`,
`script/buggy.lua`, `script/range.lua`, `script/director.lua`, `script/gate.lua`,
`script/blaster.lua`, `script/menu.lua`). Copy from here before inventing.

## Contents

1. A level that loads
2. Shapes from XML
3. A vehicle from scratch
4. Server logic, client presentation
5. HUD and world-space UI
6. Camera
7. Weather and look
8. Shooting and destruction
9. Joints and triggers
10. A custom hand tool
11. A clickable menu
12. Development loop that worked
13. Game modes
14. Finding and fixing what costs frame time
15. Mod files, assets and other mods
16. Models built by a script (`tools/vox.py`)
17. Animated models (`<animator>`, `tools/td_anim.py`)

## 1. A level that loads

A mod folder with `info.txt` (`name`, `author`, `description`, `tags = Map`, `version = 2`) and
`main.xml`:

```xml
<scene version="3" shadowVolume="200 100 200">
	<environment template="sunny" skyboxrot="-45"/>
	<voxbox pos="-100.0 -0.1 -100.0" size="2000 1 2000"/>
	<spawnpoint pos="0.0 0.0 0.0" rot="0.0 180.0 0.0"/>
	<boundary>
		<vertex pos="-100.0 -100.0"/> <vertex pos="100.0 -100.0"/>
		<vertex pos="100.0 100.0"/> <vertex pos="-100.0 100.0"/>
	</boundary>
	<script file="main.lua"/>
	<script file="sandbox.lua"/>
	<script file="MOD/script/mine.lua"/>
</scene>
```

- **`<spawnpoint rot="0 0 0">` faces +z; `rot="0 180 0"` faces -z**, as above (measured
  2026-11-28 by the skill-test session: at rot 0, `TransformToParentVec(GetPlayerTransform(1),
  Vec(0, 0, -1))`, the way the player looks, was (0, 0, 1), and a screenshot showed the level
  behind the player; after rot 180 the joined client spawned facing -z). Vehicles differ:
  their forward is their local -z (section 3).
- `main.lua` and `sandbox.lua` without `MOD/` are the game's own scripts (`data/script/`):
  they give players tools, respawn and the sandbox rules. Keep them unless the mod replaces
  them.
- Environment templates: `sunny`, `night`, `sunset`, `sunrise`, `foggy`, `rain`.
- A brand-new local mod folder starts in the multiplayer test straight away; the main game
  does not need to have listed it.
- This level reloads in about 2 seconds, which makes it a good place to develop a script
  before moving it into a heavy mod.
- A 0.1 m thick ground lets debris sink through (a broken piece ended at y = -1). Use a
  thicker floor where things get destroyed.

## 2. Shapes from XML

- `<voxbox pos="x y z" size="w h l" color="r g b" material="wood"/>`: `size` is in voxels
  (0.1 m), `pos` is the box's corner in metres, relative to the parent. `material="wood"` gave
  a shape that `GetShapeMaterialAtPosition` reports as `wood` and that breaks when shot.
  Without a material a voxbox is indestructible.
- Wrap shapes in `<body dynamic="true">` for a physical prop, plain `<body>` for a static one.
  `tags="a b=1"` on any element.
- `Spawn(xml, transform)` takes this XML as a string, or a prefab file (`MOD/prefab/x.xml`,
  root `<prefab version="2.0.0"><group>...`). It returns every handle it made, the body first
  for a single-body string; pick by `GetEntityType(h)`.

## 3. A vehicle from scratch

```xml
<vehicle name="buggy" tags="mybuggy" sound="classic1" spring="0.5" damping="0.7"
         topspeed="60" acceleration="4" strength="4" steerassist="0.1">
	<body pos="0.0 0.6 0.0" dynamic="true">
		<voxbox pos="-0.8 0.0 -1.5" size="16 5 30" color="0.8 0.2 0.1"/>
		<location name="player" tags="player" pos="0.0 0.9 0.0"/>
		<location tags="exit" pos="-1.6 0.3 0.0"/>
		<wheel name="fl" pos="-0.95 -0.25 -1.1" steer="1" travel="-0.2 0.1">
			<voxbox pos="-0.1 -0.35 -0.35" size="2 7 7" color="0.1 0.1 0.1"/>
		</wheel>
		<!-- fr the same with x mirrored; bl / br with drive="1" instead of steer="1" -->
	</body>
	<rig name="driver" tags="driver sort=0">
		<location name="seat" tags="seat" pos="0.0 0.9 0.3"/>
	</rig>
</vehicle>
```

- **Wheels go inside `<body>`.** As siblings of the body they are inert boxes: the vehicle
  rests on its belly and `DriveVehicle` does nothing. No error is logged.
- Forward is -Z: front wheels at negative z, `steer="1"` on the front, `drive="1"` on the rear.
- `DriveVehicle(vehicle, drive, steering, handbrake)` from server code moves it with nobody
  inside (drive 0.6, steering 0.5 gave a 30 km/h circle). Call it every tick.
- `SetPlayerVehicle(vehicle, playerId)` (server) seats a player; `GetPlayerVehicle(p)` then
  returns the handle.
- Speed: `VecLength(GetBodyVelocity(GetVehicleBody(v))) * 3.6` km/h; readable on clients.
- Driven by hand (player seated, `w` held 2.5 s through `send_input`) it reached 55 km/h;
  `e` got the player out.

## 4. Server logic, client presentation

The shape that worked for all three mini-mods:

```lua
#version 2

function server.init()
	-- create things, publish what clients need
	shared.thing = { vehicle = handle, score = 0 }
	SetInt("level.thing.score", 0, true)        -- mirror for tools and other scripts
end

function server.tick(dt)
	if GetBool("level.thing.go") then ... end   -- a switch you can flip from outside
	ClientCall(0, "client.thingFx", x, y, z)    -- 0 = every client, the host included
end

function client.init()  sound = LoadSound("tools/gun0.ogg")  end
function client.thingFx(x, y, z)  PlaySound(sound, Vec(x, y, z), 0.6)  end
function client.draw()  local t = shared.thing  if not t then return end  ...  end
```

- `shared` belongs to **one script**. Another script, the bridge included, sees its own empty
  `shared`. Anything that must be visible outside the script goes into the registry.
- Nested writes work and arrive: `shared.t.n = shared.t.n + 1`, `shared.t.list[i] = {...}`.
- Registry switches (`level.<mod>.<flag>`) make a script drivable from the bridge:
  `registry_set` flips them. Set them with `sync: true` if client code reads them, or the
  joined client never sees the change.
- Per-shot effects: send plain numbers through `ClientCall` and build the `Vec` on the client.
  Load sounds in `client.init` (client-side loads are not replicated).

### Sending effects to clients efficiently

Effects (particles, sounds, lights, lines, sprites, camera shake, UI) change nothing the
server needs to know. Measured (quirks.md section 4): when the server half calls
`SpawnParticle`, `PointLight`, `DrawLine` or `PlaySound`, each call is sent to every client;
`ClientCall(0, "client.f", args)` is one message per call, and `ClientCall(p, ...)` reaches
only player p. So:

- **Send the event, not the effect.** On a hit, call `ClientCall(0, "client.hit", x, y, z,
  kind)` once; `client.hit` spawns the 20 particles, plays the sound and adds the light in
  every game. Twenty server-side `SpawnParticle` calls would each travel.
- **Never repeat an effect call from the server every tick.** A server `PointLight` or
  `DrawLine` each tick is a message per tick per client. Send a start (and an end), or put a
  value in `shared` and let each client draw from it every frame (a glow that follows a body:
  the client reads the body's transform itself, nothing needs sending).
- **Things each client can see for itself need no message at all:** a body's transform,
  tags, `shared` values. Client code reads them directly (Hive Breach's legs, flares' sparks,
  screens are drawn that way).
- Effects only one player should see (their own hit marker, a private hint): `ClientCall(p, ...)`.
- Keep `ClientCall` arguments small and plain (numbers, short strings); send a handle or an id
  rather than a table describing a thing the client can look up.
- What must stay identical for everyone (bodies, destruction, player health) belongs to the
  server and replicates by itself; do not mirror it with calls.
- **The player's own action plays at once on their own client** (official tutorial, part 7):
  input goes to the server, the server acts and calls back, and over a real network that
  round trip shows as input lag. Detect the same input in `client.tick` and play the
  sound, flash and recoil there; the server does only the consequence (damage, spawning) and
  tells the other players. Section 10 has the pattern. The local multiplayer test has almost
  no latency, so it does not show the lag.

## 5. HUD and world-space UI

- The canvas is 1920 wide whatever the window: the multiplayer test's half-screen window is
  1920 x 2280. A 28 px font is tiny there; 40 to 64 reads well.
- Label over an entity:

```lua
local x, y, dist = UiWorldToPixel(worldPos)
if dist > 0 then            -- in front of the camera
	UiPush()  UiTranslate(x, y)  UiAlign("center middle")
	UiFont("bold.ttf", 28)  UiTextOutline(0, 0, 0, 1, 0.2)  UiText("...")
	UiPop()
end
```

- Bar: a dark `UiRect(w, h)`, then `UiAlign("left middle")`, translate by half the width to
  the left, and a coloured `UiRect(w * fraction, h)`.
- Panel: `UiColor(r, g, b, a)` then `UiRoundedRect(w, h, radius)`, then translate inside it.
  `UiCircle(r)`, `UiTextShadow`, `UiAlign("right top")` all behaved as named.
- Always pair `UiPush()` / `UiPop()` around each element, and return early while `shared` is
  still nil (the first client frames).
- Scripts draw in the order they are listed in the level, on one canvas: two scripts that
  both use the top-right corner overlap. Give each HUD its own region.
- Fonts used: `regular.ttf`, `bold.ttf`.

### Screens in the world that work in multiplayer

Two ways, both shown on the host and a joined client (2026-11-22):

- **`<screen>`** on a shape, with a version 1 script (`init`, `tick`, `draw` as plain
  globals, full `Ui*` inside `draw`, 640 x 480 canvas). The script runs once, on the host;
  joined clients see its picture, so every player sees the same screen, and it reads the
  host's registry and entities directly (quirks.md section 1). It needs a shape to sit on,
  and gives a full UI canvas (buttons, images, fonts) that any player can operate.
- **`DrawSprite` quads** (below): no resolution, no shape needed, any size, on moving bodies,
  drawn from your own `#version 2` client code and `shared` state, but text needs a font made
  of images and the rules below.

Built and run with sprites (facts in quirks.md "World-space sprites"):

- A screen is a set of `DrawSprite` quads placed every frame from `client.render` (a screen
  on a moving body then stays on it): coloured rectangles from `gfx/white.png`, and text from
  one PNG per character (white on transparent, 2 px padding so filtering does not bleed;
  `DrawSprite` has no source rectangle, so no atlas).
- A ready library: the **Teardown Screens** mod (`C:\Projects\Teardown Screens`, linked as
  `mods/teardown-screens`). Reference it in place:
  `#include "../../teardown-screens/screens/screens.lua"` from `<mod>/script/` (relative to
  the top script) and `TDScreens.init("MOD/../teardown-screens/screens")`; the folder name
  becomes the library's Workshop id once it is published. Then `TDScreens.screen(t, w, h)`,
  `s:text`, `s:rect`, `s:bar`, `s:line`, `s:image`, custom fonts from TTF or hand-drawn
  bitmaps (`tools/make_font.py`), housings from `screens/housing.lua`. Its `README.md` and
  `docs/REFERENCE.md` cover the risks (a missing library stops the whole script; the
  multiplayer parity check does not cover included mods; quirks.md sections 7 and 10) and
  copying the folder as the fallback.
- Draw it from shared state in every game; skip it when the camera is far away.
- Build a housing: a dark back plate at least 15 cm behind the image (it is the screen's
  background), a bezel around the opening. No sprite may overlap another (no background
  rectangle under text, bars as two side-by-side parts).
- `PointLight` in front of the screen gives it a glow on the surroundings in the dark.
- For interaction, keep the full-screen UI opened by `interact` (section 11): world UI
  projected with `UiWorldToPixel` has no perspective and shows through walls.

## 6. Camera

```lua
function client.tick(dt)
	if GetBool("level.cam.orbit") then
		angle = angle + dt * 0.35
		local pos = VecAdd(CENTER, Vec(math.cos(angle) * 16, 7, math.sin(angle) * 16))
		SetCameraTransform(Transform(pos, QuatLookAt(pos, CENTER)))
		SetCameraFov(60)
	end
end
```

- Both calls are client-only and must be made every tick; stop calling and the normal camera
  returns.
- Each client runs its own camera: with a synced flag both players orbit, each at its own
  angle.
- To move a *player* instead (for a screenshot), use the bridge's `look_at`, or on the server
  `SetPlayerTransformWithPitch(Transform(feet, QuatLookAt(eye, target)), playerId)` with
  `eye = feet + Vec(0, 1.7, 0)`.

## 7. Weather and look

`SetEnvironmentProperty(name, ...)` is server-only and reaches joined clients. Names are not
case-sensitive. Present in 2.2.0 with their values in the `sunny` template:

`skybox` (day.dds), `skyboxtint` (r,g,b), `skyboxbrightness`, `skyboxrot`, `constant` (r,g,b),
`ambient`, `ambientexponent`, `fogColor` (r,g,b), `fogParams` (start, end, amount, exponent),
`fogscale`, `sunBrightness`, `sunColorTint` (r,g,b), `sunDir`, `sunSpread`, `sunLength`,
`sunFogScale`, `sunGlare`, `exposure` (min,max), `brightness`, `wetness`, `puddleamount`,
`puddlesize`, `rain`, `nightlight`, `ambience` (file, volume), `slippery`, `waterhurt`,
`snowdir`, `snowamount`, `wind`. Unknown: `template`, `snowonground`, `fogheight`.

Post-processing (`GetPostProcessingProperty`): `saturation`, `colorbalance` (r,g,b),
`brightness`, `gamma`, `bloom`. Unknown: `contrast`, `vignette`, `exposure`.

A dusk fade that worked: over 3 s lerp `sunBrightness` 6 → 0.6, `skyboxtint` to (1, 0.55,
0.35), `skyboxbrightness` 1 → 0.35, `fogColor` to (0.45, 0.25, 0.2), `rain` to 0.8, `wetness`
to 1.

A sky of your own: section 15, "Custom skybox".

## 8. Shooting and destruction

- `Shoot(origin, dir, "bullet", strength)` is server-only. **Strength scales the size of the
  damage, not just its force**: one strength-3 bullet at a row of wooden planks 3 m apart
  destroyed the one it hit and its neighbour and damaged the next. Use 1 for a normal shot.
- A `Shoot` issued in the first second after a level load had no effect (seen twice). Delay
  scripted shots.
- A hit dynamic prop breaks apart: the pieces are new bodies, and the original shape handle
  stays valid with few or zero voxels. Test `IsBodyBroken(body)` for "destroyed";
  `GetShapeVoxelCount` of the original shape is not a health value once it has split.
- `QueryRaycast(origin, dir, maxDist)` → `hit, dist, normal, shape` is the cheap way to check
  what a shot would hit first.

## 9. Joints and triggers

A gate on a motorised hinge that opens while a player stands in a zone:

```xml
<group name="gate" pos="14.0 0.0 -8.0">
	<body> <voxbox pos="-2.4 0.0 -0.2" size="4 30 4"/> </body>              <!-- static post -->
	<body dynamic="true" tags="gate">
		<voxbox pos="-2.0 0.3 -0.1" size="36 20 2" material="wood">
			<joint tags="gatehinge" pos="0.0 1.0 0.1" rot="90.0 0.0 0.0" type="hinge"
			       size="0.3" limits="-100 100" rotstrength="0" rotspring="0"/>
		</voxbox>
	</body>
	<trigger tags="gatezone" pos="0.0 -0.5 3.0" type="box" size="6 4.5 5"/>
</group>
```

```lua
function server.tick(dt)
	local occupied = false
	for p in Players() do                                  -- #include "script/include/player.lua"
		if IsPointInTrigger(zone, GetPlayerTransform(p).pos) then occupied = true end
	end
	SetJointMotorTarget(hinge, occupied and 90 or 0, 1.5, 500)   -- degrees, max speed, strength
end
```

- **A joint connects the shapes it overlaps.** It is written as a child of one shape, at a
  position relative to that shape, and it must reach into the other one: `size` is its reach
  in metres. A joint 5 cm outside its own panel attached to nothing (`#GetJointShapes(j)` was
  0, `IsBodyJointedToStatic(body)` false) without any error. Check those two after loading.
- `rot="90 0 0"` on a hinge gave a vertical axis (a door). `GetJointMovement(j)` returns the
  angle in degrees; the gate reached 89.9 within a second.
- **Trigger:** `pos` is the bottom centre, `size` is in metres (`GetTriggerBounds` confirmed
  it). `IsPointInTrigger`, `IsBodyInTrigger`, `GetTriggerDistance` (negative inside) work in
  server and client code.
- **A standing player's position is about 1 cm below the surface they stand on** (y = -0.01 on
  a floor at y = 0). A trigger whose floor is exactly at ground level does not contain them.
  Start the trigger below the floor, or test a point above the feet.
- A dynamic body that loads at rest is asleep (`IsBodyActive` false) and stays where it is,
  even in mid-air, until something touches it. An unattached gate hung in the air looking
  correct; a body staying put is not proof that a joint holds it.

### Things a player presses (levers, buttons)

Built and run by the skill-test session (2026-11-28, multiplayer test, both players):

- Tag the body or shape `interact`. The prompt text is the tag's value, but a value in XML
  `tags=` cannot hold a space (spaces separate tags). For a text with spaces set it from
  the script: `SetTag(lever, "interact", "Pull lever")` in `server.init`. (The game's own
  XML uses `interact=loc@KEY`, a key in its translation tables, which mods cannot add to.)
- Read the press on the server, per player:

```lua
function server.tick(dt)
	for p in Players() do   -- every player; needs #include "script/include/player.lua"
		if InputPressed("interact", p) and GetPlayerInteractBody(p) == lever then
			-- start the action here; tell clients with ClientCall for sounds and effects
		end
	end
end
```

## 10. A custom hand tool

Prefab (`prefab/blaster.xml`), modelled on the built-in minigun:

```xml
<prefab version="1.5.4">
	<group name="instance=MOD/prefab/blaster.xml">
		<group name="eye_space">
			<location name="fp_action" tags="name=fp_action" pos="0.25 -0.3 -0.2"/>
			<location name="fp_run" tags="name=fp_run" pos="0.25 -0.5 -0.2" rot="20.0 40.0 0.0"/>
			<location name="tp_action" tags="name=tp_action" pos="0.1 -0.5 -0.1"/>
			<!-- tp_run, tp_crouch likewise -->
		</group>
		<group name="tool_space">
			<body dynamic="false">
				<voxbox pos="-0.05 -0.05 -0.6" size="1 1 6" color="0.15 0.5 0.9"/>
			</body>
			<location name="tp_action_rh" tags="name=tp_action_rh ik_hand" pos="0.0 -0.1 -0.05" rot="0.0 90.0 0.0"/>
			<location name="muzzle" tags="name=muzzle" pos="0.0 0.0 -0.65"/>
		</group>
	</group>
</prefab>
```

```lua
function server.init()
	RegisterTool("mcpblaster", "Blaster", "MOD/prefab/blaster.xml", 6)
end

local function fire(p)
	local muzzle = GetToolLocationWorldTransform("muzzle", p)
	local hit, startPos, endPos, dir = GetPlayerAimInfo(muzzle.pos, 60, p)
	Explosion(endPos, 0.5)
	ClientCall(0, "client.blasterBeam", muzzle.pos[1], muzzle.pos[2], muzzle.pos[3], endPos[1], endPos[2], endPos[3])
end

function server.tick(dt)
	for p in PlayersAdded() do                 -- every player who joins, the host included
		SetToolEnabled("mcpblaster", true, p)
		SetToolAmmo("mcpblaster", 50, p)
	end
	for p in Players() do
		if GetPlayerTool(p) == "mcpblaster" and InputPressed("usetool", p) then fire(p) end
	end
end

function client.tick(dt)
	if GetPlayerTool() == "mcpblaster" then
		SetToolTransform(Transform(Vec(0.3, -0.3, -0.5)), 1.0)   -- every frame
	end
end
```

- **Responsive firing** (official tutorial, part 7: "all our internal tools are implemented
  this way"): run the input check twice, on the server for the effect on the world and in
  the client for the player's own feedback, so nothing waits for a network round trip:

  ```lua
  function client.tick(dt)
  	if GetPlayerTool() == "mcpblaster" and InputPressed("usetool") then
  		PlaySound(fireSound, GetCameraTransform().pos)       -- this player, now
  	end
  end
  function client.blasterBeam(p, ...)                       -- from the server's fire()
  	if p ~= GetLocalPlayer() then PlaySound(fireSound, ...) end   -- the others, a moment later
  	-- the beam itself to the real hit point, for everyone
  end
  ```

  The client's check must match the server's (tool, ammo, health, fire rate) or a player
  hears shots the server never fired. Ammo the server keeps in `shared` reaches the client
  late, so count the client's own unconfirmed shots against it.
  *Observed* (2026-11-26, 2.2.0, multiplayer test, an automatic rifle with this pattern, 12
  shots each): on the joined client all 12 were predicted and confirmed one to one. On the
  **host** 2 of the server's calls arrived before that frame's `client.tick` had made its
  prediction, as if the server's `ClientCall` reaches the host's client half within the same
  frame. Treat a confirmation that comes with nothing pending as the shot itself: play it and
  restart the client's fire timer, or the host hears that shot twice.
- **Without `SetToolTransform` every frame the tool sits exactly at the camera**: the whole
  view was the colour of the barrel. The `eye_space` locations alone do not place it (the
  built-in tools use `script/toolanimation.lua` or their own `SetToolTransform`).
  `SetToolTransform` is client-only.
- The tool shows up in the tool bar under its name once registered and enabled;
  `SetPlayerTool("mcpblaster", p)` (server) equips it. Its name is also readable at
  `game.tool.<id>.name`.
- `GetToolBody(p)` returns a negative handle: the held tool is a client-side entity.
- `GetPlayerAimInfo(pos, maxDist, p)` returns `hit, startPos, endPos, dir`: where the
  player's aim from `pos` lands. Works in server code for any player.
- A real click through `send_input` reached `InputPressed("usetool", p)` on the server and
  fired once. A registry switch (`level.blaster.fire = <playerId>`, consumed in
  `server.tick`) is still handy for firing without taking window focus.
- With the sandbox script loaded, `GetToolAmmo` reported 9999 (unlimited ammo) whatever
  `SetToolAmmo` set.

## 11. A clickable menu

Opened with a key, with buttons and a slider; each control reports to the server. Clicked and
dragged through `send_input`, results read back from the registry.

```lua
local open, slider = false, 0.5

function server.menuAction(action, value)            -- runs on the server for every client
	SetInt("level.menu.clicks", GetInt("level.menu.clicks") + 1, true)
	if action == "dusk" then SetBool("level.director.dusk", not GetBool("level.director.dusk"), true) end
end

function client.tick(dt)
	if InputPressed("m") then open = not open end      -- any key name works as an input
end

function client.draw()
	if not open then return end
	UiMakeInteractive()                                -- frees the cursor, blocks game input
	UiPush()
	UiTranslate(UiCenter(), 500)
	UiAlign("center top")
	UiColor(0.08, 0.09, 0.12, 0.92)
	UiRoundedRect(560, 520, 18)
	UiTranslate(0, 120)
	UiColor(1, 1, 1, 1)
	UiFont("bold.ttf", 36)
	UiButtonImageBox("ui/common/box-outline-6.png", 6, 6, 1, 1, 1, 0.8)
	UiButtonHoverColor(1, 0.8, 0.3, 1)
	if UiTextButton("Toggle dusk", 420, 70) then
		ServerCall("server.menuAction", "dusk", 0)
	end
	UiTranslate(-200, 260)
	UiAlign("center middle")
	local value, done = UiSlider("ui/common/dot.png", "x", slider * 400, 0, 400)
	slider = value / 400
	if done then ServerCall("server.menuAction", "slider", slider) end   -- on release
	UiPop()
end
```

- `UiTextButton(label, w, h)` returns true on the frame it is clicked. One real click gave
  exactly one action.
- `UiSlider(image, "x", current, min, max)` returns `value, done`; `done` is true when the
  drag ends. `current` and the range are in canvas pixels of travel.
- While `UiMakeInteractive()` is called each frame, clicks go to the UI and not to the held
  tool (the tool's shot counter did not move).
- UI state (`open`, `slider`) lives on the client; anything that must affect the game goes
  through `ServerCall`, and anything the UI shows about the game comes from synced registry
  keys or `shared`.

A text field (same `client.draw`, after the slider). `name`, `typing` and `focus` are client
locals:

```lua
UiAlign("left top")
UiColor(1, 1, 1, 0.8)
UiImageBox("ui/common/box-outline-4.png", 400, 44, 12, 12)   -- UiTextInput draws no frame
UiColor(1, 1, 1, 1)
UiFont("regular.ttf", 26)
local text, active = UiTextInput(name, 400, 44, focus)       -- focus: true for one frame
focus = false
if text ~= name then
	name = text
	ServerCall("server.menuAction", "name", name)
end
typing = active          -- in client.tick: ignore key bindings while typing is true
```

## 12. Development loop that worked

1. `mp_test_start { "mod": "<folder>" }` once.
2. Edit, `reload_scene` (2 s here), `read_log` for errors with the mod's name in `contains`.
3. Flip the mod's registry switches with `registry_set`, then `look_at` the action and
   `screenshot`. For the joined client's view: `screenshot` with `window: "TD - Client1"`.
4. When the picture and the numbers disagree, read the entities directly (a small Lua file
   with `FindBodies(tag, true)`, voxel counts, `IsBodyBroken`, positions) rather than
   trusting the mod's own counters. Sampling the same reader a few times a second showed the
   cause of a wrong score within one run.

## 13. Game modes

Read from Teardown 2.2.0's own files (2026-11-21): `mods/contentgamemodeexample` (Tuxedo Labs'
example), `mods/mpclassics` (Deathmatch, Team deathmatch, Capture the flag),
`data/script/sandbox.lua`, the pause menu in `data/ui/hud.lua` and the mod manager in
`data/ui/components/mod_manager.lua`; then measured through the bridge in the local
multiplayer test (2026-11-21, MCP Playground with its probe mode).

A mod declares game modes in a `gamemodes.txt` next to its `info.txt`, one `[Name]` section
each. There are two kinds, and **which one depends on the kind of mod**:

- **Content game mode, in a level mod** (a mod with `main.xml`): `layers = race`, optionally
  `mission = ...`. Nothing else is needed: the playground's `[MCP probe]` / `layers =
  mcpprobe` worked. The mode is part of the mod's own level: elements with `layer="race"`
  load only in that mode, `layer="-race"` only without it. The example puts `main.lua` and
  `sandbox.lua` in a `layer="-race"` group and the mode's script, goal trigger, spawn point
  and cars in a `layer="race"` group:

```xml
<group name="Default" layer="-race">
	<script file="main.lua"/>
	<script file="sandbox.lua"/>
</group>
<group name="Race game mode" layer="race">
	<instance pos="-38.3 3.2 -3.5" file="BUILT-IN/prefab/vehicle/land/musclecar_green.xml"/>
	<script file="MOD/race.lua">
		<trigger tags="goal" pos="40.3 0.0 -3.4" type="box" size="20 10 30"/>
		<spawnpoint pos="-42.3 3.2 -0.3" rot="0.0 90.0 0.0"/>
	</script>
</group>
```

- **Global game mode, in a global mod** (no level of its own; the player switches it on in
  the mod manager, like Multiplayer Classics): `path = deathmatch.lua` (relative to the mod
  folder). Its script runs on whatever level is played with the mode. `en_name`,
  `en_description` (and `de_name`, ...) and `preview` are optional display texts; the mode's
  name defaults to the section name. A `path` mode in a level mod is **not** offered: the
  playground with `path = script/probemode.lua` was never listed.

How the game lists and starts them:

- **Listing** (mod manager code, confirmed in game): the Play dialog of a mod shows that
  mod's own modes plus the modes of every **enabled** mod flagged as a game-mode mod
  (`mods.available.<id>.gamemode`). Global modes therefore appear only after their mod is
  enabled.
- **Registry:** each mode is `gamemodes.<mod id>.<key>`, where the key is the section name in
  lower case with spaces as `-` (`[MCP probe]` became `mcp-probe`, `[Team deathmatch]`
  `team-deathmatch`), with `name`, `nameoriginal`, `description`, `preview` and `layers` /
  `mission` (content) or `path` / `restart` (global). The running mode is `game.gamemode`
  ("" = none).
- **Local multiplayer test:** the mod manager's two-player test runs
  `Command("mods.testplay", 2, mod, layers, gameModeId)`, which starts the host as
  `teardown_modtest.exe -half_screen 1 -mod_test_host "<mod>" "<level>" "<layers>"
  "<mode id>" "<global mod ids, space-separated, in one argument>"` (recorded with Process
  Monitor). `mp_test_start` takes `layers`, `game_mode` and `extra_mods` for those slots, and
  without `layers` it reads the mode's `layers` from its mod's `gamemodes.txt`, as the mod
  manager does: its launch of the probe mode passed `... main.xml mcpprobe
  local-mcp-playground.mcp-probe local-mcp-bridge`, and ours now does the same.
- **Switching while a level runs:** `SetString("game.gamemode.next", "<mod>.<key>")` in client
  code (what the pause menu's Game Modes > Play does) reloaded the level with that mode's
  layers in about 2.5 s (the log shows `Loading: .../main.xml (mcpprobe)`); `""` switched back to
  no mode the same way. **Switching a content mode on or off reloads the level** (its layers
  change); **ending a global mode does not**: the mode's script is unloaded in place (its
  `destroy` runs, see below) and the level keeps running. A bridge `batch` with
  `wait_scene: true` then waits in vain for a new scene: leave `wait_scene` off there.
- `sandbox.lua` switches itself off while any game mode runs, content modes included (it
  takes the mode's `path` as its test, and `""` counts as true in Lua; quirks.md section 4),
  and back on when the mode ends. A level can keep it in its XML.

Measured with the probe mode (2026-11-21, 2.2.0, local multiplayer test):

- Started with `mp_test_start` `game_mode: "local-mcp-playground.MCP probe"` and `layers:
  "mcpprobe"`: the mode was registered, `game.gamemode` held the id as passed, and the probe
  script's `init` ran in the host's server and client halves and on the joined client. The
  engine needs the layers in the launch: with the id alone (before `mp_test_start` looked them
  up) the mode was registered but `game.gamemode` stayed empty and the layer did not load.
- **`layer="-mcpprobe"` works as the example suggests:** a marker box in a `-mcpprobe` group
  was absent while the mode ran and present after it was switched off, on host and joined
  client.
- **Single-player** (mod manager, One Player, "MCP probe"): the same: `game.gamemode` set,
  `init` in both halves, marker absent, sandbox off; `game.levelid` was the mode id
  (`local-mcp-playground.mcp-probe`) instead of `main`. Switching the mode off reloaded the
  level there too (2.8 s; marker back, sandbox on, `game.levelid` `main`).
- **Global modes start too**, with `game_mode: "builtin-mpclassics.deathmatch"` and
  `extra_mods: ["builtin-mpclassics"]`: `game.gamemode` was set on the host and the joined
  client, and Multiplayer Classics stayed active through the level load (2026-11-21, after the
  fix below). **The mode's mod need not be enabled in the player's mod list:** the probe
  global mod (`probemode/` in the teardown-mcp repo, linked as `mcp-probe-mode`, never enabled)
  started with `game_mode: "local-mcp-probe-mode.mcp-global-probe"` and `extra_mods:
  ["local-mcp-probe-mode"]`.
- **The global mods are ONE argument.** Recorded with Process Monitor, the mod manager starts
  the host with `"local-mcp-bridge builtin-mpclassics"` as a single quoted argument. Passed as
  two arguments, the host takes only the first: Multiplayer Classics was dropped when the
  level loaded and Deathmatch never started. The host's log line `Args:` joins the arguments
  with spaces, so both launches look identical there; Process Monitor's "Process Create"
  event shows the real quoting. `mp_test_start` passes them as one argument since.
- **From a level script in the main game, `Command("mods.testplay", 2, mod, layers,
  gameModeId)`** (the mod manager's own call; bridge `run_lua` `side: client` in
  single-player) opened **no** test windows: it restarted the current single-player level with
  the mode (`Command("mods.testplay", 2, "local-mcp-playground", "",
  "builtin-mpclassics.deathmatch")` gave Deathmatch's setup screen with one player,
  `game.gamemode` set, `IsMultiplayer()` false). So a script can start a global mode in
  single-player, and global modes do run there; the player count is ignored from a level.
- Single-player lists game modes like the test does: with Multiplayer Classics enabled,
  `gamemodes` held `builtin-mpclassics` and `local-mcp-playground` in a single-player level.
- `SetString("game.gamemode.next", "")` during that single-player Deathmatch ended the mode
  **without a level reload** (observed once): `game.gamemode` became "", Deathmatch's screens
  closed, the bridge's scene counter and `GetTime()` kept going, and the player kept the gun
  from Deathmatch's loadout. The same holds in the multiplayer test for a global mode
  (below); only content modes reload.
- **`destroy()` did not run when the content mode was switched off** (no line from its
  `destroy` in the host log between the probe's init and the reload). `game.*` registry keys
  the probe wrote were gone after the switch.
- **A global mode's `destroy()` does run when the mode ends** (the probe global mod, multiplayer
  test, `SetString("game.gamemode.next", "")`): `server.destroy` and `client.destroy` both ran
  on the host in the same frame, and the joined client's own `client.destroy` ran; there was
  no level reload, `game.gamemode` became `""` and `level.sandbox` came back on. That is where
  Deathmatch calls mplib's `toolsCleanup()`: a global mode must clean up there, because the
  level and everything the mode spawned stay.

The mpclassics modes are built on mplib (`unofficial-docs.md` section 6): `spawnInit`,
`spawnSetDefaultLoadout`, `utilLoadLevelPlayerSpawns` (locations tagged `playerspawn`, else 64
generated points), loot tiers from `ammospawn` locations, `countdownInit`, `statsTick`,
`hudDrawGameModeHelpText`; `server.tick` calls `spawnTick`, `countdownTick`, `toolsTick`.

## 14. Finding and fixing what costs frame time

**Level assets: VoxTool.** Tuxedo Labs' own VoxTool (teardowngame.com/voxtool, version 0.87,
a separate download with a GUI, documented in a Google Doc linked from that page; read
2026-11-22, not used here yet) turns OBJ/FBX models, heightmaps and images into `.vox`, and
for optimization: splits a model into a target number of shapes that fit its visible voxels
(Optimization: Target Count, Method Model or Terrain), hollows models (Hull), fixes
connectivity, rescales by whole factors, and compresses `.vox` (not readable by MagicaVoxel
afterwards). Its docs give two rules of thumb: lag when something breaks usually means a shape
whose box is too large (the engine processes the empty space too; split it), constant lag
usually too many shapes (keep a scene around 5,000). The first agrees with our Spawn-cost
measurement (quirks.md, "Spawn and prefabs") and with a hole-cost measurement: holes in a
44.5M-cell, 6 % filled voxagon cost 200+ ms frames, holes in a thin shape of a million voxels
nothing (quirks.md, "Destruction and debris"). A voxagon can be cut into tiles along the
10 cm grid without changing its voxels (same section).


From an optimization pass on a co-op mod (Hive Breach, 2026-11-22, 2.2.0, multiplayer test
host, a wave of 20 walking bodies with client-animated legs).

**Measure, per callback.** Scripts cannot time themselves: the sandbox has no `os` table (no
`os.clock`), and `GetTime()` is game time. Use:

1. `perf_sample` for frame times (median, 95th percentile, frames over 33 ms) before and
   after, in the same situation;
2. the engine profiler (bridge.md): its Lua list gives each script's ms per frame; pause,
   click the script, scroll the panel, and read average and peak per callback (Tick, Update,
   Post physics update per half, Render, Draw).

Compare like with like: the same wave, the bodies at the same distance and on screen or not.
Physics time alone moved from 0.6 to 5 ms between two moments of one wave.

**What it found, and what fixed it:**

- `client.render` placing about 300 client-side parts (20 rigs) every frame: 4.2 ms average,
  8.6 ms peak, the biggest single cost. **Level of detail**: rigs outside a cone around the
  view direction are placed every 8th frame, rigs on screen every frame within 15 m, every 2nd
  frame to 30 m, every 3rd beyond; skipped frames add their dt to the next update. With the
  bodies piled around the player (all close and on screen, the worst case) render fell to
  2.2 ms average, 5.2 ms peak. Cost per part placed: about 10 µs (two-bone IK, `QuatLookAt`,
  `SetBodyTransform`). Parts of a rig that comes on screen may be up to 8 frames stale for
  that moment.
- Synced registry writes every tick (status strings for test tools, `SetString(k, v, true)`
  with `string.format`, several keys): the server tick average went from 0.70 to 0.35 ms when
  they were written only on change. A synced write goes to every client; write it when the
  value changes, and round values that would otherwise change every tick.
- Frame times over all: median 14.2 ms to 12.1 ms, 95th percentile 25.6 to 18.8 ms.
- Not fixed: generating a level with many brush edits per tick spikes frames over 33 ms. That
  was acceptable behind a progress screen; elsewhere lower the edits per tick.
- Cheap enough to leave alone (measured): about 60 `DrawSprite` quads a frame for screens
  (1000 cost about 0.2 ms); `FindBodies(tag, true)` per frame for a tag with 20 bodies.

## 15. Mod files, assets and other mods

From Tuxedo Labs' modding guides (`official/modding.md`, `official/multiplayer.md`, read
2026-11-28; pictures in `official/images/`), in short. Marked "measured" where we checked it.

**The mod folder** (`Documents/Teardown/mods/<name>`; use only Latin letters, digits and
spaces in names, or loading may fail). These files go in the root:

| File | What it does |
|---|---|
| `info.txt` | Required. `name`, `author`, `description`, optional `tags` (Workshop categories: Map, Gameplay, Asset, Vehicle, Tool). `version = 2` marks the mod as multiplayer-capable; without it, it is a single-player mod. `preview = <file>` is the picture in the Multiplayer menu: 512 × 292, PNG or JPG, inside the mod folder |
| `main.xml` | Makes it a content mod (a playable level). `MOD/` in paths is the mod root; `LEVEL/` is the folder named like the open XML file, next to it |
| `main.lua` | Runs whenever the mod is active (a global mod: in every game while enabled) |
| `options.lua` | Adds an Options button in the mod manager; the game runs its `draw()` for the options screen (the built-in Speedometer mod is the example). Settings go in `savegame.mod.*` |
| `spawn.txt` | Prefabs for the spawn menu, one per line: `path/to/thing.xml : Category/Name`, path from the mod root. Static bodies become dynamic when spawned from the menu |
| `gamemodes.txt` | Game modes (section 13) |
| `preview.jpg` | Workshop thumbnail, at most 1 MB; publishing also writes `id.txt` (the Workshop item id, used for updates) |

- **Replacing game content:** a file at the same path as under the game's `data/` folder
  (`<mod>/data/vox/pipebomb.vox`) replaces it while the mod plays, or always for an enabled
  global mod. The game's sounds are encrypted `.tde` files; an `.ogg` with the same name
  (without `.tde`) in the same place is loaded instead.
- **More levels:** `StartLevel("id", "MOD/level2.xml", "layer1 layer2")`. Groups get a
  `layer` property; a group with `-name` in it is left out when layer `name` loads.
- **Custom tools:** `RegisterTool(id, name, vox)` once, then enable it per player with
  `SetToolEnabled` (tools start disabled; section 10 has a working tool).
- **Script parameters** (`GetIntParam` etc.) come from the `<script>` element's attributes in
  the level XML; copy them to variables in `init`.

**Multiplayer requirements:** `version = 2` in `info.txt`, and `#version 2` on the first line of
**every** script: a script without it does not run in multiplayer, silently. Game logic goes
on the server, presentation on the client, `shared` is server-to-client only.

**Standard multiplayer level markup**, used by the built-in modes and recommended so levels
work with other people's game modes (a convention, not engine behaviour). Built-in levels keep
these location nodes under a group `multiplayer`:

| Location tag | Used for |
|---|---|
| `playerspawn` | spawn points in modes without teams (Deathmatch) |
| `teamspawn=N` | team N's spawn points (1, 2, ...) |
| `ammospawn rarity=low` / `medium` / `high` | crate spots; the mode decides which crate goes where (built-in colours: low grey, medium blue, high red, tool-mod crates purple) |
| `pointofinterest=N` | where a mode puts team N's objective (a flag base, say) |

**MagicaVoxel models:**

- **The palette index is the material**, not the colour: index 9 is grass whatever colour it
  has, 57 to 72 are wood. `official/images/modding/teardown_palette.png` is the full chart;
  Tuxedo Labs also offer an annotated palette file (`downloadable/teardown_palette.vox` on
  the modding page).
- Object size: at most 256 × 256 × 256 voxels; about 128³ is fine; big flat objects
  (256 × 256 × 10) are fine. Split objects with a lot of empty space (a tree: crown and
  trunk; a house: one object per wall and floor), because the engine walks the empty space
  too (section 14 has the measured cost).
- Overlapping objects: the newer one (top of the scene outline) overwrites the older one's
  voxels on import. Cut and paste an object to make it the newest.
- Voxels hold together only through faces, not edges or corners.
- Do not enclose soft material in hard material (plaster grout between bricks gets stuck
  loose when broken); make the grout from the hard material in a lighter colour.
- Rendering: use the Metal material type (Diffuse is Metal at full roughness). Glass is clear
  for any transparency value other than 100. Emissive needs Power 2 or more to glow in game,
  and glowing voxels light nothing.

**Materials** (softest to hardest; which tools break them):

| Material | Hardness | Sledge | Blowtorch | Guns | Explosives |
|---|---|---|---|---|---|
| glass, grass, plastic, wood | soft | yes | yes | yes | yes |
| dirt, plaster | soft | yes | no | yes | yes |
| concrete, brick | medium | no | no | yes | yes |
| weak metal | medium | no | yes | yes | yes |
| hard masonry, hard metal | hard | no | no | no | yes |
| heavy metal, rock | unbreakable | no | no | no | no |

**Custom skybox.** The modding page's `skybox-dds.zip` holds `cmft.exe` (an open-source cubemap
filter) and `generate.bat`, which turns `sky.hdr` (an equirectangular HDR image) into
`sky.dds`: radiance filter, 256-pixel faces, 20 mip levels (gloss scale 12, bias 3, Blinn
BRDF, base level excluded), output as an `rgba32f` cubemap DDS. Use it with
`<environment skybox="MOD/sky.dds"/>` or `SetEnvironmentProperty("skybox", "MOD/sky.dds")`.

Measured 2026-11-28 (2.2.0): `cmft.exe` with `generate.bat`'s parameters turned the kit's
sample `sky.hdr` into an 8 MB `sky.dds` (256 × 256 faces, 9 mips) in 0.6 s. (Running the .bat
through `cmd /c` from PowerShell failed with "not recognized"; calling `cmft.exe` with the same
arguments worked.) Set at run time from the server with `SetEnvironmentProperty`, the new sky
and its lighting showed at once on the host and the joined client; `MOD/` resolves against
the calling script's own mod on each machine. At 256-pixel faces the sky looks softer than the
built-in `day.dds`; raise `dstFaceSize` for a sharper one (not tried).

**Depending on another mod** (measured 2026-11-28, 2.2.0, multiplayer test, MCP Playground,
host server, host client and joined client alike). The registry lists mods under
`mods.available.<mod id>` (`builtin-<name>`, `steam-<workshop id>`, `local-<folder>`), each
with `name`, `listname`, `author`, `description`, `tags`, `path`, `local`, `playable`,
`gamemode`, `multiplayer`, `active` and more (`unofficial-docs.md` section 4).

- **In a multiplayer session it holds only the session's mods:** the 40 built-in mods, the
  content mod being played and the global mods loaded with it (42 keys here, while 7 more
  local mods and 176 Workshop mods were installed). So `HasKey("mods.available.<id>")` answers
  "can this session use mod X", which is the question that matters.
- `.active` was true for the global mod (the bridge) and **false for the content mod being
  played**; that one is `GetString("game.mod")`.
- `Spawn("<id>:path.xml", t)` for a mod not in the list (installed or not) raises a normal,
  catchable error: `Spawn('<path>') blocked in multiplayer: mod '<id>' is not listed in
  mods.available.` A built-in mod's prefab spawned fine though that mod was not active.
- A missing file in your own mod (`Spawn("MOD/nothing.xml", t)`) raises nothing and returns
  an empty table.

```lua
-- Before using another mod's prefabs: say so instead of failing quietly.
local DEP = "steam-1234567890"
function server.init()
	hasDep = HasKey("mods.available." .. DEP)
	if not hasDep then shared.missingDep = DEP end  -- the client shows a notice
end
```

The game's own spawn menu (`data/script/spawn.lua`) uses the same `HasKey` check.

**Outdated in the guide:** "objects cannot be spawned at run time" (its Possibilities and
Limitations section) predates `Spawn`; ignore it. Its other limit, water height fixed at run
time, is not measured; `SetProperty` lists a water `depth`.

**Reusing the game's assets** (quirks.md "Sounds" and "Spawn and prefabs" have the measurements):

- Sounds: the game's own load by name, `LoadSound("alarm1.ogg")` (from `data/snd/`, list the
  folder for names; the files end in `.ogg.tde`, leave that off). Your own go in the mod:
  `LoadSound("MOD/snd/beep.ogg")`, Ogg Vorbis. A wrong path still returns a handle and only
  logs `Couldn't find sound bank <path>` (`LoadLoop`: `Couldn't find sound loop: <path>`), so
  check the log after adding sounds: `read_log` with `contains: "Couldn't find sound"`. `LoadLoop` for
  looping sounds, played with `PlayLoop` every frame you want to hear them. Play them on the
  client (section 4).
- Prefabs and models: built-in prefabs spawn by their path under `data/`
  (`Spawn("player/player_sven_lagom.xml", t, false, false)`), and the game's `.vox` files load
  the same way (`<vox file="vox/...">`). Another mod's files: `<mod id>:path` in `Spawn`
  (see "Depending on another mod" above).
- Levels and prefabs in the game's `data/` and `mods/` folders are plain XML: the best
  examples of any element. `references/game/xml.md` indexes them.

**What a level or prefab element means:** `references/game/xml.md` (generated from the editor's
help text and the game's own XML) lists every element and attribute the game uses, with the
editor's description and default where it has one.

## 16. Models built by a script (`tools/vox.py`)

When no one is at MagicaVoxel, write the `.vox` from Python. `tools/vox.py` (Teardown-MCP repo,
stdlib only) gives out palette indices by material name, so the voxels break, burn and weigh
as that material (the material is the palette index's range, not its colour: quirks.md
"Materials and .vox files").

```python
import sys; sys.path.insert(0, r"C:\Projects\Teardown-MCP\tools")
import vox
v = vox.Vox()
wood = v.color("wood", (120, 80, 40))              # an index in 57-72
steel = v.color("metal", (90, 90, 100))
lamp = v.color("plaster", (255, 220, 120), emit=1.0)   # glows (MATL _emit)
pane = v.color("glass", (180, 220, 255))           # see-through (MATL _glass added for glass)
crate = v.model("crate", (10, 10, 10))             # voxels, x y z with z up
crate.box((0, 0, 0), (9, 9, 9), wood)              # inclusive corners
crate.box((1, 1, 1), (8, 8, 8), 0)                 # 0 clears: hollow it
crate.box((0, 4, 4), (0, 5, 5), pane)              # a window on one side
v.save(r"C:\path\to\mod\vox\props.vox")            # several models per file are fine
```

```xml
<body dynamic="true"><vox file="MOD/vox/props.vox" object="crate"/></body>
```

- Each `model` becomes a named object, loaded with `object="name"`. **A named object stands
  with its bottom centre on the body origin** (measured), so a crate spawned at floor height
  rests on the floor. `pos` on the `<vox>` moves it from there.
- Axes: `.vox` x, y, z (z up) become Teardown x, y up, -z. 1 voxel = 0.1 m.
- Up to 256 voxels a side, but keep objects compact: split shapes with a lot of empty space
  (section 15, MagicaVoxel models).
- Read a model back with `python -I tools/vox.py file.vox` (models, sizes, voxel counts per
  material). After spawning, `GetShapeMaterialAtPosition` or `GetShapeMaterial` confirms the
  material in the game.
- Hollow models with 1-voxel walls break into large pieces (a hollow wood crate split in
  two from one bullet, skill-test session): use 2-3 voxel walls, or solid models, for props
  that should break gradually.
- A shape the player should not collide with: use `unphysical` (225-240). Indestructible:
  `rock` or `heavymetal`, or the `unbreakable` tag.
- Ranges: glass 8 slots, foliage 16, dirt 16, rock 16, wood 16, masonry 32, plaster 16, metal
  16, heavymetal 16, plastic 16, hardmetal 8, hardmasonry 8, ice 8 colours per file; `color`
  raises an error when a material's range is used up.

## 17. Animated models (`<animator>`, `tools/td_anim.py`)

The engine has a skeletal animation system: an `<animator>` element with a skeleton and clips
from a `.bin` file, one body per bone, and Lua functions to play and blend clips
(`PlayAnimation`, `PlayAnimationLoop`, `PlayAnimationFrame`, `BeginAnimationGroup`,
`SetAnimationClipSpeed`, IK and ragdoll functions: `official/api/animation.md`). Two ways to
make one:

1. **The official route, from a rigged FBX** (Mixamo or Blender): Tuxedo Labs' VoxTool turns
   an FBX, a bone map and a node template into `animdata.bin`, `prefab.xml`, `model.vox`
   and `animation.lua`. Step by step in `official/animation/character-mod.md` (with
   `guide-files/` examples). `official/animation/custom-tools.md` animates a held tool,
   `vehicle-ik-rigging.md` puts a driver in a vehicle, `guide.md` explains the system
   (clips, groups, layers, IK, ragdoll). A mod with `characters.txt` (`[Name]`, `description`,
   `preview`, `path` to the prefab, `anim_speed`, `height`) adds a player character; the
   built-in Merlin mod (`<Teardown>/mods/merlin/`) is a complete example.
2. **By script, for simple rigs** (doors, arms, turrets, creatures made of boxes):
   `tools/td_anim.py` writes the `.bin`, `tools/vox.py` one model per bone. This was built and
   run (2026-11-28, multiplayer test, both windows; the run had `pos` on each `<vox>`, see the
   rules below):

```python
import math, sys; sys.path.insert(0, r"C:\Projects\Teardown-MCP\tools")
import td_anim
I = (0, 0, 0, 1)                      # no rotation (quaternion x y z w)
bones = [("root", -1, (0, 0, 0), I),  # name, parent index, T-pose pos (local to parent), rot
         ("base", 0, (0, 0, 0), I),
         ("seg1", 1, (0, 0.4, 0), I),
         ("seg2", 2, (0, 1.0, 0), I)]
k1, k2 = [], []
for f in range(61):                   # 2 s at 30 fps
    t = f / 30
    k1.append(((0, 0.4, 0), td_anim.quat_axis_angle((0, 0, 1), 40 * math.sin(math.pi * t)), t))
    k2.append(((0, 1.0, 0), td_anim.quat_axis_angle((0, 0, 1), 60 * math.sin(math.pi * t + 1)), t))
td_anim.write("anim/arm.bin", bones, [("wave", 30.0, {2: k1, 3: k2})])  # root, base: T-pose
```

```xml
<group>
	<script file="MOD/script/arm.lua"/>
	<animator name="arm" tags="arm" file="MOD/anim/arm.bin">
		<body name="root" tags="bone=root root nomerge" dynamic="false">
			<body name="base" tags="bone=base base nomerge" dynamic="false">
				<vox tags="unbreakable nocull bone" file="MOD/vox/arm.vox" object="base"/>
				<joint tags="bone" type="ball" size="0.1"/>
				<body name="seg1" tags="bone=seg1 seg1 nomerge" pos="0 0.4 0" dynamic="false">
					<vox tags="unbreakable nocull bone" file="MOD/vox/arm.vox" object="seg1"/>
					<joint tags="bone" type="ball" size="0.1"/>
					<body name="seg2" tags="bone=seg2 seg2 nomerge" pos="0 1.0 0" dynamic="false">
						<vox tags="unbreakable nocull bone" file="MOD/vox/arm.vox" object="seg2"/>
						<joint tags="bone" type="ball" size="0.1"/>
					</body>
				</body>
			</body>
		</body>
	</animator>
</group>
```

```lua
#version 2
function server.init()
	anim = FindAnimator("arm", true)      -- by tag
end
function server.tick(dt)
	if anim ~= 0 then PlayAnimationLoop(anim, "wave") end  -- every tick while it should play
end
```

Rules learned building it (quirks.md "Animators and animation files"):

- **Every clip needs a track for every bone**, or the game crashes when the clip plays.
  `td_anim.write` adds T-pose tracks for bones you leave out.
- Body `pos` = the bone's T-pose position, so the skeleton and the bodies agree. A bone
  model made as a named object stands on its bone's origin (bottom centre), so a segment
  built upward from z = 0 in the `.vox` grows from its joint. (The run above used `pos="0 0.5 0"`
  on the `<vox>`, assuming centred models, and each segment started 0.5 m above its joint:
  leave `pos` off for this layout.)
- `FindAnimator` takes a tag: give the `<animator>` a `tags=`.
- Animation driven on the server replicates to every client (the bone bodies move like any
  server body). The game itself animates player characters on each client instead (local
  models, quirks.md "Client-side entities and animation"); do that for purely visual parts.
- **A clip played once per event** (a lever pull): keep your own clock on the server and call
  `PlayAnimationFrame(anim, clip, t)` every tick, with `t = 0` while idle (the first frame,
  so the rest pose if the clip starts there). Do things at the right moment from `t`, at
  `GetBoneWorldTransform(anim, bone)` (the skill-test session spawned a crate at a hook bone
  this way). Measured 2026-11-28 by that session: it replicated to the client, and the bones
  came back exactly to rest (rotation 0, 0, 0, 1) on host and client after the clip.
  `PlayAnimation` (the engine's own one-shot) was not tried, nor what pose bones take when
  no clip is played at all.
- Clips play while you call `PlayAnimation*` for them; blend several with weights or inside
  `BeginAnimationGroup` / `EndAnimationGroup`. `data/script/player_locomotion.lua` in the game
  is the full example of blending, speeds, fades and IK.
- Not tested: rotations in the T-pose (all were identity here), scaled bone models
  (`scale="0.5"`, as the characters use), whether the `<joint tags="bone">` elements are
  needed when nothing is ragdolled.
