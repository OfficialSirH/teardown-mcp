#version 2

-- Mini-mod 6: a clickable menu.
--
-- Client: M opens / closes a panel with buttons and a slider. Each control sends its action to
-- the server with ServerCall.
-- Server: applies the action and counts clicks in level.menu.* (synced) so the result can be
-- read from outside.

local open = false
local slider = 0.5
local name = ""
local typing = false
local focusName = false

function server.menuAction(action, value)
	SetInt("level.menu.clicks", GetInt("level.menu.clicks") + 1, true)
	SetString("level.menu.last", tostring(action), true)
	if action == "dusk" then
		SetBool("level.director.dusk", not GetBool("level.director.dusk"), true)
	elseif action == "autopilot" then
		SetBool("level.buggy.autopilot", not GetBool("level.buggy.autopilot"), true)
	elseif action == "slider" then
		SetFloat("level.menu.slider", value, true)
	elseif action == "name" then
		SetString("level.menu.name", tostring(value), true)
	end
end

function client.tick(dt)
	if InputPressed("m") then
		if typing then
			-- Measured: does a key typed into a text field also reach InputPressed?
			SetInt("level.menu.m_while_typing", GetInt("level.menu.m_while_typing") + 1)
		else
			open = not open
			if open then focusName = false end
			SetBool("level.menu.open", open)
		end
	end
end

local function button(label, action)
	UiPush()
	UiFont("bold.ttf", 36)
	UiButtonImageBox("ui/common/box-outline-6.png", 6, 6, 1, 1, 1, 0.8)
	UiButtonHoverColor(1, 0.8, 0.3, 1)
	if UiTextButton(label, 420, 70) then
		ServerCall("server.menuAction", action, 0)
	end
	UiPop()
end

function client.draw()
	if not open then return end
	UiMakeInteractive()
	UiPush()
	UiTranslate(UiCenter(), 500)
	UiAlign("center top")
	UiColor(0.08, 0.09, 0.12, 0.92)
	UiRoundedRect(560, 600, 18)
	UiTranslate(0, 30)
	UiColor(1, 0.8, 0.3, 1)
	UiFont("bold.ttf", 48)
	UiText("PLAYGROUND MENU")
	UiColor(1, 1, 1, 1)
	UiTranslate(0, 90)
	button("Toggle dusk", "dusk")
	UiTranslate(0, 90)
	button("Toggle buggy autopilot", "autopilot")
	UiTranslate(0, 110)
	UiFont("regular.ttf", 30)
	UiText(string.format("Slider: %.2f    clicks: %d", slider, GetInt("level.menu.clicks")))
	UiTranslate(-200, 60)
	UiAlign("left middle")
	UiColor(1, 1, 1, 0.25)
	UiRect(400, 6)
	UiColor(1, 1, 1, 1)
	UiAlign("center middle")
	local value, done = UiSlider("ui/common/dot.png", "x", slider * 400, 0, 400)
	slider = value / 400
	if done then ServerCall("server.menuAction", "slider", slider) end

	-- A text field: click it, type, the text goes to the server on every change.
	-- UiTextInput draws no frame of its own; the box is drawn at the same top-left corner.
	UiTranslate(0, 38)
	UiAlign("left top")
	UiColor(1, 1, 1, 0.8)
	UiImageBox("ui/common/box-outline-4.png", 400, 44, 12, 12)
	UiColor(1, 1, 1, 1)
	UiFont("regular.ttf", 26)
	local text, active = UiTextInput(name, 400, 44, focusName)
	focusName = false
	if #text > 20 then text = string.sub(text, 1, 20) end
	if text ~= name then
		name = text
		ServerCall("server.menuAction", "name", name)
	end
	if active ~= typing then
		typing = active
		SetBool("level.menu.typing", typing)
	end
	if name == "" and not active then
		UiColor(1, 1, 1, 0.5)
		UiTranslate(10, 8)
		UiText("Name")
	end
	UiPop()
end
