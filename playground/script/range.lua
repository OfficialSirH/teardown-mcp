#version 2

-- Mini-mod 2: a target range.
--
-- Server: a row of breakable targets and a turret that shoots them while level.range.fire is
-- set. Keeps the score in `shared` and mirrors it to level.range.* (synced) so tools outside
-- this script can read it.
-- Client: scoreboard, health bars over the targets, muzzle flash and sound per shot.

local TURRET = Vec(0, 1.5, -6)
local targets = {}      -- { body, shape, full }
local cooldown, nextTarget = 0, 1
local shotSound

function server.init()
	for i = 1, 5 do
		local x = (i - 3) * 3
		local hs = Spawn('<body dynamic="true" tags="mcptarget"><voxbox pos="-0.5 0 -0.2" size="10 14 4" material="wood" color="0.7 0.5 0.2"/></body>',
			Transform(Vec(x, 0.1, -22)))
		local shape = GetBodyShapes(hs[1])[1]
		targets[i] = { body = hs[1], shape = shape, full = GetShapeVoxelCount(shape) }
	end
	local pole = Spawn('<body tags="mcpturret"><voxbox pos="-0.1 0 -0.1" size="2 15 2" color="0.2 0.2 0.25"/></body>', Transform(Vec(TURRET[1], 0, TURRET[3])))
	shared.range = { score = 0, total = #targets, shots = 0, targets = {} }
	for i, t in ipairs(targets) do shared.range.targets[i] = { body = t.body, shape = t.shape, full = t.full } end
	SetInt("level.range.total", #targets, true)
end

-- A hit target breaks apart: the pieces become new bodies and the original shape is left
-- with few or no voxels, so "is the body broken" is the reliable test, not a voxel count.
local function standing(t)
	return IsHandleValid(t.body) and not IsBodyBroken(t.body)
end

function server.tick(dt)
	local score = 0
	for _, t in ipairs(targets) do
		if not standing(t) then score = score + 1 end
	end
	if score ~= shared.range.score then
		shared.range.score = score
		SetInt("level.range.score", score, true)
	end

	cooldown = cooldown - dt
	if GetBool("level.range.fire") and cooldown <= 0 and score < #targets then
		cooldown = 0.25
		-- next target that still stands
		for _ = 1, #targets do
			local t = targets[nextTarget]
			nextTarget = nextTarget % #targets + 1
			if standing(t) then
				local aim = VecAdd(GetBodyTransform(t.body).pos, Vec(0, 0.7, 0))
				local dir = VecNormalize(VecSub(aim, TURRET))
				-- Strength scales the size of the damage: 3 wrecks props 3 m to either side.
				Shoot(TURRET, dir, "bullet", 1)
				shared.range.shots = shared.range.shots + 1
				SetInt("level.range.shots", shared.range.shots, true)
				ClientCall(0, "client.rangeShot", TURRET[1], TURRET[2], TURRET[3])
				break
			end
		end
	end
end

-- Client ---------------------------------------------------------------

local flash = 0

function client.init()
	shotSound = LoadSound("tools/gun0.ogg")
end

function client.rangeShot(x, y, z)
	local p = Vec(x, y, z)
	PlaySound(shotSound, p, 0.6)
	flash = 0.08
	ParticleReset()
	ParticleType("smoke")
	ParticleRadius(0.1, 0.4)
	ParticleAlpha(0.6, 0)
	SpawnParticle(p, Vec(0, 0.5, -1), 0.6)
end

function client.tick(dt)
	if flash > 0 then
		flash = flash - dt
		PointLight(TURRET, 1, 0.8, 0.4, 3)
	end
end

function client.draw()
	local r = shared.range
	if not r then return end

	-- Health bar over each target
	for i, t in ipairs(r.targets) do
		if IsHandleValid(t.body) and not IsBodyBroken(t.body) then
			local left = GetShapeVoxelCount(t.shape) / t.full
			local pos = VecAdd(GetShapeWorldTransform(t.shape).pos, Vec(0.5, 1.9, 0))
			local x, y, dist = UiWorldToPixel(pos)
			if dist > 0 and dist < 60 then
				UiPush()
				UiTranslate(x, y)
				UiAlign("center middle")
				UiColor(0, 0, 0, 0.7)
				UiRect(84, 14)
				UiAlign("left middle")
				UiTranslate(-40, 0)
				if left >= 0.5 then UiColor(0.2, 0.9, 0.3, 1) else UiColor(0.9, 0.2, 0.2, 1) end
				UiRect(80 * left, 10)
				UiPop()
			end
		end
	end

	-- Scoreboard, top right
	UiPush()
	UiTranslate(UiWidth() - 40, 240)
	UiAlign("right top")
	UiColor(0, 0, 0, 0.65)
	UiRoundedRect(420, 150, 12)
	UiTranslate(-24, 18)
	UiColor(1, 0.85, 0.3, 1)
	UiFont("bold.ttf", 40)
	UiText("TARGET RANGE")
	UiTranslate(0, 52)
	UiColor(1, 1, 1, 1)
	UiFont("regular.ttf", 32)
	UiText(string.format("Targets down: %d / %d", r.score, r.total))
	UiTranslate(0, 38)
	UiText(string.format("Shots fired: %d", r.shots))
	UiPop()
end
