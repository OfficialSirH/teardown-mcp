#version 2
-- Probe: which engine events each half sees, and their arguments.
-- Not part of the playground level. Spawn it from the bridge (run_lua, server side):
--   Spawn('<script file="MOD/../mcp-playground/script/probeevents.lua"/>', Transform())
-- Each tick it reads GetEventCount / GetEvent for the names below and appends one registry
-- line per event: level.probeev.<half>.<i> = "<tick> <name> <i>/<count>: type:value | ..."
-- (level.probeev.<half>.n holds the count; a joined client writes to its own registry).
-- Event names from the engine's own table (teardown.exe strings) plus one custom name.

local NAMES = {
	"playerhurt", "playerhit", "playerdied", "explosion", "projectilehit",
	"memberadded", "memberremoved", "netsessionjoinblocked", "netsessionended",
	"connectiontimeout", "jipavailabilitychanged", "custom", "mcpprobe",
}

local function fmt(...)
	local parts = {}
	for i = 1, select("#", ...) do
		local v = select(i, ...)
		if type(v) == "table" then
			local t = {}
			for k, x in pairs(v) do t[#t + 1] = tostring(k) .. "=" .. tostring(x) end
			v = "{" .. table.concat(t, ",") .. "}"
			parts[#parts + 1] = "table:" .. v
		else
			parts[#parts + 1] = type(v) .. ":" .. tostring(v)
		end
	end
	return table.concat(parts, " | ")
end

local function poll(half, tick)
	for _, name in ipairs(NAMES) do
		local count = GetEventCount(name)
		for i = 1, count do
			local k = "level.probeev." .. half
			local n = GetInt(k .. ".n") + 1
			SetInt(k .. ".n", n)
			if n <= 200 then
				SetString(k .. "." .. n, tick .. " " .. name .. " " .. i .. "/" .. count .. ": " .. fmt(GetEvent(name, i)))
			end
		end
	end
end

local sTick, cTick = 0, 0
function server.tick() sTick = sTick + 1 poll("server", sTick) end
function client.tick() cTick = cTick + 1 poll("client", cTick) end
