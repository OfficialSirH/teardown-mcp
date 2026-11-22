-- Probe: does a <screen> script run, and draw, in multiplayer, and what is shared between the
-- copies? A version 1 script (no #version line), like the game's own ui/terminal.lua. Not part
-- of the playground level. Spawn a screen with it from the bridge (run_lua, server side), e.g.:
--   Spawn('<body tags="interact"><voxbox pos="-1 0 -0.05" size="20 12 1" color="0.1 0.1 0.1">' ..
--     '<screen pos="1 0.6 0.11" size="1.8 1.0" script="MOD/../mcp-playground/screen/probescreen.lua" enabled="true" interactive="true"/>' ..
--     '</voxbox></body>', Transform(pos), true)
-- The screen shows: its own frame counter (each game's copy counts for itself), the mouse
-- position this copy sees, how often its button was clicked, and GetTime().
-- Counts also go to this game's registry (host: side client; joined client: side remote):
--   level.probescreen.init / .tick / .update / .draw / .clicks

frame = 0
clicks = 0

local function bump(k) SetInt("level.probescreen." .. k, GetInt("level.probescreen." .. k) + 1) end

-- Values that would differ between machines if every game ran its own copy
local rnd = 0
local who = "?"
function init()
	bump("init")
	rnd = math.random(1000, 9999)
	local ok, lp = pcall(GetLocalPlayer)
	who = ok and tostring(lp) or "err"
end
function tick() frame = frame + 1 bump("tick") end
function update() bump("update") end

function draw()
	bump("draw")
	UiColor(0.08, 0.1, 0.14, 1)
	UiRect(UiWidth(), UiHeight())
	UiColor(1, 0.8, 0.3, 1)
	UiFont("bold.ttf", 48)
	UiPush()
	UiTranslate(UiCenter(), 70)
	UiAlign("center middle")
	UiText("FRAME " .. frame)
	UiTranslate(0, 60)
	UiFont("regular.ttf", 32)
	local mx, my = UiGetMousePos()
	UiText(string.format("mouse %d %d   time %.1f", mx, my, GetTime()))
	UiTranslate(0, 50)
	UiText(UiWidth() .. " x " .. UiHeight() .. "   random " .. rnd .. "   local player " .. who)
	UiTranslate(0, 45)
	local okc, lp = pcall(GetLocalPlayer)
	UiText("now: local player " .. (okc and tostring(lp) or "err") .. "   host " .. tostring(GetBool("game.multiplayer.ishost")) .. "   name " .. tostring(okc and GetPlayerName and GetPlayerName(lp) or "?"))
	UiPop()
	UiPush()
	UiTranslate(UiCenter(), 330)
	UiAlign("center middle")
	UiFont("bold.ttf", 40)
	UiColor(0.3, 0.6, 1, 1)
	if UiTextButton("CLICK ME (" .. clicks .. ")", 360, 70) then
		clicks = clicks + 1
		bump("clicks")
	end
	UiPop()
end
