-- Tag probe, round 2, step A (run_lua side=server; single-player or multiplayer host).
-- Spawn playground/script/probeevents.lua and probedrive.lua first. Puts everything at
-- z = 60..80, x = -60..60, away from the spawn point; handles go under level.probetag2.*.
-- Step B (tags2_b.lua) reads the results 4-5 s later.
local K = "level.probetag2."
local R = {}

local function spawn(xml, pos, rot)
	local hs = Spawn(xml, Transform(pos, rot or QuatEuler(0, 0, 0)), true)
	local out = {}
	for _, h in ipairs(hs) do
		local t = GetEntityType(h)
		if not out[t] then out[t] = h end
	end
	return out
end

-- A small metal car (indestructible voxboxes would hide brokenthreshold).
local function car(tags)
	return '<vehicle tags="' .. tags .. '" topspeed="60" acceleration="4" strength="4" spring="0.5" damping="0.7">'
		.. '<body pos="0 0.6 0" dynamic="true">'
		.. '<voxbox pos="-0.8 0 -1.5" size="16 5 30" material="metal" color="0.6 0.6 0.6"/>'
		.. '<location tags="player" pos="0 0.9 0"/>'
		.. '<wheel pos="-0.95 -0.25 -1.1" steer="1" travel="-0.2 0.1"><voxbox pos="-0.1 -0.35 -0.35" size="2 7 7"/></wheel>'
		.. '<wheel pos="0.95 -0.25 -1.1" steer="1" travel="-0.2 0.1"><voxbox pos="-0.1 -0.35 -0.35" size="2 7 7"/></wheel>'
		.. '<wheel pos="-0.95 -0.25 1.1" drive="1" travel="-0.2 0.1"><voxbox pos="-0.1 -0.35 -0.35" size="2 7 7"/></wheel>'
		.. '<wheel pos="0.95 -0.25 1.1" drive="1" travel="-0.2 0.1"><voxbox pos="-0.1 -0.35 -0.35" size="2 7 7"/></wheel>'
		.. '</body></vehicle>'
end

-- passive: driven next to an untagged control (both tagged mcpdrive), facing -z.
for _, v in ipairs({ { "ctl", "mcpdrive", -40 }, { "passive", "mcpdrive passive", -30 } }) do
	local s = spawn(car(v[2]), Vec(v[3], 0, 80))
	SetInt(K .. "drive_" .. v[1], s.vehicle or 0)
end

-- noroll: both set spinning about their forward axis.
for _, v in ipairs({ { "ctl", "", -15 }, { "noroll", "noroll", -5 } }) do
	local s = spawn(car(v[2]), Vec(v[3], 0, 70))
	SetInt(K .. "roll_" .. v[1], s.vehicle or 0)
	if s.body then
		SetBodyActive(s.body, true)
		SetBodyAngularVelocity(s.body, Vec(0, 0, 9))
	end
end

-- brokenthreshold=0.95 vs control: cut the same hole into both, compare health.
for _, v in ipairs({ { "ctl", "", 10 }, { "thr", "brokenthreshold=0.95", 20 } }) do
	local s = spawn(car(v[2]), Vec(v[3], 0, 70))
	SetInt(K .. "thr_" .. v[1], s.vehicle or 0)
	if s.body then
		R["thr_" .. v[1] .. "_health_before"] = GetVehicleHealth(s.vehicle)
		local p = GetBodyTransform(s.body).pos
		MakeHole(VecAdd(p, Vec(0, 0.8, 0)), 0.6, 0.6, 0.6)
	end
end

-- nodrive: can the server seat the host in it?
local nd = spawn(car("nodrive"), Vec(40, 0, 70))
SetInt(K .. "nodrive", nd.vehicle or 0)

-- invisible: a 1 m box in the open, plus an untagged one beside it.
for _, v in ipairs({ { "inv", "invisible", 0 }, { "ctl", "", 3 } }) do
	local s = spawn('<body><voxbox size="10 10 10" material="wood" color="0.9 0.2 0.2" tags="' .. v[2] .. '"/></body>', Vec(v[3], 0, 60))
	SetInt(K .. "vis_" .. v[1], s.shape or 0)
end

-- booster=5 on a dynamic box, smoke on another; positions read in step B.
local b = spawn('<body dynamic="true"><voxbox size="5 5 5" material="metal" tags="booster=5"/></body>', Vec(10, 0.2, 60))
SetInt(K .. "booster", b.shape or 0)
local sm = spawn('<body dynamic="true"><voxbox size="5 5 5" material="metal" tags="smoke"/></body>', Vec(15, 0.2, 60))
SetInt(K .. "smoke", sm.shape or 0)

-- impactexplode: body tagged impactexplode with an explosive shape, dropped from 6 m; the
-- control has the explosive shape only. Explosions show in probeevents' level.probeev.*.
spawn('<body dynamic="true" tags="impactexplode"><voxbox size="4 4 4" material="wood" tags="explosive=1"/></body>', Vec(25, 6, 60))
spawn('<body dynamic="true"><voxbox size="4 4 4" material="wood" tags="explosive=1"/></body>', Vec(30, 6, 60))

-- night: two lights, one tagged night; nightlight is switched in step B.
for _, v in ipairs({ { "night", "night", 50 }, { "ctl", "", 55 } }) do
	local s = spawn('<light pos="0 2 0" type="sphere" color="1 0.8 0.6" scale="3" tags="' .. v[2] .. '"/>', Vec(v[3], 0, 60))
	SetInt(K .. "light_" .. v[1], s.light or 0)
end
R.nightlight = GetEnvironmentProperty("nightlight")
R.lights_active = { night = IsLightActive(GetInt(K .. "light_night")), ctl = IsLightActive(GetInt(K .. "light_ctl")) }
R.time = GetTime()
return R
