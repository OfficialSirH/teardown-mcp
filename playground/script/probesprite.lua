#version 2
-- Probe: DrawSprite as a world-space screen. Not part of the playground level. Spawn it from the
-- bridge (run_lua, server side) into any level:
--   Spawn('<script file="MOD/../mcp-playground/script/probesprite.lua"/>', Transform())
-- Settings, read every frame from the client half's registry (set them with side: client, or
-- side: remote for the joined client):
--   level.probesprite.where   "tick" (default), "render", "draw", "postUpdate", "update":
--                             the callback that calls DrawSprite
--   level.probesprite.count   panels to draw per frame (default 1), in a grid of 0.1 m cells
--   level.probesprite.depth   bool: depthTest (default false)
--   level.probesprite.x/y/z   world position of the grid's bottom left corner
--   level.probesprite.yaw     degrees; the panels face +z rotated by this
--   level.probesprite.image   image path (default "gfx/white.png")
-- Results under level.probesprite.out: calls per callback this second, frames.

local sprites = {}
local calls = {}
local frames, t0 = 0, 0

local function cfg(k) return "level.probesprite." .. k end

local function sprite(path)
	if not sprites[path] then sprites[path] = LoadSprite(path) end
	return sprites[path]
end

local function draw(from)
	local where = GetString(cfg("where"))
	if where == "" then where = "tick" end
	if where ~= from then return end
	calls[from] = (calls[from] or 0) + 1
	local n = GetInt(cfg("count"))
	if n <= 0 then n = 1 end
	local img = GetString(cfg("image"))
	if img == "" then img = "gfx/white.png" end
	local h = sprite(img)
	local base = Transform(Vec(GetFloat(cfg("x")), GetFloat(cfg("y")), GetFloat(cfg("z"))), QuatEuler(0, GetFloat(cfg("yaw")), 0))
	local depth = GetBool(cfg("depth"))
	local cols = math.max(1, math.floor(math.sqrt(n) * 1.6))
	for i = 0, n - 1 do
		local cx, cy = i % cols, math.floor(i / cols)
		local t = Transform(TransformToParentPoint(base, Vec(cx * 0.1 + 0.05, cy * 0.1 + 0.05, 0)), base.rot)
		local k = (cx + cy) % 2
		DrawSprite(h, t, 0.09, 0.09, k, 0.6 + 0.4 * (cy % 2), 1 - k, 1, depth)
	end
end

function client.init()
	t0 = GetTime()
end

function client.tick(dt)
	frames = frames + 1
	draw("tick")
	if GetTime() - t0 >= 1 then
		local parts = { "frames=" .. frames }
		for k, v in pairs(calls) do parts[#parts + 1] = k .. "=" .. v end
		SetString(cfg("out"), table.concat(parts, " "))
		frames, calls, t0 = 0, {}, GetTime()
	end
end

function client.update(dt) draw("update") end
function client.postUpdate(dt) draw("postUpdate") end
function client.render(dt) draw("render") end
function client.draw(dt) draw("draw") end
