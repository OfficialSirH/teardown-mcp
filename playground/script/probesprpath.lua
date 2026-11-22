#version 2
-- Probe: which path forms LoadSprite accepts for another mod's image, and whether DrawSprite
-- takes a source rectangle (UV) in undocumented extra arguments. Spawn from the bridge:
--   Spawn('<script file="MOD/../mcp-playground/script/probesprpath.lua"/>', Transform())
-- Draws, 2 m in front of where the local player stood at init, one row of 0.4 m quads, from
-- the left: one per path form (PATHS order), then the UV test: the same image drawn normally
-- and with extra arguments 0.5, 0, 1, 0.5 (a source rectangle would show a quarter of it).
-- Writes level.probesprpath.<i> = "<path> handle=<h> has=<HasFile>" (client half, not synced).
local PATHS = {
	"MOD/../teardown-screens/tests/probe/a.png",
	"local-teardown-screens:tests/probe/a.png",
	"../teardown-screens/tests/probe/a.png",
	"MOD/../teardown-screens/tests/probe/missing.png",
}
local handles = {}
local base

function client.init()
	for i, p in ipairs(PATHS) do
		handles[i] = LoadSprite(p)
		SetString("level.probesprpath." .. i, p .. " handle=" .. tostring(handles[i]) .. " has=" .. tostring(HasFile(p)))
	end
	local t = GetPlayerCameraTransform()
	local fwd = TransformToParentVec(t, Vec(0, 0, -1))
	fwd[2] = 0
	fwd = VecNormalize(fwd)
	local yaw = math.deg(math.atan2(-fwd[1], -fwd[3]))
	-- quads face back toward the player (+z toward them)
	base = Transform(VecAdd(t.pos, VecScale(fwd, 2)), QuatEuler(0, yaw, 0))
end

function client.render()
	if not base then return end
	local n = #PATHS + 2
	for i = 1, n do
		local x = (i - (n + 1) / 2) * 0.5
		local tt = Transform(TransformToParentPoint(base, Vec(x, 0, 0)), base.rot)
		if i <= #PATHS then
			DrawSprite(handles[i], tt, 0.4, 0.4, 1, 1, 1, 1, true)
		elseif i == #PATHS + 1 then
			DrawSprite(handles[1], tt, 0.4, 0.4, 0.4, 1, 0.4, 1, true)
		else
			DrawSprite(handles[1], tt, 0.4, 0.4, 1, 0.4, 0.4, 1, true, false, false, 0.5, 0, 1, 0.5)
		end
	end
end
