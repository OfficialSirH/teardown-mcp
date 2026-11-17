#version 2

-- Mini-mod 3: a director.
--
-- Server: level.director.dusk fades the weather from noon to a rainy dusk.
-- Client: level.director.orbit takes the camera on an orbit around the range, with letterbox
-- bars, a caption and a (display-only) settings panel.

local CENTER = Vec(0, 1, -16)
local dusk = 0

local function lerp(a, b, t) return a + (b - a) * t end

function server.tick(dt)
	local target = GetBool("level.director.dusk") and 1 or 0
	if dusk == target then return end
	dusk = math.max(0, math.min(1, dusk + (target > dusk and dt or -dt) / 3))
	SetEnvironmentProperty("sunBrightness", lerp(6, 0.6, dusk))
	SetEnvironmentProperty("skyboxtint", 1, lerp(1, 0.55, dusk), lerp(1, 0.35, dusk))
	SetEnvironmentProperty("skyboxbrightness", lerp(1, 0.35, dusk))
	SetEnvironmentProperty("fogColor", lerp(0.9, 0.45, dusk), lerp(0.9, 0.25, dusk), lerp(0.9, 0.2, dusk))
	SetEnvironmentProperty("rain", dusk * 0.8)
	SetEnvironmentProperty("wetness", dusk)
	SetFloat("level.director.duskamount", dusk, true)
end

local angle, orbitTime = 0, 0

function client.tick(dt)
	if not GetBool("level.director.orbit") then
		orbitTime = 0
		return
	end
	orbitTime = orbitTime + dt
	angle = angle + dt * 0.35
	local pos = VecAdd(CENTER, Vec(math.cos(angle) * 16, 7, math.sin(angle) * 16))
	SetCameraTransform(Transform(pos, QuatLookAt(pos, CENTER)))
	SetCameraFov(60)
end

local function panelRow(label, value, fill)
	UiPush()
	UiAlign("left middle")
	UiColor(1, 1, 1, 0.9)
	UiFont("regular.ttf", 28)
	UiText(label)
	UiTranslate(230, 0)
	UiColor(1, 1, 1, 0.15)
	UiRoundedRect(240, 12, 6)
	UiColor(1, 0.8, 0.3, 1)
	UiRoundedRect(240 * fill, 12, 6)
	UiTranslate(240 * fill, 0)
	UiAlign("center middle")
	UiColor(1, 1, 1, 1)
	UiCircle(13)
	UiTranslate(260 - 240 * fill, 0)
	UiAlign("left middle")
	UiText(value)
	UiPop()
end

function client.draw()
	if not GetBool("level.director.orbit") then return end
	local w, h = UiWidth(), UiHeight()
	local fade = math.min(orbitTime / 1.0, 1)

	-- Letterbox
	UiPush()
	UiColor(0, 0, 0, fade)
	UiRect(w, 170)
	UiTranslate(0, h - 170)
	UiRect(w, 170)
	UiPop()

	-- Caption
	UiPush()
	UiTranslate(70, h - 300)
	UiAlign("left bottom")
	UiColor(1, 1, 1, fade)
	UiFont("bold.ttf", 64)
	UiTextShadow(0, 0, 0, 0.6, 2.0)
	UiText("THE RANGE")
	UiTranslate(0, 44)
	UiFont("regular.ttf", 30)
	UiColor(1, 1, 1, fade * 0.8)
	UiText(string.format("orbit %.0f deg   dusk %d%%", (angle * 57.3) % 360, GetFloat("level.director.duskamount") * 100))
	UiPop()

	-- Settings panel
	UiPush()
	UiTranslate(w - 700, 260)
	UiColor(0.08, 0.09, 0.12, 0.85 * fade)
	UiRoundedRect(640, 300, 16)
	UiTranslate(36, 44)
	UiAlign("left middle")
	UiColor(1, 0.8, 0.3, fade)
	UiFont("bold.ttf", 36)
	UiText("DIRECTOR")
	UiTranslate(0, 64)
	panelRow("Orbit speed", "0.35", 0.35)
	UiTranslate(0, 56)
	panelRow("Field of view", "60", 0.5)
	UiTranslate(0, 56)
	panelRow("Dusk", string.format("%d%%", GetFloat("level.director.duskamount") * 100), GetFloat("level.director.duskamount"))
	UiPop()
end
