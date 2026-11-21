-- Tag probe, round 2, step B (run_lua side=server, 4-5 s after tags2_a.lua).
local K = "level.probetag2."
local R = { time = GetTime() }

local function veh(name) return GetInt(K .. name) end

R.top_speed_kmh = {
	ctl = GetFloat("level.probedrive." .. veh("drive_ctl")),
	passive = GetFloat("level.probedrive." .. veh("drive_passive")),
}

local function up(v)
	local t = GetBodyTransform(GetVehicleBody(v))
	return string.format("%.2f", TransformToParentVec(t, Vec(0, 1, 0))[2])
end
R.roll_up_y = { ctl = up(veh("roll_ctl")), noroll = up(veh("roll_noroll")) }

R.health_after_hole = { ctl = GetVehicleHealth(veh("thr_ctl")), thr = GetVehicleHealth(veh("thr_thr")) }

local nd = veh("nodrive")
SetPlayerVehicle(nd, 1)
R.nodrive_seated = GetPlayerVehicle(1) == nd
SetPlayerVehicle(0, 1)

local function hitAt(x)
	QueryRequire("physical")
	local hit, dist, n, s = QueryRaycast(Vec(x, 3, 60.5), Vec(0, -1, 0), 5)
	return tostring(hit) .. " shape " .. tostring(s)
end
R.invisible = {
	inv_shape = veh("vis_inv"), inv_raycast = hitAt(0.5), inv_voxels = GetShapeVoxelCount(veh("vis_inv")),
	ctl_shape = veh("vis_ctl"), ctl_raycast = hitAt(3.5),
}

local function pos(s)
	if not IsHandleValid(s) then return "gone" end
	local p = GetBodyTransform(GetShapeBody(s)).pos
	return string.format("%.2f %.2f %.2f tag=%s", p[1], p[2], p[3], tostring(GetTagValue(s, "booster")))
end
R.booster = pos(veh("booster"))
R.smoke = pos(veh("smoke"))

SetEnvironmentProperty("nightlight", false)
R.night_off = { night = IsLightActive(veh("light_night")), ctl = IsLightActive(veh("light_ctl")) }
return R
