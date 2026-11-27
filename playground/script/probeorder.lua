#version 2
-- probe (2026-11-26): order of server.tick / server.update / server.postUpdate within frames.
-- Records the first 60 server callbacks as letters (T tick, U update, P postUpdate) in level.mcp.order.
-- Ticks run once per frame, so each T starts a frame; the sequence shows where the U and P calls fall.
local seq, n = {}, 0
local function add(c)
	if n >= 60 then return end
	n = n + 1
	seq[n] = c
	if n == 60 then SetString("level.mcp.order", table.concat(seq)) end
end
function server.tick() add("T") end
function server.update() add("U") end
function server.postUpdate() add("P") end
