local P = "savegame.mod.mcp_"
local keys = {}
for _, k in ipairs(ListKeys("savegame.mod") or {}) do
	if string.sub(k, 1, 4) == "mcp_" and not string.match(k, "^mcp_r%d+_%d+$") then keys[#keys + 1] = k .. "=" .. GetString("savegame.mod." .. k) end
end
return table.concat(keys, "  "), GetTime(), GetString("savegame.mod.mcpprobe_at")
