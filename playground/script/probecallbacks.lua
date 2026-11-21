#version 2
-- Probe: which callbacks run, how often, and what handleCommand / destroy receive.
-- Not part of the playground level. Spawn it from the bridge (run_lua, server side):
--   Spawn('<script file="MOD/../mcp-playground/script/probecallbacks.lua"/>', Transform())
-- Results go to the registry under level.probecb.<half>, where half is "server" or
-- "client" (on a joined client the client half writes to that client's own registry):
--   <cb>          calls of each callback (tick, update, postUpdate, draw, render)
--   rendernotick  render calls with no tick since the previous render (rendering while
--                 ticks stop, e.g. in a pause menu)
--   drawnotick    the same for draw
--   lastrendertime / lasttime   GetTime() in the last render / the last tick
--   hc.n, hc.<i>  handleCommand calls: each argument as type:value, " | " between them
--   destroy       set to GetTime() when destroy() runs

local function key(half, name) return "level.probecb." .. half .. "." .. name end

local function bump(half, name)
	local k = key(half, name)
	SetInt(k, GetInt(k) + 1)
end

local function recordCommand(half, ...)
	local parts = {}
	for i = 1, select("#", ...) do
		local v = select(i, ...)
		parts[#parts + 1] = type(v) .. ":" .. tostring(v)
	end
	local n = GetInt(key(half, "hc.n")) + 1
	SetInt(key(half, "hc.n"), n)
	if n <= 50 then SetString(key(half, "hc." .. n), table.concat(parts, " | ")) end
end

function server.tick(dt) bump("server", "tick") SetFloat(key("server", "lasttime"), GetTime()) end
function server.update(dt) bump("server", "update") end
function server.postUpdate(dt) bump("server", "postUpdate") end
function server.render(dt) bump("server", "render") end
function server.draw(dt) bump("server", "draw") end
function server.handleCommand(...) recordCommand("server", ...) end
function server.destroy() SetFloat(key("server", "destroy"), GetTime()) end

local ticks, ticksAtRender, ticksAtDraw = 0, -1, -1

function client.tick(dt)
	ticks = ticks + 1
	bump("client", "tick")
	SetFloat(key("client", "lasttime"), GetTime())
end
function client.update(dt) bump("client", "update") end
function client.postUpdate(dt) bump("client", "postUpdate") end
function client.render(dt)
	bump("client", "render")
	if ticks == ticksAtRender then bump("client", "rendernotick") end
	ticksAtRender = ticks
	SetFloat(key("client", "lastrendertime"), GetTime())
	SetFloat(key("client", "lastrenderdt"), dt or -1)
end
function client.draw(dt)
	bump("client", "draw")
	if ticks == ticksAtDraw then bump("client", "drawnotick") end
	ticksAtDraw = ticks
end
function client.handleCommand(...) recordCommand("client", ...) end
function client.destroy() SetFloat(key("client", "destroy"), GetTime()) end
