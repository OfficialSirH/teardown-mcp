-- Tag probe, step A (run_lua side=server in the MCP Playground multiplayer test, with
-- playground/script/probeevents.lua spawned first). Spawns test objects 30-100 m away from
-- the spawn point, acts on them, stores handles under level.probetag.* and returns what can be
-- read at once. Step B (tags_b.lua) reads the rest a few seconds later.
local R = {}
local K = "level.probetag."

local function spawn(xml, pos)
	local hs = Spawn(xml, Transform(pos), true)
	local out = { all = hs }
	for _, h in ipairs(hs) do
		local t = GetEntityType(h)
		if not out[t] then out[t] = h end
	end
	return out
end

-- unbreakable: tag on the body, tag on the shape, no tag. Same MakeHole into each.
local ub = {}
for i, v in ipairs({ { "body", '<body tags="unbreakable"><voxbox size="10 10 10" material="wood"/></body>' },
		{ "shape", '<body><voxbox size="10 10 10" material="wood" tags="unbreakable"/></body>' },
		{ "none", '<body><voxbox size="10 10 10" material="wood"/></body>' } }) do
	local p = Vec(30 + i * 3, 0, 40)
	local s = spawn(v[2], p)
	local before = GetShapeVoxelCount(s.shape)
	MakeHole(VecAdd(p, Vec(0.5, 0.5, 0.5)), 0.4, 0.4, 0.4)
	ub[v[1]] = { before = before, after = GetShapeVoxelCount(s.shape) }
	SetInt(K .. "ub_" .. v[1], s.shape)
end
R.unbreakable = ub

-- inherittags: a dynamic post cut through the middle; count tagged bodies in step B.
for _, v in ipairs({ { 1, "inherittags mcpk=1", 45 }, { 2, "mcpk=2", 48 } }) do
	local p = Vec(v[3], 0, 40)
	spawn('<body dynamic="true" tags="' .. v[2] .. '"><voxbox size="4 30 4" material="wood"/></body>', p)
	MakeHole(VecAdd(p, Vec(0.2, 1.5, 0.2)), 0.45, 0.45, 0.45)
end

-- explosive: a dynamic bar broken in the middle (tagged) and an untagged control.
for _, v in ipairs({ { "explosive=1", 55 }, { "", 60 } }) do
	local p = Vec(v[2], 0, 40)
	local s = spawn('<body dynamic="true" tags="' .. v[1] .. '"><voxbox size="20 4 4" material="wood"/></body>', p)
	MakeHole(VecAdd(p, Vec(1.0, 0.2, 0.2)), 0.35, 0.35, 0.35)
	SetInt(K .. "expl_" .. (v[1] == "" and "none" or "tag"), s.body)
end

-- bomb=2 on a static shape.
local b = spawn('<body><voxbox size="4 4 4" material="wood" tags="bomb=2 bombstrength=1"/></body>', Vec(66, 0, 40))
SetInt(K .. "bomb", b.shape)
R.bomb_spawned_at = GetTime()

-- bounded: dynamic boxes pushed out through the boundary (x = 100).
for _, v in ipairs({ { "bounded", 0 }, { "", 6 } }) do
	local s = spawn('<body dynamic="true" tags="' .. v[1] .. '"><voxbox size="5 5 5" material="wood"/></body>', Vec(94, 0.5, v[2]))
	SetBodyVelocity(s.body, Vec(15, 2, 0))
	SetInt(K .. "bnd_" .. (v[1] == "" and "none" or "tag"), s.body)
end

-- nomerge: two touching voxboxes in one body, with and without the tag.
local nm = {}
for _, v in ipairs({ { "nomerge", -40 }, { "", -45 } }) do
	local s = spawn('<body tags="' .. v[1] .. '"><voxbox size="5 5 5" material="wood"/><voxbox pos="0.5 0 0" size="5 5 5" material="wood"/></body>', Vec(v[2], 0, 40))
	nm[v[1] == "" and "none" or "tag"] = #GetBodyShapes(s.body)
end
R.nomerge_shapes = nm

-- customhealth: SetVehicleHealth on a tagged and an untagged minimal vehicle.
local vh = {}
for _, v in ipairs({ { "customhealth", -55 }, { "", -60 } }) do
	local s = spawn('<vehicle tags="' .. v[1] .. '"><body dynamic="true"><voxbox size="10 5 20" material="metal"/></body></vehicle>', Vec(v[2], 0.5, 40))
	local h0 = s.vehicle and GetVehicleHealth(s.vehicle)
	if s.vehicle then SetVehicleHealth(s.vehicle, 0.3) end
	vh[v[1] == "" and "none" or "tag"] = { vehicle = s.vehicle, before = h0, after = s.vehicle and GetVehicleHealth(s.vehicle) }
	if s.vehicle then SetInt(K .. "veh_" .. (v[1] == "" and "none" or "tag"), s.vehicle) end
end
R.customhealth = vh

-- nonav: a 6 m wide, 3 m high wall across a 10 m path; tagged lane first (step B runs the
-- untagged lane). Walls stand on the floor at z = 35.
spawn('<body><voxbox pos="-3 0 0" size="60 30 2" tags="nonav"/></body>', Vec(-70, 0, 35))
spawn('<body><voxbox pos="-3 0 0" size="60 30 2"/></body>', Vec(-85, 0, 35))
local id = QueryPath(Vec(-70, 0.5, 30), Vec(-70, 0.5, 40), 100, 1)
R.querypath_returns = tostring(id)

-- interact: put the host in front of a tagged box (read in step B, client side).
local box = spawn('<body><voxbox size="5 5 5" material="wood" tags="interact=Probe"/></body>', Vec(80, 1.3, 28.3))
SetInt(K .. "interact", box.shape)
SetPlayerTransformWithPitch(Transform(Vec(80.25, 0, 30), QuatEuler(0, 0, 0)), 1)

return R
