#version 2
-- Probe: sets the registry switches listed in level.probeswitch.on (comma-separated keys,
-- e.g. "game.disablepause,hud.hide") to true every tick and every draw on the host's client
-- half, the way the game's own scripts do. Clear the list to stop. Not part of the level.
-- Spawn it from the bridge (server side):
--   Spawn('<script file="MOD/../mcp-playground/script/probeswitches.lua"/>', Transform())

local function apply()
	local list = GetString("level.probeswitch.on")
	if list == "" then return end
	for key in string.gmatch(list, "[^,]+") do SetBool(key, true) end
end

function client.tick() apply() end
function client.draw() apply() end
