-- Calls state-changing API functions with valid, harmless arguments and reports for each
-- whether this half (server / client) is allowed to. Run on the server first (it spawns the
-- test body the client then tries to touch), then on the client, then on the server again
-- to read back what the client wrote.
--
-- SIDE is set by the caller: "server" or "client".

local out = {}
local function note(name, text) out[#out + 1] = name .. "|" .. text end

local index = 0

-- ONLY (set by the caller) runs just the probe with that number.
local function probe(name, fn)
	index = index + 1
	if ONLY and index ~= ONLY then return end
	SetString("savegame.mod.mcpprobe_at", name)
	local r = { pcall(fn) }
	if not r[1] then
		note(name, "err|" .. tostring(r[2]))
		return
	end
	local parts = {}
	for i = 2, 4 do
		if r[i] ~= nil then parts[#parts + 1] = type(r[i]) == "table" and ("table#" .. #r[i]) or tostring(r[i]) end
	end
	note(name, "ok|" .. table.concat(parts, ","))
end

local FAR = Vec(0, 60, 0) -- above the map: nothing to damage
local P = 0

-- Test body: spawned by the server, found by tag on the client.
local B = FindBody("mcpprobe", true)
if B == 0 and SIDE == "server" then
	local hs = Spawn('<body tags="mcpprobe" dynamic="false"><voxbox size="4 4 4" tags="mcpprobeshape"/></body>', Transform(Vec(0, 40, 0)))
	B = hs[1]
end
local S = FindShape("mcpprobeshape", true)
local V = FindVehicle("", true)
local L = FindLight("", true)
note("handles", table.concat({ B, S, V, L }, ","))

-- What the other half left behind
note("read level.mcpprobe.srv_nosync", GetString("level.mcpprobe.srv_nosync"))
note("read level.mcpprobe.srv_sync", GetString("level.mcpprobe.srv_sync"))
note("read level.mcpprobe.cli", GetString("level.mcpprobe.cli"))
note("read level.mcpprobe.cli_sync", GetString("level.mcpprobe.cli_sync"))
note("read savegame.mod.mcpprobe_srv", GetString("savegame.mod.mcpprobe_srv"))
note("read savegame.mod.mcpprobe_cli", GetString("savegame.mod.mcpprobe_cli"))
note("read shared.mcpprobe", tostring(shared and shared.mcpprobe))
note("read tag on test body", GetTagValue(B, "fromclient") .. "/" .. GetTagValue(B, "fromserver"))
note("read body pos y", string.format("%.1f", GetBodyTransform(B).pos[2]))

-- Registry
if SIDE == "server" then
	probe("SetString level (no sync)", function() SetString("level.mcpprobe.srv_nosync", "s1") end)
	probe("SetString level (sync=true)", function() SetString("level.mcpprobe.srv_sync", "s2", true) end)
	probe("SetString savegame.mod", function() SetString("savegame.mod.mcpprobe_srv", "s3") end)
	probe("shared write", function() shared.mcpprobe = "from-server" end)
else
	probe("SetString level (no sync)", function() SetString("level.mcpprobe.cli", "c1") end)
	probe("SetString level (sync=true)", function() SetString("level.mcpprobe.cli_sync", "c2", true) end)
	probe("SetString savegame.mod", function() SetString("savegame.mod.mcpprobe_cli", "c3") end)
	probe("shared write", function() shared.mcpprobe_c = "from-client" end)
end
probe("SetString game.*", function() SetString("game.mcpprobe", "x") return GetString("game.mcpprobe") end)
probe("ClearKey", function() ClearKey("level.mcpprobe.tmp") end)

-- Entities
probe("SetTag", function() SetTag(B, SIDE == "server" and "fromserver" or "fromclient", "1") end)
probe("SetDescription", function() SetDescription(B, "probe") end)
probe("SetBodyTransform", function()
	local t = GetBodyTransform(B)
	t.pos[2] = t.pos[2] + (SIDE == "server" and 1 or 5)
	SetBodyTransform(B, t)
	return GetBodyTransform(B).pos[2]
end)
probe("SetBodyDynamic", function() SetBodyDynamic(B, false) end)
probe("SetBodyVelocity", function() SetBodyVelocity(B, Vec(0, 0, 0)) end)
probe("SetBodyAngularVelocity", function() SetBodyAngularVelocity(B, Vec(0, 0, 0)) end)
probe("SetBodyActive", function() SetBodyActive(B, false) end)
probe("ApplyBodyImpulse", function() ApplyBodyImpulse(B, GetBodyTransform(B).pos, Vec(0, 0.01, 0)) end)
probe("DrawBodyOutline", function() DrawBodyOutline(B, 1, 1, 1, 1) end)
probe("DrawBodyHighlight", function() DrawBodyHighlight(B, 1) end)
probe("SetShapeLocalTransform", function() SetShapeLocalTransform(S, GetShapeLocalTransform(S)) end)
probe("SetShapeEmissiveScale", function() SetShapeEmissiveScale(S, 1) end)
probe("SetShapeDensity", function() SetShapeDensity(S, 1) end)
probe("SetShapeCollisionFilter", function() SetShapeCollisionFilter(S, 1, 255) end)
probe("DrawShapeOutline", function() DrawShapeOutline(S, 1) end)
probe("DrawShapeHighlight", function() DrawShapeHighlight(S, 1) end)
probe("SetBrush+DrawShapeBox", function() SetBrush("sphere", 1, 1) DrawShapeBox(S, 0, 0, 0, 1, 1, 1) end)
probe("CreateShape", function() return CreateShape(B, Transform(), 0) end)
probe("Spawn (xml string)", function()
	local hs = Spawn('<body dynamic="false" tags="mcpprobe_' .. SIDE .. '"><voxbox size="2 2 2"/></body>', Transform(Vec(3, 40, 0)))
	return #hs, hs[1]
end)
probe("Delete own spawn", function()
	local b = FindBody("mcpprobe_" .. SIDE, true)
	Delete(b)
	return b, IsHandleValid(b)
end)
probe("MakeHole (empty air)", function() return MakeHole(FAR, 0.2, 0.2, 0.2) end)
probe("Paint (empty air)", function() Paint(FAR, 0.3, "spraycan") end)
probe("SpawnFire (empty air)", function() SpawnFire(FAR) end)
probe("Explosion (high above)", function() Explosion(Vec(0, 150, 0), 0.5) end)
probe("Shoot (upwards)", function() Shoot(Vec(0, 100, 0), Vec(0, 1, 0), "bullet") end)
probe("SetLightEnabled", function() SetLightEnabled(L, IsLightActive(L)) end)
probe("SetLightIntensity", function() SetLightIntensity(L, 1) end)
probe("PointLight", function() PointLight(FAR, 1, 1, 1, 1) end)
probe("DriveVehicle", function() DriveVehicle(V, 0, 0, false) end)
probe("SetVehicleHealth", function() SetVehicleHealth(V, 1) end)

-- Players (0 = host on the server, local player on the client; 2 = the joined client)
probe("SetPlayerTransform(self)", function() SetPlayerTransform(GetPlayerTransform(P), P) end)
probe("SetPlayerVelocity(self)", function() SetPlayerVelocity(GetPlayerVelocity(P), P) end)
probe("SetPlayerHealth(self)", function() SetPlayerHealth(1, P) end)
probe("SetPlayerHealth(player 2)", function() SetPlayerHealth(1, 2) end)
probe("SetPlayerTransform(player 2)", function() SetPlayerTransform(GetPlayerTransform(2), 2) end)
probe("SetPlayerWalkingSpeed", function() SetPlayerWalkingSpeed(7, P) end)
probe("SetPlayerTool", function() SetPlayerTool(GetPlayerTool(P), P) end)
probe("SetToolEnabled", function() SetToolEnabled("sledge", true, P) end)
probe("SetToolAmmo", function() SetToolAmmo("gun", 5, P) return GetToolAmmo("gun", P) end)
probe("SetPlayerSpawnTransform", function() SetPlayerSpawnTransform(GetPlayerTransform(P), P) end)
probe("ApplyPlayerDamage(0)", function() ApplyPlayerDamage(P, 0) end)
probe("SetPlayerRegenerationState", function() SetPlayerRegenerationState(true, P) end)
probe("RegisterTool", function() RegisterTool("mcpprobetool", "MCP probe", "MOD/none.vox") end)

-- Sound, particles, camera, environment
probe("LoadSound(builtin)", function() return LoadSound("alarm1.ogg") end)
probe("PlaySound", function() PlaySound(LoadSound("alarm1.ogg"), FAR, 0.01) end)
probe("LoadLoop+PlayLoop", function() PlayLoop(LoadLoop("alarm2-loop.ogg"), FAR, 0.01) end)
probe("SpawnParticle", function() ParticleReset() SpawnParticle(FAR, Vec(0, 1, 0), 0.5) end)
probe("LoadSprite+DrawSprite", function() DrawSprite(LoadSprite("gfx/circle.png"), Transform(FAR), 0.1, 0.1) end)
probe("DrawLine", function() DrawLine(FAR, VecAdd(FAR, Vec(0, 1, 0))) end)
probe("DebugLine", function() DebugLine(FAR, VecAdd(FAR, Vec(0, 1, 0))) end)
probe("DebugCross", function() DebugCross(FAR) end)
probe("DebugWatch", function() DebugWatch("mcpprobe " .. SIDE, GetTime()) end)
probe("SetCameraTransform", function() SetCameraTransform(GetPlayerCameraTransform(P)) end)
probe("SetCameraFov", function() SetCameraFov(90) end)
probe("ShakeCamera", function() ShakeCamera(0.01) end)
probe("SetCameraDof", function() SetCameraDof(10, 0) end)
probe("SetEnvironmentProperty", function() SetEnvironmentProperty("rain", GetEnvironmentProperty("rain")) end)
probe("SetPostProcessingProperty", function() SetPostProcessingProperty("saturation", GetPostProcessingProperty("saturation")) end)
probe("SetTimeScale", function() SetTimeScale(1) end)
probe("SetGravity", function() SetGravity(Vec(0, -10, 0)) end)
probe("AddHeat", function() AddHeat(S, FAR, 0) end)

-- Input, events, calls between the halves
probe("InputDown", function() return InputDown("usetool") end)
probe("InputDown(player 2)", function() return InputDown("usetool", 2) end)
probe("InputValue", function() return InputValue("mousedx") end)
probe("LastInputDevice", function() return LastInputDevice() end)
probe("PostEvent", function() PostEvent("mcpprobe") end)
probe("TriggerEvent", function() TriggerEvent("mcpprobe") end)
probe("ServerCall", function() ServerCall("server.mcpReady", 1) end)
probe("ClientCall", function() ClientCall(0, "client.mcpNothing") end)
probe("PauseMenuButton", function() return PauseMenuButton("MCP probe") end)
probe("SetValue", function() mcpProbeValue = 0 SetValue("mcpProbeValue", 1, "linear", 0.1) end)
probe("GetLocalPlayer", function() return GetLocalPlayer() end)
probe("IsPlayerHost(1),(2)", function() return IsPlayerHost(1), IsPlayerHost(2) end)
probe("GetPlayerName(1),(2)", function() return GetPlayerName(1), GetPlayerName(2) end)

return table.concat(out, "\n")
