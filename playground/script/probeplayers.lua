#version 2
#include "script/include/player.lua"
-- Probe: when GetAddedPlayers() / GetRemovedPlayers() and the include's PlayersAdded() /
-- PlayersRemoved() report players, in each script half. Not part of the level. Spawn it from
-- the bridge (server side):
--   Spawn('<script file="MOD/../mcp-playground/script/probeplayers.lua"/>', Transform())
-- Each non-empty result is one line: level.probeplayers.<half>.<i> = "<tick> <time> <what>: ids"
-- (level.probeplayers.<half>.n counts them; a joined client writes to its own registry).

local function ids(t)
	local o = {}
	for _, v in ipairs(t or {}) do o[#o + 1] = tostring(v) end
	return table.concat(o, ",")
end

local function iter(f)
	local o = {}
	for p in f() do o[#o + 1] = tostring(p) end
	return table.concat(o, ",")
end

local function note(half, tick, what, list)
	if list == "" then return end
	local k = "level.probeplayers." .. half
	local n = GetInt(k .. ".n") + 1
	SetInt(k .. ".n", n)
	if n <= 100 then
		SetString(k .. "." .. n, tick .. " " .. string.format("%.2f", GetTime()) .. " " .. what .. ": " .. list)
	end
end

local function poll(half, tick)
	note(half, tick, "GetAddedPlayers", ids(GetAddedPlayers()))
	note(half, tick, "GetRemovedPlayers", ids(GetRemovedPlayers()))
	note(half, tick, "PlayersAdded", iter(PlayersAdded))
	note(half, tick, "PlayersRemoved", iter(PlayersRemoved))
end

local sTick, cTick = 0, 0
function server.tick() sTick = sTick + 1 poll("server", sTick) end
function client.tick() cTick = cTick + 1 poll("client", cTick) end
