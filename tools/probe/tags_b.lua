-- Tag probe, step B (run_lua side=server, a few seconds after tags_a.lua). Reads the results
-- and starts the untagged path query (read with tags_c.lua). The interact check runs on the
-- client side: tags_interact.lua.
local R = {}
local K = "level.probetag."

local ub = {}
for _, k in ipairs({ "body", "shape", "none" }) do ub[k] = GetShapeVoxelCount(GetInt(K .. "ub_" .. k)) end
R.unbreakable_now = ub

local cnt = { ["1"] = 0, ["2"] = 0 }
for _, b in ipairs(FindBodies("mcpk", true)) do
	local v = GetTagValue(b, "mcpk")
	cnt[v] = (cnt[v] or 0) + 1
end
R.inherittags_bodies = cnt

local ex = {}
for _, k in ipairs({ "tag", "none" }) do
	local b = GetInt(K .. "expl_" .. k)
	ex[k] = { valid = IsHandleValid(b), broken = IsHandleValid(b) and IsBodyBroken(b) }
end
R.explosive = ex

local s = GetInt(K .. "bomb")
R.bomb = { valid = IsHandleValid(s), voxels = IsHandleValid(s) and GetShapeVoxelCount(s) or -1, tag = IsHandleValid(s) and GetTagValue(s, "bomb") or "", time = GetTime() }

local bnd = {}
for _, k in ipairs({ "tag", "none" }) do
	local b = GetInt(K .. "bnd_" .. k)
	local p = GetBodyTransform(b).pos
	bnd[k] = string.format("%.2f %.2f %.2f", p[1], p[2], p[3])
end
R.bounded_pos = bnd

local vh = {}
for _, k in ipairs({ "tag", "none" }) do
	local v = GetInt(K .. "veh_" .. k)
	if v ~= 0 then vh[k] = GetVehicleHealth(v) end
end
R.vehicle_health_now = vh

R.path_tagged = { state = GetPathState(), length = GetPathLength() }
QueryPath(Vec(-85, 0.5, 30), Vec(-85, 0.5, 40), 100, 1)
return R
