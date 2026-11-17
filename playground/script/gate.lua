#version 2

-- Mini-mod 4: a gate that swings open while a player stands in its trigger zone.
--
-- Server: drives the hinge's motor towards open or closed; publishes the state.
-- Client: a prompt near the gate.

#include "script/include/player.lua"

local hinge, zone = 0, 0

function server.init()
	hinge = FindJoint("mcpgatehinge", true)
	zone = FindTrigger("mcpgatezone", true)
end

function server.tick(dt)
	if hinge == 0 or zone == 0 then return end
	local occupied = GetBool("level.gate.force")
	for p in Players() do
		if IsPointInTrigger(zone, GetPlayerTransform(p).pos) then occupied = true end
	end
	SetJointMotorTarget(hinge, occupied and 90 or 0, 1.5, 500)
	SetBool("level.gate.open", occupied, true)
	SetFloat("level.gate.angle", GetJointMovement(hinge), true)
end

function client.draw()
	local zoneC = FindTrigger("mcpgatezone", true)
	if zoneC == 0 then return end
	local pos = VecAdd(GetTriggerTransform(zoneC).pos, Vec(0, 2.6, -3))
	local x, y, dist = UiWorldToPixel(pos)
	if dist <= 0 or dist > 40 then return end
	UiPush()
	UiTranslate(x, y)
	UiAlign("center middle")
	UiFont("bold.ttf", 40)
	UiTextOutline(0, 0, 0, 1, 0.3)
	if GetBool("level.gate.open") then
		UiColor(0.3, 1, 0.4, 1)
		UiText(string.format("GATE OPEN  %d deg", GetFloat("level.gate.angle")))
	else
		UiColor(1, 0.8, 0.3, 1)
		UiText("STEP IN FRONT TO OPEN")
	end
	UiPop()
end
