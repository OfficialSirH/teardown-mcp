local v = FindVehicle("mcpbuggy", true)
local b = GetVehicleBody(v)
local t = GetBodyTransform(b)
local wheels = {}
for i, w in ipairs(GetVehicleWheels and GetVehicleWheels(v) or {}) do wheels[i] = w end
return string.format("pos %.2f %.2f %.2f  speed %.1f km/h  health %.2f  driver(player0 vehicle)=%s  wheels=%d  mass=%.0f",
	t.pos[1], t.pos[2], t.pos[3], VecLength(GetBodyVelocity(b)) * 3.6, GetVehicleHealth(v), tostring(GetPlayerVehicle(0)), #wheels, GetBodyMass(b))
