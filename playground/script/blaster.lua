#version 2

-- Mini-mod 5: a custom hand tool, the Blaster.
--
-- Server: registers the tool, hands it to every player, and on fire blows a small hole where
-- the player aims. Fires on the use-tool input, or when level.blaster.fire holds a player id
-- (so it can be triggered without a mouse).
-- Client: a short beam and a flash for each shot.

#include "script/include/player.lua"

local beams = {}

function server.init()
	RegisterTool("mcpblaster", "Blaster", "MOD/prefab/blaster.xml", 6)
end

local function fire(p)
	local muzzle = GetToolLocationWorldTransform("muzzle", p)
	local from = muzzle and muzzle.pos or GetPlayerCameraTransform(p).pos
	local hit, startPos, endPos, dir = GetPlayerAimInfo(from, 60, p)
	Explosion(endPos, 0.5)
	SetInt("level.blaster.shots", GetInt("level.blaster.shots") + 1, true)
	SetString("level.blaster.last", string.format("hit=%s end=%.1f,%.1f,%.1f muzzle=%s", tostring(hit), endPos[1], endPos[2], endPos[3], tostring(muzzle ~= nil)), true)
	ClientCall(0, "client.blasterBeam", from[1], from[2], from[3], endPos[1], endPos[2], endPos[3])
end

function server.tick(dt)
	for p in PlayersAdded() do
		SetToolEnabled("mcpblaster", true, p)
		SetToolAmmo("mcpblaster", 50, p)
	end
	for p in Players() do
		if GetPlayerTool(p) == "mcpblaster" and InputPressed("usetool", p) then fire(p) end
	end
	local forced = GetInt("level.blaster.fire")
	if forced ~= 0 then
		SetInt("level.blaster.fire", 0)
		fire(forced)
	end
end

function client.blasterBeam(x0, y0, z0, x1, y1, z1)
	beams[#beams + 1] = { from = Vec(x0, y0, z0), to = Vec(x1, y1, z1), life = 0.25 }
end

function client.tick(dt)
	-- A tool is drawn at the camera itself unless the script places it every frame.
	if GetPlayerTool() == "mcpblaster" then
		SetToolTransform(Transform(Vec(0.3, -0.3, -0.5)), 1.0)
	end
	for i = #beams, 1, -1 do
		local b = beams[i]
		b.life = b.life - dt
		if b.life <= 0 then
			table.remove(beams, i)
		else
			DrawLine(b.from, b.to, 0.3, 0.7, 1, b.life * 4)
			PointLight(b.to, 0.3, 0.6, 1, 4)
		end
	end
end
