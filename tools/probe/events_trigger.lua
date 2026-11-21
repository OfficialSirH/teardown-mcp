-- Event probe triggers (run_lua side=server in the MCP Playground multiplayer test, with
-- playground/script/probeevents.lua spawned). STEP selects what to fire; results are read
-- from level.probeev.* afterwards. Player 2 is the joined client.
local step = STEP or 1
local p2 = GetPlayerTransform(2).pos
if step == 1 then
	-- damage from the script, with an instigator
	ApplyPlayerDamage(2, 0.2, "mcpprobe", 1)
	-- a bullet fired by player 1 straight at player 2's chest, from 3 m away
	local from = VecAdd(p2, Vec(0, 1.2, 3))
	Shoot(from, Vec(0, 0, -1), "bullet", 1, 10, 1)
	-- a rocket into the ground far away
	Shoot(Vec(-20, 3, 60), Vec(0, -1, 0), "rocket", 1, 10, 1)
	-- a custom event with mixed parameters
	PostEvent("mcpprobe", 7, "seven", true)
	return { p2 = p2, health2 = GetPlayerHealth(2) }
elseif step == 2 then
	SetPlayerHealth(0, 2)
	return { health2 = GetPlayerHealth(2) }
elseif step == 3 then
	RespawnPlayerAtTransform(Transform(Vec(3, 0, 5)), 2)
	return { health2 = GetPlayerHealth(2) }
end
