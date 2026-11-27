#version 2
-- probe (2026-11-26): frame cost of rifle-sized holes in a small vs a huge shape.
-- level.mcp.hole.small / .big = shape handles (set before spawning). Three 3 s windows: none, small, big; a burst of
-- 3 holes every 0.25 s at the shape's surface point nearest to the host. Per window: frames, slowest frame (1/GetFps),
-- count of frames over 50 / 100 ms. Result in level.mcp.hole.result.
local W = 3
local phase, t0, stats, results = 0, nil, nil, {}
local nextHole = 0

local function newStats() return { n = 0, worst = 0, over50 = 0, over100 = 0, sum = 0 } end

function server.tick()
	local now = GetTime()
	if not t0 then t0 = now stats = newStats() end
	local fps = GetFps()
	local ft = fps > 0 and 1000 / fps or 0
	stats.n = stats.n + 1
	stats.sum = stats.sum + ft
	if ft > stats.worst then stats.worst = ft end
	if ft > 50 then stats.over50 = stats.over50 + 1 end
	if ft > 100 then stats.over100 = stats.over100 + 1 end
	local target = phase == 1 and GetInt("level.mcp.hole.small") or phase == 2 and GetInt("level.mcp.hole.big") or 0
	if target ~= 0 and now >= nextHole and IsHandleValid(target) then
		nextHole = now + 0.25
		local hit, point = GetShapeClosestPoint(target, VecAdd(GetPlayerTransform(1).pos, Vec(0, 1, 0)))
		if hit then
			for i = 0, 2 do MakeHole(VecAdd(point, Vec(math.random() - 0.5, 0, math.random() * 2)), 0.3, 0.2, 0.1) end
		end
	end
	if now - t0 >= W then
		results[#results + 1] = string.format("%s: %d frames, avg %.0f ms, worst %.0f ms, >50ms %d, >100ms %d",
			({ "none", "small", "big" })[phase + 1], stats.n, stats.sum / math.max(1, stats.n), stats.worst,
			stats.over50, stats.over100)
		phase = phase + 1
		t0, stats = now, newStats()
		if phase > 2 then
			SetString("level.mcp.hole.result", table.concat(results, " | "))
			phase = -99
		end
	end
	if phase < 0 then t0 = now end
end
