-- Count negative-handle (client-side) bodies this side can see: all of them, broken ones,
-- dynamic ones, and those far (> 4 m) from every player (player models and held tools stay
-- near their player). Run with run_lua `file`, any side.
local total, broken, dyn, far = 0, 0, 0, 0
local players = GetAllPlayers()
for _, b in ipairs(FindBodies("", true)) do
	if b < 0 then
		total = total + 1
		if IsBodyBroken(b) then broken = broken + 1 end
		if IsBodyDynamic(b) then dyn = dyn + 1 end
		local c = TransformToParentPoint(GetBodyTransform(b), GetBodyCenterOfMass(b))
		local near = false
		for _, p in ipairs(players) do
			if VecLength(VecSub(c, GetPlayerTransform(p).pos)) < 4 then near = true end
		end
		if not near then far = far + 1 end
	end
end
return string.format("total=%d broken=%d dynamic=%d far=%d", total, broken, dyn, far)
