-- Game mode state as this side sees it: the running mode, every listed mode with its
-- path / layers / mission, and the marks the playground's probe mode writes
-- (game.mcpprobemode.<half>.init / ticks / destroy).
local modes = {}
for _, m in ipairs(ListKeys("gamemodes")) do
	for _, g in ipairs(ListKeys("gamemodes." .. m)) do
		local k = "gamemodes." .. m .. "." .. g
		local fields = {}
		for _, f in ipairs(ListKeys(k)) do fields[f] = GetString(k .. "." .. f) end
		modes[m .. "." .. g] = fields
	end
end
local probe = {}
for _, half in ipairs({ "server", "client" }) do
	local k = "game.mcpprobemode." .. half
	probe[half] = {}
	for _, f in ipairs(ListKeys(k)) do probe[half][f] = GetString(k .. "." .. f) end
end
return {
	gamemode = GetString("game.gamemode"),
	gamemode_next = GetString("game.gamemode.next"),
	modes = modes,
	probe = probe,
	scripts = #FindEntities("", true, "script"),
}
