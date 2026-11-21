#version 2
-- Probe: what UiImageAlpha / UiImageOverlayAlpha draw. Spawn it from the bridge (server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeuialpha.lua"/>', Transform())
-- then take a screenshot. Columns, left to right: the image, the mask, the 2-argument form,
-- the 10-argument form the game's own menus use (image rect, then mask rect), and
-- UiImageOverlayAlpha with two masks. Return counts go to level.probeuialpha.*.

-- From a mod script the game's UI images need the "ui/" prefix (the game's own menu scripts
-- write "menu/..." because they resolve from data/ui).
local IMG = "ui/menu/customgame_thumb.jpg"
local MASK = "ui/menu/480x270-rounded-corners-alpha.png"
local MASK2 = "ui/menu/480x270_promo_preveiw_alpha.png"

local function count(...)
	local t = {}
	for i = 1, select("#", ...) do t[i] = tostring((select(i, ...))) end
	return "(" .. table.concat(t, ",") .. ")"
end

function client.draw()
	UiPush()
	UiTranslate(40, 600)
	UiImageBox(IMG, 300, 170, 0, 0)
	UiTranslate(0, 200)
	UiImageBox(MASK, 300, 170, 0, 0)
	UiTranslate(0, 200)
	local n2 = count(UiImageAlpha(IMG, MASK))
	UiTranslate(0, 300)
	local iw, ih = UiGetImageSize(IMG)
	local mw, mh = UiGetImageSize(MASK)
	local n10 = count(UiImageAlpha(IMG, MASK, 0, 0, iw / 2, ih, 0, 0, mw, mh))
	UiTranslate(0, 300)
	local no = count(UiImageOverlayAlpha(IMG, MASK, MASK2))
	UiPop()
	SetString("level.probeuialpha.returns", n2 .. " " .. n10 .. " " .. no .. " imgsize " .. tostring(iw) .. "x" .. tostring(ih))
end
