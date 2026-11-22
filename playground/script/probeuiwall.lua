#version 2
-- Probe: draw ordinary Ui* content onto a rectangle in the world. Not part of the playground
-- level. Spawn it from the bridge (run_lua, server side) into any level:
--   Spawn('<script file="MOD/../mcp-playground/script/probeuiwall.lua"/>', Transform())
-- client.draw projects three corners of a world rectangle with UiWorldToPixel, fits
-- UiRotate * UiScale * UiRotate (an affine map) to them, and draws a panel of W x H UI units
-- (background, text, bar, and a rectangle larger than the panel under UiClipRect). client.tick
-- outlines the same rectangle in the world with DrawLine, to compare.
-- Settings (client registry): level.probeuiwall.x/y/z (rectangle centre), .yaw (degrees),
-- .w/.h (metres, default 1.6 x 1.0), .sign (1 or -1: the UiRotate direction to test),
-- .clip (bool). Results: level.probeuiwall.out (the fitted angles and scales).

local W, H = 400, 250

local function cfg(k) return "level.probeuiwall." .. k end

local function rect()
	local w, h = GetFloat(cfg("w")), GetFloat(cfg("h"))
	if w <= 0 then w, h = 1.6, 1.0 end
	local t = Transform(Vec(GetFloat(cfg("x")), GetFloat(cfg("y")), GetFloat(cfg("z"))), QuatEuler(0, GetFloat(cfg("yaw")), 0))
	local tl = TransformToParentPoint(t, Vec(-w / 2, h / 2, 0))
	local tr = TransformToParentPoint(t, Vec(w / 2, h / 2, 0))
	local bl = TransformToParentPoint(t, Vec(-w / 2, -h / 2, 0))
	local br = TransformToParentPoint(t, Vec(w / 2, -h / 2, 0))
	return tl, tr, bl, br
end

function client.tick()
	local tl, tr, bl, br = rect()
	DrawLine(tl, tr, 1, 0, 0, 1)
	DrawLine(tr, br, 1, 0, 0, 1)
	DrawLine(br, bl, 1, 0, 0, 1)
	DrawLine(bl, tl, 1, 0, 0, 1)
end

function client.draw()
	local tl, tr, bl = rect()
	local x0, y0, d0 = UiWorldToPixel(tl)
	local x1, y1, d1 = UiWorldToPixel(tr)
	local x2, y2, d2 = UiWorldToPixel(bl)
	if d0 <= 0 or d1 <= 0 or d2 <= 0 then return end
	-- columns of the map from panel units to pixels
	local a, c = (x1 - x0) / W, (y1 - y0) / W
	local b, d = (x2 - x0) / H, (y2 - y0) / H
	if a * d - b * c <= 0 then return end -- seen from behind
	-- M = R(phi) * diag(sx, sy) * R(theta)
	local E, F, G, Hh = (a + d) / 2, (a - d) / 2, (c + b) / 2, (c - b) / 2
	local Q, R = math.sqrt(E * E + Hh * Hh), math.sqrt(F * F + G * G)
	local sx, sy = Q + R, Q - R
	local a1, a2 = math.atan2(G, F), math.atan2(Hh, E)
	local theta, phi = (a2 - a1) / 2, (a2 + a1) / 2
	local sign = GetFloat(cfg("sign"))
	if sign == 0 then sign = 1 end
	SetString(cfg("out"), string.format("phi %.1f theta %.1f sx %.3f sy %.3f", math.deg(phi), math.deg(theta), sx, sy))
	UiPush()
	UiTranslate(x0, y0)
	UiRotate(sign * math.deg(phi))
	UiScale(sx, sy)
	UiRotate(sign * math.deg(theta))
	UiColor(0.05, 0.07, 0.09, 0.9)
	UiRect(W, H)
	if GetBool(cfg("clip")) then UiClipRect(W, H) end
	UiColor(0.9, 0.3, 0.2, 0.5)
	UiTranslate(-40, 180)
	UiRect(W + 80, 120) -- sticks out of the panel: clipped or not?
	UiTranslate(40, -180)
	UiColor(1, 0.8, 0.3, 1)
	UiFont("bold.ttf", 48)
	UiTranslate(20, 20)
	UiAlign("left top")
	UiText("HELLO WALL 123")
	UiTranslate(0, 80)
	UiColor(0, 0, 0, 0.6)
	UiRect(360, 24)
	UiColor(0.3, 0.9, 0.4, 1)
	UiRect(360 * 0.6, 24)
	UiPop()
end
