#version 2

-- Mini-mod 1: a hand-built buggy with an autopilot and a speed HUD.
--
-- Server: spawns the buggy, drives it in a circle while level.buggy.autopilot is set.
-- Client: speed readout and a label floating over the buggy.

local vehicle = 0

function server.init()
	local handles = Spawn("MOD/prefab/buggy.xml", Transform(Vec(8, 0.5, 0)))
	for _, h in ipairs(handles) do
		if GetEntityType(h) == "vehicle" then vehicle = h end
	end
	shared.buggy = { vehicle = vehicle, spawned = #handles }
end

function server.tick(dt)
	if vehicle == 0 then return end
	if GetBool("level.buggy.autopilot") then
		DriveVehicle(vehicle, 0.6, 0.5, false)
	end
end

local function buggySpeed(v)
	local body = GetVehicleBody(v)
	return VecLength(GetBodyVelocity(body)) * 3.6, body
end

function client.draw()
	local v = shared.buggy and shared.buggy.vehicle or 0
	if v == 0 then return end
	local kmh, body = buggySpeed(v)

	-- Label over the buggy
	local top = VecAdd(GetBodyTransform(body).pos, Vec(0, 2.2, 0))
	local x, y, dist = UiWorldToPixel(top)
	if dist > 0 then
		UiPush()
		UiTranslate(x, y)
		UiAlign("center middle")
		UiFont("bold.ttf", 28)
		UiTextOutline(0, 0, 0, 1, 0.2)
		UiText(string.format("BUGGY  %d km/h", kmh))
		UiPop()
	end

	-- Speed bar, bottom centre
	UiPush()
	UiTranslate(UiCenter(), UiHeight() - 140)
	UiAlign("center middle")
	UiColor(0, 0, 0, 0.6)
	UiRoundedRect(420, 60, 10)
	UiAlign("left middle")
	UiTranslate(-200, 0)
	UiColor(0.2, 0.9, 0.3, 1)
	UiRect(400 * math.min(kmh / 60, 1), 30)
	UiColor(1, 1, 1, 1)
	UiFont("bold.ttf", 24)
	UiTranslate(10, 0)
	UiText(string.format("%d km/h", kmh))
	UiPop()
end
