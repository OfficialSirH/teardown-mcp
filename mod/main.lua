#version 2
#include "mcp_config.lua"

-- MCP Bridge
--
-- Commands in : the file named by MCP_INBOX (mcp_config.lua), kept next to the savegame, polled with loadfile
--               (the game never re-reads savegame.xml, so commands cannot come through it).
-- Results out : savegame.mod.mcp_* registry keys, which the game writes to savegame.xml within
--               about half a second; the server reads them from that file.
--
-- Keys (all under savegame.mod.):
--   mcp_session   session id of the server whose inbox was last accepted
--   mcp_ack       id of the last executed command
--   mcp_r<id>_n   number of result chunks of command <id>
--   mcp_r<id>_<i> chunk i of the percent-encoded JSON result
--   mcp_waiting   1 from a scene-reloading command until the new scene runs
--   mcp_halt      1 while a scene load is pending (no command runs in the dying scene)
--   mcp_scene     counts scenes started; the server waits for it to grow
--   mcp_level     level path of the running scene
--   mcp_log       1 if the server also wants results in the game's log file
--   mcp_paused    1 while the single-player pause menu is open (polling then runs from
--                 client.render, client commands only)

local P = "savegame.mod.mcp_"
local INBOX = MCP_INBOX
local CHUNK = 4000
local MAXLEN = 240000
local POLL = 0.1

---------------------------------------------------------------------------
-- JSON out
---------------------------------------------------------------------------

local function jstr(s)
	s = string.gsub(s, '[%c"\\]', function(c)
		if c == '"' then return '\\"' end
		if c == "\\" then return "\\\\" end
		if c == "\n" then return "\\n" end
		if c == "\t" then return "\\t" end
		return string.format("\\u%04x", string.byte(c))
	end)
	return '"' .. s .. '"'
end

local function enc(v, depth, seen)
	local t = type(v)
	if t == "nil" then return "null" end
	if t == "boolean" then return v and "true" or "false" end
	if t == "number" then
		if v ~= v or v == math.huge or v == -math.huge then return "null" end
		if v == math.floor(v) and math.abs(v) < 1e15 then return string.format("%d", v) end
		return string.format("%.9g", v)
	end
	if t == "string" then return jstr(v) end
	if t ~= "table" then return jstr(tostring(v)) end
	if seen[v] then return '"<cycle>"' end
	if depth > 12 then return '"<too deep>"' end
	seen[v] = true
	local n, count = #v, 0
	for _ in pairs(v) do count = count + 1 end
	local parts = {}
	if count == n then
		for i = 1, n do parts[i] = enc(v[i], depth + 1, seen) end
		seen[v] = nil
		return "[" .. table.concat(parts, ",") .. "]"
	end
	for k, x in pairs(v) do
		parts[#parts + 1] = jstr(tostring(k)) .. ":" .. enc(x, depth + 1, seen)
	end
	seen[v] = nil
	return "{" .. table.concat(parts, ",") .. "}"
end

-- Keep the stored value free of anything XML could mangle.
local function pct(s)
	return (string.gsub(s, "[^%w %.,:;{}%[%]_=+*/()!@#$^|~?-]", function(c)
		return string.format("%%%02X", string.byte(c))
	end))
end

local function writeResult(id, tbl)
	local s = pct(enc(tbl, 0, {}))
	if #s > MAXLEN then
		s = pct(enc({ ok = false, error = "result too large (" .. #s .. " chars), narrow the query" }, 0, {}))
	end
	local n = math.max(1, math.ceil(#s / CHUNK))
	for i = 1, n do
		SetString(P .. "r" .. id .. "_" .. i, string.sub(s, (i - 1) * CHUNK + 1, i * CHUNK))
	end
	SetInt(P .. "r" .. id .. "_n", n)
	SetInt(P .. "ack", id)
end

-- Second way out, for the multiplayer test windows (they never write savegame.xml):
-- a failed LoadSound puts its path in the game's log file, so the path carries the data.
-- The log is written in 4 KB blocks; padding lines push the real ones out.
--
-- Only the client half does this. A sound loaded by the server half is replicated to every
-- joined client (each one then logs the same failure and keeps the sound for the whole
-- scene), so the server half only writes its results to the registry and the client half of
-- the same game relays them.
local padCount = 0

local function logLine(name)
	pcall(LoadSound, "MOD/" .. name .. ".ogg")
end

local function hex(s)
	return (string.gsub(s, ".", function(c) return string.format("%02x", string.byte(c)) end))
end

local function relayToLog()
	if GetInt(P .. "log") ~= 1 then return end
	local wrote = false
	if GetInt(P .. "announce") == 1 then
		SetInt(P .. "announce", 0)
		logLine("mcps_" .. GetString(P .. "session") .. "_" .. GetInt(P .. "ack") .. "_"
			.. hex(GetString("game.levelpath")) .. "_" .. GetInt(P .. "scene"))
		wrote = true
	end
	local ack, relayed = GetInt(P .. "ack"), GetInt(P .. "relayed")
	if relayed > ack then relayed = 0 end -- a new session started the ids over
	for id = relayed + 1, ack do
		local n = GetInt(P .. "r" .. id .. "_n")
		if n > 0 then
			local parts = {}
			for i = 1, n do parts[i] = GetString(P .. "r" .. id .. "_" .. i) end
			local json = string.gsub(table.concat(parts), "%%(%x%x)", function(h) return string.char(tonumber(h, 16)) end)
			local h = hex(json)
			local m = math.max(1, math.ceil(#h / 400))
			local head = "mcpr_" .. GetString(P .. "session") .. "_" .. id .. "_"
			for i = 1, m do
				logLine(head .. i .. "_" .. m .. "_" .. string.sub(h, (i - 1) * 400 + 1, i * 400))
			end
			wrote = true
		end
	end
	if ack ~= relayed then SetInt(P .. "relayed", ack) end
	if wrote then
		local pad = string.rep("p", 200)
		local tag = "mcpp_" .. GetInt(P .. "scene") .. "_" .. math.random(100000, 999999) .. "_"
		for _ = 1, 16 do
			padCount = padCount + 1
			logLine(tag .. padCount .. "_" .. pad)
		end
	end
end

local function clearResults(upTo)
	for _, k in ipairs(ListKeys("savegame.mod") or {}) do
		local id = string.match(k, "^mcp_r(%d+)_")
		if id and (upTo == nil or tonumber(id) <= upTo) then ClearKey("savegame.mod." .. k) end
	end
end

---------------------------------------------------------------------------
-- Actions
---------------------------------------------------------------------------

local function vec(a) return Vec(a[1] or 0, a[2] or 0, a[3] or 0) end

local function quat(a)
	if not a then return Quat() end
	if #a == 3 then return QuatEuler(a[1], a[2], a[3]) end
	return Quat(a[1], a[2], a[3], a[4])
end

local function try(fn, ...)
	local ok, a, b, c, d = pcall(fn, ...)
	if ok then return a, b, c, d end
	return nil
end

local function entityPos(h, t)
	local tr
	if t == "body" then tr = try(GetBodyTransform, h)
	elseif t == "shape" then tr = try(GetShapeWorldTransform, h)
	elseif t == "vehicle" then tr = try(GetVehicleTransform, h)
	elseif t == "location" then tr = try(GetLocationTransform, h)
	elseif t == "trigger" then tr = try(GetTriggerTransform, h)
	elseif t == "light" then tr = try(GetLightTransform, h)
	end
	return tr
end

local function describe(h)
	local d = { handle = h, valid = IsHandleValid(h) }
	if not d.valid then return d end
	local t = GetEntityType(h)
	d.type = t
	d.description = try(GetDescription, h)
	local tags = {}
	for _, k in ipairs(try(ListTags, h) or {}) do tags[k] = GetTagValue(h, k) end
	if next(tags) then d.tags = tags end
	d.transform = entityPos(h, t)
	if t == "body" then
		d.dynamic = try(IsBodyDynamic, h)
		d.mass = try(GetBodyMass, h)
		d.velocity = try(GetBodyVelocity, h)
		d.shapes = try(GetBodyShapes, h)
		d.broken = try(IsBodyBroken, h)
	elseif t == "shape" then
		d.body = try(GetShapeBody, h)
		local x, y, z, scale = try(GetShapeSize, h)
		d.size = { x, y, z }
		d.voxel_scale = scale
		d.voxel_count = try(GetShapeVoxelCount, h)
	elseif t == "vehicle" then
		d.body = try(GetVehicleBody, h)
		d.health = try(GetVehicleHealth, h)
	end
	return d
end

local function regNode(key, depth, budget)
	local node = { value = GetString(key) }
	local kids = ListKeys(key) or {}
	if #kids == 0 then return node end
	if depth <= 0 then
		node.child_keys = kids
		return node
	end
	node.children = {}
	for _, k in ipairs(kids) do
		budget.n = budget.n - 1
		if budget.n < 0 then
			node.truncated = true
			break
		end
		node.children[k] = regNode(key .. "." .. k, depth - 1, budget)
	end
	return node
end

local A = {}

-- Players whose own game has this script running (each client reports once, see client.tick).
-- Only the server half fills this.
local readyPlayers = {}

local function playerList(side)
	local list = {}
	for i, p in ipairs(GetAllPlayers() or {}) do
		list[i] = { id = p, name = try(GetPlayerName, p), host = try(IsPlayerHost, p) }
		if side == "server" then list[i].ready = readyPlayers[p] == true end
		if side ~= "server" then list[i].me = (p == GetLocalPlayer()) end
	end
	return list
end

function A.ping(a, side)
	return {
		side = side,
		players = playerList(side),
		version = GetVersion(),
		level_path = GetString("game.levelpath"),
		level_id = GetString("game.levelid"),
		time = GetTime(),
		multiplayer = try(IsMultiplayer),
		scene = GetInt(P .. "scene"),
		-- "PLAY" in a level. game.paused is true while a pause menu is open, also in the
		-- multiplayer test (where nothing stops); paused_sp: the single-player pause, where
		-- only side=client commands run.
		state = GetString("game.state"),
		paused = GetBool("game.paused"),
		paused_sp = GetInt(P .. "paused") == 1,
	}
end

function A.run_lua(a)
	local f, err
	if a.file then
		f, err = loadfile(a.file)
	else
		f, err = loadstring("return " .. a.code, "=mcp")
		if not f then f, err = loadstring(a.code, "=mcp") end
	end
	if not f then error("syntax: " .. tostring(err), 0) end
	local out = {}
	local oldPrint = print
	print = function(...)
		local p = {}
		for i = 1, select("#", ...) do p[i] = tostring((select(i, ...))) end
		out[#out + 1] = table.concat(p, "\t")
	end
	local r = { pcall(f) }
	print = oldPrint
	local res = { output = (#out > 0) and table.concat(out, "\n") or nil }
	if not r[1] then
		res.error = tostring(r[2])
		res.failed = true
		return res
	end
	local n = 0
	for k in pairs(r) do if k > n then n = k end end
	res.returns = {}
	for i = 2, n do res.returns[i - 1] = (r[i] == nil) and "<nil>" or r[i] end
	return res
end

function A.call(a)
	local f = _G
	for part in string.gmatch(a.fn, "[^%.]+") do
		if type(f) ~= "table" then f = nil break end
		f = f[part]
	end
	if type(f) ~= "function" then error("no such function: " .. tostring(a.fn), 0) end
	local args = a.args or {}
	local r = { f(unpack(args, 1, a.argc or #args)) }
	local n = 0
	for k in pairs(r) do if k > n then n = k end end
	for i = 1, n do if r[i] == nil then r[i] = "<nil>" end end
	return r
end

function A.restart() Restart() end

function A.start_level(a)
	StartLevel(a.mission or "", a.path, a.layers or "")
end

function A.game_command(a)
	local args = a.args or {}
	Command(a.command, unpack(args))
end

function A.registry_get(a)
	local node = regNode(a.key, a.depth or 1, { n = a.max_keys or 500 })
	node.exists = HasKey(a.key)
	return node
end

function A.registry_set(a)
	local t, v, sync = a.type or "string", a.value, a.sync or false
	if t == "int" then SetInt(a.key, tonumber(v), sync)
	elseif t == "float" then SetFloat(a.key, tonumber(v), sync)
	elseif t == "bool" then SetBool(a.key, v == true or v == "true" or v == 1, sync)
	else SetString(a.key, tostring(v), sync) end
	return GetString(a.key)
end

function A.registry_clear(a)
	ClearKey(a.key)
end

-- game.player.*: the engine's view of this game's local player (can grab / interact / use a
-- tool, grabbing, input disabled, character, current tool and zoom...). Values as strings.
local function localPlayerState()
	local s = {}
	for _, k in ipairs(ListKeys("game.player") or {}) do
		s[k] = GetString("game.player." .. k)
	end
	return s
end

function A.player_get(a)
	local p = a.player or 0
	local isLocal = p == 0 or p == try(GetLocalPlayer)
	return {
		local_state = isLocal and localPlayerState() or nil,
		player = p,
		name = try(GetPlayerName, p),
		transform = GetPlayerTransform(p),
		camera = try(GetPlayerCameraTransform, p),
		velocity = try(GetPlayerVelocity, p),
		health = try(GetPlayerHealth, p),
		vehicle = try(GetPlayerVehicle, p),
		tool = try(GetPlayerTool, p),
		all_players = try(GetAllPlayers),
	}
end

function A.player_set(a)
	local p = a.player or 0
	if a.pos or a.rot then
		local t = GetPlayerTransform(p)
		if a.pos then t.pos = vec(a.pos) end
		if a.rot then t.rot = quat(a.rot) end
		SetPlayerTransform(t, p)
	end
	if a.velocity then SetPlayerVelocity(vec(a.velocity), p) end
	if a.health then SetPlayerHealth(a.health, p) end
	return { transform = GetPlayerTransform(p), health = try(GetPlayerHealth, p) }
end

function A.spawn(a)
	local t = Transform(vec(a.pos or {}), quat(a.rot))
	local hs = Spawn(a.xml, t, a.allow_static or false, a.joint_existing or false) or {}
	local res = {}
	for i, h in ipairs(hs) do res[i] = { handle = h, type = try(GetEntityType, h) } end
	return res
end

function A.delete(a)
	local n = 0
	for _, h in ipairs(a.handles or {}) do
		if IsHandleValid(h) then
			Delete(h)
			n = n + 1
		end
	end
	return { deleted = n }
end

function A.find(a)
	local all = FindEntities(a.tag or "", true, a.type) or {}
	local limit = a.limit or 100
	local res = { count = #all, entities = {} }
	for i = 1, math.min(#all, limit) do
		local h = all[i]
		local t = GetEntityType(h)
		local tr = entityPos(h, t)
		local desc = try(GetDescription, h)
		res.entities[i] = { handle = h, type = t, pos = tr and tr.pos or nil, description = (desc ~= "") and desc or nil }
	end
	return res
end

function A.describe(a)
	return describe(a.handle)
end

function A.raycast(a)
	local origin, dir
	if a.origin and a.dir then
		origin, dir = vec(a.origin), VecNormalize(vec(a.dir))
	else
		local cam = GetPlayerCameraTransform(a.player or 0)
		origin, dir = cam.pos, TransformToParentVec(cam, Vec(0, 0, -1))
	end
	local hit, dist, normal, shape = QueryRaycast(origin, dir, a.max_dist or 100, a.radius or 0)
	local res = { hit = hit, origin = origin, dir = dir }
	if hit then
		res.dist = dist
		res.point = VecAdd(origin, VecScale(dir, dist))
		res.normal = normal
		res.shape = shape
		res.body = try(GetShapeBody, shape)
	end
	return res
end

-- Put a player where they can see something: at `target` (a position or an entity handle) plus
-- `from` (default 5 m to the side, 2 m up), facing it. Server side.
function A.look_at(a)
	local t = a.target
	if type(t) == "number" then
		local tr = entityPos(t, GetEntityType(t))
		if not tr then error("cannot take a position from entity " .. tostring(t), 0) end
		t = tr.pos
	else
		t = vec(t or {})
	end
	local p = a.player or 0
	local feet = VecAdd(t, a.from and vec(a.from) or Vec(-5, 2, 5))
	SetPlayerVelocity(Vec(0, 0, 0), p)
	SetPlayerTransformWithPitch(Transform(feet, QuatLookAt(VecAdd(feet, Vec(0, 1.7, 0)), t)), p)
	return { feet = feet, target = t }
end

function A.debug_print(a)
	DebugPrint(tostring(a.text))
end

-- Actions that tear the scene down: acknowledged before they run.
-- Frame times for perf_sample: client.tick runs once per rendered frame, dt = that frame's time.
local FRING = 4096
local fN, fDt = 0, {}

function A.perf(a, side)
	if side == "server" or side == "draw" then error("perf runs on side client (or remote)") end
	local r = { frame = fN, time = GetTime() }
	if a.since then
		local n = math.min(fN - a.since, FRING)
		local v, sum = {}, 0
		for i = fN - n + 1, fN do
			local x = fDt[i % FRING] or 0
			v[#v + 1] = x
			sum = sum + x
		end
		table.sort(v)
		local function q(p) return v[math.max(1, math.ceil(#v * p))] * 1000 end
		r.frames = #v
		if #v > 0 then
			local over33, over50 = 0, 0
			for _, x in ipairs(v) do
				if x > 1 / 30 then over33 = over33 + 1 end
				if x > 0.05 then over50 = over50 + 1 end
			end
			r.frame_time_sum_s = sum
			r.dt_avg_ms = sum / #v * 1000
			r.dt_min_ms = v[1] * 1000
			r.dt_p50_ms = q(0.5)
			r.dt_p95_ms = q(0.95)
			r.dt_p99_ms = q(0.99)
			r.dt_max_ms = v[#v] * 1000
			r.frames_over_33ms = over33
			r.frames_over_50ms = over50
		end
	end
	return r
end

function A.scene_stats()
	local bodies = FindBodies("", true)
	local active, dynamic, broken = 0, 0, 0
	for i = 1, #bodies do
		local b = bodies[i]
		if IsBodyActive(b) then active = active + 1 end
		if IsBodyDynamic(b) then dynamic = dynamic + 1 end
		if IsBodyBroken(b) then broken = broken + 1 end
	end
	return {
		bodies = #bodies, active_bodies = active, dynamic_bodies = dynamic, broken_bodies = broken,
		shapes = #FindShapes("", true), vehicles = #FindVehicles("", true), lights = #FindLights("", true),
		joints = #FindJoints("", true), triggers = #FindTriggers("", true),
		registry_level_keys = #ListKeys("level"),
	}
end

local RELOADS = { restart = true, start_level = true }

---------------------------------------------------------------------------
-- Wrong-side guard
---------------------------------------------------------------------------

-- Calling one of these from the wrong half does not raise a Lua error: the engine logs
-- "<fn> is not available in <side> script" and stops that half of the script for the rest of
-- the scene once the callback returns. That would silently kill the bridge, so the bridge
-- replaces them in its own Lua state with functions that raise a normal error instead.
-- Lists measured in Teardown 2.2.0 (tools/probe/allcalls.lua).
local CLIENT_ONLY = { -- "not available in server script"
	"AddMapMarker", "AttachCameraTo", "CopyAnimatorState", "CreateHaptic", "DisableMotionBlur",
	"GetCameraTransform", "HapticIsPlaying", "LoadHaptic", "PlayHaptic", "PlayHapticDirectional",
	"PlaySoundForUser", "RequestFirstPerson", "RequestThirdPerson", "SaveCameraOverrideTransform",
	"SelectedMapMarker", "SetCameraDof", "SetCameraFov", "SetCameraOffsetTransform",
	"SetCameraTransform", "SetLowHealthBlurThreshold", "SetPivotClipBody",
	"SetPlayerCameraOffsetTransform", "SetPlayerRigTags", "SetSoundLoopUser", "SetToolAllowedZoom",
	"SetToolHandPoseLocalTransform", "SetToolHaptic", "SetToolOffset", "SetToolTransform",
	"SetToolTransformOverride", "ShakeCamera", "StopHaptic",
}
local SERVER_ONLY = { -- "not available in client script"
	"AddHeat", "AddSnow", "ApplyPlayerDamage", "DisablePlayerDamage", "DriveVehicle", "Explosion",
	"MakeHole", "Paint", "PaintRGBA", "RegisterTool", "ReleasePlayerGrab", "RemoveAabbFires",
	"SetEnvironmentProperty", "SetGravity", "SetPlayerCrouchSpeedScale", "SetPlayerGroundVelocity",
	"SetPlayerHealth", "SetPlayerHurtSpeedScale", "SetPlayerParam", "SetPlayerRegenerationState",
	"SetPlayerScreen", "SetPlayerSpawnHealth", "SetPlayerSpawnTool", "SetPlayerSpawnTransform",
	"SetPlayerTool", "SetPlayerTransform", "SetPlayerTransformWithPitch", "SetPlayerVehicle",
	"SetPlayerVelocity", "SetPlayerWalkingSpeed", "SetTimeScale", "SetToolAmmo",
	"SetToolAmmoPickupAmount", "SetToolEnabled", "SetVehicleEngineHealth", "SetVehicleHealth",
	"SetVehicleParam", "Shoot", "SpawnFire",
}

local guarded = false

local function installGuards(side)
	if guarded then return end
	guarded = true
	local half = (side == "server") and "server" or "client"
	for _, name in ipairs(half == "server" and CLIENT_ONLY or SERVER_ONLY) do
		_G[name] = function()
			error(name .. " is not available in " .. half .. " script. The bridge blocked the call: the engine would not raise an error, it would stop the " .. half .. " half of this script. Run it with side=" .. (half == "server" and "client" or "server") .. ".", 2)
		end
	end
end

---------------------------------------------------------------------------
-- Pump
---------------------------------------------------------------------------

local lastDone = -1
local pendingReload = nil

local PAUSED_MSG = "the game is paused (single-player pause menu): only side=client commands run there, from client.render. Resume first with run_lua side=client \"SetPaused(false)\" (or send_input: a click on Continue), or run this with side=client."

-- side: "server" / "client" (the host's two halves), "draw" (the host's client.draw, the only
-- place Ui* functions exist), "remote" (a joined client's game; local multiplayer test only,
-- where it can read the same command file and answers through its own log file).
-- paused: called from client.render while the single-player pause menu stops every other
-- callback; commands for the other sides are answered with an error instead of waiting.
local function pump(side, paused)
	if not INBOX then return end -- no path in mcp_config.lua
	installGuards(side)
	local f = loadfile(INBOX)
	if not f then return end
	local ok, box = pcall(f)
	if not ok or type(box) ~= "table" or type(box.session) ~= "string" then return end

	if box.session ~= GetString(P .. "session") then
		clearResults(nil)
		SetInt(P .. "ack", 0)
		SetInt(P .. "started", 0)
		SetInt(P .. "relayed", 0)
		SetInt(P .. "halt", 0)
		SetInt(P .. "waiting", 0)
		SetString(P .. "session", box.session)
	end
	SetInt(P .. "log", box.log and 1 or 0)
	if pendingReload then
		pendingReload.polls = pendingReload.polls - 1
		if pendingReload.polls <= 0 then
			local pr = pendingReload
			pendingReload = nil
			local good, err = pcall(pr.h, pr.args, side)
			if not good then
				SetInt(P .. "halt", 0)
				SetInt(P .. "waiting", 0)
				writeResult(pr.id, { ok = false, error = tostring(err) })
				SetInt(P .. "relayed", pr.id - 1)
			end
		end
		return
	end
	if GetInt(P .. "halt") == 1 then return end

	local done = box.done or 0
	if done ~= lastDone then
		lastDone = done
		clearResults(done)
	end

	local ack = GetInt(P .. "ack")
	for _, cmd in ipairs(box.cmds or {}) do
		local cside = cmd.side or "server"
		if cmd.id <= ack then
			-- done already
		elseif (side == "remote") ~= (cside == "remote") then
			-- Another game's command. A joined client just looks past it; the host steps
			-- over it so its own later commands are not held up.
			if side ~= "remote" then
				ack = cmd.id
				SetInt(P .. "ack", ack)
			end
		elseif cside ~= side then
			if not paused then return end -- the host's other half goes first
			writeResult(cmd.id, { ok = false, error = PAUSED_MSG })
			ack = cmd.id
		else
			local h = A[cmd.action]
			local args = cmd.args or {}
			if not h then
				writeResult(cmd.id, { ok = false, error = "unknown action: " .. tostring(cmd.action) })
			elseif GetInt(P .. "started") == cmd.id then
				-- It was started on an earlier poll and never came back: the engine ended the
				-- callback in a way pcall cannot catch. Running it again would loop forever.
				writeResult(cmd.id, { ok = false, error = "the command ended the script callback without returning (an engine error that pcall cannot catch); it was not run again" })
			elseif RELOADS[cmd.action] or cmd.wait_scene then
				SetInt(P .. "halt", 1)
				if cmd.wait_scene then SetInt(P .. "waiting", 1) end
				writeResult(cmd.id, { ok = true, result = "scene load requested" })
				pendingReload = { h = h, args = args, id = cmd.id, polls = 3 }
				return
			else
				SetInt(P .. "started", cmd.id)
				local r = { pcall(h, args, side) }
				if r[1] then
					writeResult(cmd.id, { ok = true, result = r[2] })
				else
					writeResult(cmd.id, { ok = false, error = tostring(r[2]) })
				end
			end
			ack = cmd.id
		end
	end
end

---------------------------------------------------------------------------
-- Callbacks
---------------------------------------------------------------------------

local sTicks, sTimer, cTimer = 0, 0, 0

function server.tick(dt)
	sTicks = sTicks + 1
	if sTicks < 3 then return end
	if sTicks == 3 then
		-- The scene is loaded and running: tell the server.
		SetString(P .. "level", GetString("game.levelpath"))
		SetInt(P .. "scene", GetInt(P .. "scene") + 1)
		SetInt(P .. "halt", 0)
		SetInt(P .. "waiting", 0)
		SetInt(P .. "announce", 1)
	end
	sTimer = sTimer + dt
	if sTimer < POLL then return end
	sTimer = 0
	pump("server")
end

-- A client's game calls this once its scripts run: the player is really in the level.
function server.mcpReady(p)
	readyPlayers[p] = true
end

local reported = false
-- pausedFlag starts true so the first tick clears a stale mcp_paused loaded from the savegame
-- (a game quit from the pause menu saves it as 1).
local cTicks, ticksAtRender, idleRenders, rTimer, pausedFlag = 0, -1, 0, 0, true

function client.tick(dt)
	cTicks = cTicks + 1
	if pausedFlag then
		pausedFlag = false
		SetInt(P .. "paused", 0)
	end
	fN = fN + 1
	fDt[fN % FRING] = dt
	if not reported then
		reported = true
		ServerCall("server.mcpReady", GetLocalPlayer())
	end
	cTimer = cTimer + dt
	if cTimer < POLL then return end
	cTimer = 0
	-- The host's game runs "client" commands; a joined client's game only "remote" ones.
	pump(IsPlayerHost(GetLocalPlayer()) and "client" or "remote")
	relayToLog()
end

-- The single-player pause menu stops tick, update and draw, but client.render keeps running
-- (measured, quirks.md section 11). Poll from here only once renders keep coming with no tick
-- in between: during normal play and in the multiplayer test (which never pauses) every frame
-- has a tick, so this never fires there and the polling above is unchanged.
local PAUSE_RENDERS = 30

function client.render(dt)
	if cTicks ~= ticksAtRender then
		ticksAtRender = cTicks
		idleRenders = 0
		return
	end
	idleRenders = idleRenders + 1
	if idleRenders < PAUSE_RENDERS then return end
	if not pausedFlag then
		pausedFlag = true
		SetInt(P .. "paused", 1)
	end
	rTimer = rTimer + math.max(dt or 0, 1 / 240)
	if rTimer < POLL then return end
	rTimer = 0
	if IsPlayerHost(GetLocalPlayer()) then pump("client", true) end
end

local dFrames = 0

function client.draw()
	dFrames = dFrames + 1
	if dFrames % 6 ~= 0 then return end
	if not IsPlayerHost(GetLocalPlayer()) then return end
	pump("draw")
end
