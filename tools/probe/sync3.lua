-- server again: what the clients left
local out = {}
local function add(k, v) out[#out + 1] = k .. "=" .. tostring(v) end
for _, who in ipairs({ "host", "remote" }) do
	add("level.cli_" .. who, GetString("level.mcpsync.cli_" .. who))
	add("level.clisync_" .. who, GetString("level.mcpsync.clisync_" .. who))
	add("client-spawned body of " .. who .. " seen by server", FindBody("mcpsync_" .. who, true))
end
add("shared.fromclient", shared.fromclient)
return table.concat(out, "\n")
