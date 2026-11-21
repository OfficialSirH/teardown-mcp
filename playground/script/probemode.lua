#version 2
-- The playground's content game mode "MCP probe" (../gamemodes.txt: layers = mcpprobe; the
-- level loads this script from a layer="mcpprobe" group). It only records its own life
-- cycle: game.mcpprobemode.<half>.init / .ticks / .destroy in the registry, and a log line
-- for init and destroy (a failed LoadSound puts its path in the game's log).

local function mark(half, what)
	SetString("game.mcpprobemode." .. half .. "." .. what, tostring(GetTime()))
end

local function logLine(what)
	pcall(LoadSound, "MOD/probemode_" .. what .. "_" .. math.floor(GetTime() * 1000) .. ".ogg")
end

function server.init() mark("server", "init") end
function server.tick() SetInt("game.mcpprobemode.server.ticks", GetInt("game.mcpprobemode.server.ticks") + 1) end
function server.destroy() mark("server", "destroy") logLine("server_destroy") end

function client.init() mark("client", "init") logLine("client_init") end
function client.tick() SetInt("game.mcpprobemode.client.ticks", GetInt("game.mcpprobemode.client.ticks") + 1) end
function client.destroy() mark("client", "destroy") logLine("client_destroy") end
