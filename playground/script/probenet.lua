#version 2
-- Probe: what ServerCall tells the server about its caller, whether ClientCall reaches only
-- its target, and which effects called by the SERVER show on a joined client. Not part of the
-- playground level. Spawn it from the bridge (run_lua, server side) into any level:
--   Spawn('<script file="MOD/../mcp-playground/script/probenet.lua"/>', Transform())
-- Triggers (registry):
--   client side (side: client = host's client half, side: remote = joined client):
--     level.probenet.call = true      ServerCall("server.probeWho", GetLocalPlayer(), "hello")
--     level.probenet.spoof = <id>     ServerCall("server.probeWho", <id>, "spoofed") (claims to be <id>)
--   server side:
--     level.probenet.cc = <id>        ClientCall(<id>, "client.probeMsg", "to <id>") (0 = everyone)
--     level.probenet.fx = true        for level.probenet.dur s (default 3) the server half spawns
--                                     level.probenet.n particles (default 5) per tick, a PointLight
--                                     and a DrawLine at level.probenet.x/y/z; with .nofx = true it
--                                     draws nothing itself, with .cceach = true it also sends one
--                                     ClientCall per tick and each client spawns the particles
-- Results:
--   server: level.probenet.who.n, level.probenet.who.<i> = "args: <count> | <a1> | <a2> | ..."
--   each client: level.probenet.got = the last ClientCall argument it received

function server.probeWho(...)
	local parts = { "args: " .. select("#", ...) }
	for i = 1, select("#", ...) do parts[#parts + 1] = tostring((select(i, ...))) end
	local n = GetInt("level.probenet.who.n") + 1
	SetInt("level.probenet.who.n", n)
	SetString("level.probenet.who." .. n, table.concat(parts, " | "))
end

local fxLeft = 0

function server.tick(dt)
	local cc = GetInt("level.probenet.cc")
	if cc >= 0 and HasKey("level.probenet.cc") then
		ClearKey("level.probenet.cc")
		ClientCall(cc, "client.probeMsg", "to " .. cc)
	end
	if GetBool("level.probenet.fx") then
		SetBool("level.probenet.fx", false)
		fxLeft = GetFloat("level.probenet.dur")
		if fxLeft <= 0 then fxLeft = 3 end
	end
	if fxLeft > 0 then
		fxLeft = fxLeft - dt
		local p = Vec(GetFloat("level.probenet.x"), GetFloat("level.probenet.y"), GetFloat("level.probenet.z"))
		ParticleReset()
		ParticleColor(0.2, 1, 0.3)
		ParticleRadius(0.3)
		ParticleEmissive(4)
		local n = GetInt("level.probenet.n")
		if n <= 0 then n = 5 end
		if GetBool("level.probenet.nofx") then n = 0 end
		for _ = 1, n do SpawnParticle(VecAdd(p, Vec(math.random() - 0.5, math.random(), math.random() - 0.5)), Vec(0, 1, 0), 1.0) end
		if not GetBool("level.probenet.nofx") then
			PointLight(p, 0.2, 1, 0.3, 4)
			DrawLine(p, VecAdd(p, Vec(0, 3, 0)), 1, 0, 1, 1)
		end
		-- level.probenet.cceach = true: one ClientCall per tick instead (the clients make the effect)
		if GetBool("level.probenet.cceach") then ClientCall(0, "client.probeFx", p[1], p[2], p[3]) end
	end
end

function client.probeFx(x, y, z)
	local n = GetInt("level.probenet.n")
	if n <= 0 then n = 5 end
	ParticleReset()
	ParticleColor(1, 0.3, 0.2)
	ParticleRadius(0.3)
	ParticleEmissive(4)
	for _ = 1, n do SpawnParticle(Vec(x + math.random() - 0.5, y + math.random(), z + math.random() - 0.5), Vec(0, 1, 0), 1.0) end
end

function client.probeMsg(text)
	SetString("level.probenet.got", text .. " (local player " .. GetLocalPlayer() .. ")")
end

function client.tick(dt)
	if GetBool("level.probenet.call") then
		SetBool("level.probenet.call", false)
		ServerCall("server.probeWho", GetLocalPlayer(), "hello")
	end
	local spoof = GetInt("level.probenet.spoof")
	if spoof ~= 0 then
		SetInt("level.probenet.spoof", 0)
		ServerCall("server.probeWho", spoof, "spoofed")
	end
end
