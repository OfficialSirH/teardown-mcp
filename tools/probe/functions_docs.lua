-- Checks claims of the Unofficial TD Docs about single functions (run_lua side=server, then
-- side=client, in the MCP Playground; the server run spawns two test bodies at x = -30).
local R = {}

-- Do these exist in 2.2.0?
local ex = {}
for _, n in ipairs({ "InputLastPressedGamepadButton", "GetPlayerRigTransform", "SetPlayerZoom",
		"GetAddedPlayers", "GetRemovedPlayers", "GetVehicleParams", "ParticleOrientation" }) do
	ex[n] = type(_G[n])
end
R.exists = ex
if type(InputLastPressedGamepadButton) == "function" then
	local ok, v = pcall(InputLastPressedGamepadButton)
	R.lastGamepadButton = { ok = ok, value = tostring(v) }
end
if type(GetPlayerRigTransform) == "function" then
	local ok, v = pcall(GetPlayerRigTransform)
	R.playerRigTransform = { ok = ok, value = type(v) == "table" and v.pos and string.format("%.2f %.2f %.2f", v.pos[1], v.pos[2], v.pos[3]) or tostring(v) }
end

-- GetAddedPlayers / GetRemovedPlayers outside a join
local function list(f)
	local ok, v = pcall(f)
	if not ok then return "error: " .. tostring(v) end
	if type(v) ~= "table" then return type(v) .. ":" .. tostring(v) end
	return "table of " .. #v
end
R.added = list(GetAddedPlayers)
R.removed = list(GetRemovedPlayers)

-- GetVehicleParams: keys of the playground buggy
local v = FindVehicle("mybuggy", true)
if v ~= 0 then
	local keys = {}
	for k, x in pairs(GetVehicleParams(v) or {}) do keys[#keys + 1] = k .. "=" .. tostring(x) end
	table.sort(keys)
	R.vehicleParams = keys
end

-- GetBodyClosestPoint and query filters (server only: spawns)
if not GetLocalPlayer or (GetLocalPlayer() == 1 and server) then
	local s = Spawn('<body><voxbox size="10 10 10" material="wood"/></body>', Transform(Vec(-30, 0, 30)), true)
	local d = Spawn('<body dynamic="true"><voxbox size="10 10 10" material="wood"/></body>', Transform(Vec(-30, 0, 35)), true)
	local sb, ss, db = 0, 0, 0
	for _, h in ipairs(s) do if GetEntityType(h) == "body" then sb = h elseif GetEntityType(h) == "shape" then ss = h end end
	for _, h in ipairs(d) do if GetEntityType(h) == "body" then db = h end end
	local o = Vec(-29.5, 3, 30.5)
	local function probe(label, setup, body)
		if setup then setup() end
		local hit, p, n, sh = GetBodyClosestPoint(body, o)
		return label .. ": hit=" .. tostring(hit) .. " shape=" .. tostring(sh)
	end
	R.closest = {
		probe("static, no filter", nil, sb),
		probe("static, QueryRequire('dynamic')", function() QueryRequire("dynamic") end, sb),
		probe("dynamic, QueryRequire('dynamic')", function() QueryRequire("dynamic") end, db),
		probe("static, QueryRejectShape(its shape)", function() QueryRejectShape(ss) end, sb),
		probe("static, QueryRejectBody(it)", function() QueryRejectBody(sb) end, sb),
		probe("static, no filter again", nil, sb),
	}
end
return R
