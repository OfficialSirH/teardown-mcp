#version 2
-- Global game mode "MCP global probe": records when its callbacks run, as lines in the game
-- log (a failed LoadSound logs its path), because registry keys do not survive a mode switch.
-- Lines: mcpglobalprobe_<what>_<GetTime ms>. The client half writes them, so the server's
-- line is not replicated to joined clients; the server half writes only destroy.
-- Started with mp_test_start game_mode "local-mcp-probe-mode.mcp-global-probe" and
-- extra_mods ["local-mcp-probe-mode"]; stopped with SetString("game.gamemode.next", "").

local function logLine(what)
	pcall(LoadSound, "MOD/mcpglobalprobe_" .. what .. "_" .. math.floor(GetTime() * 1000) .. ".ogg")
end

function server.init() SetString("level.mcpglobalprobe.server", "init") end
function server.destroy() logLine("server_destroy") end

function client.init() logLine("client_init") end
function client.destroy() logLine("client_destroy") end
