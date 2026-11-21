#version 2
-- Probe: drives every vehicle tagged "mcpdrive" straight ahead at full throttle each server
-- tick (DriveVehicle has to be called every tick) and records its top speed in km/h under
-- level.probedrive.<vehicle handle>. Not part of the level; spawn it from the bridge:
--   Spawn('<script file="MOD/../mcp-playground/script/probedrive.lua"/>', Transform())

function server.tick()
	for _, v in ipairs(FindVehicles("mcpdrive", true)) do
		DriveVehicle(v, 1, 0, false)
		local kmh = VecLength(GetBodyVelocity(GetVehicleBody(v))) * 3.6
		local k = "level.probedrive." .. v
		if kmh > GetFloat(k) then SetFloat(k, kmh) end
	end
end
