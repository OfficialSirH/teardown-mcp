-- server: set up things for the other sides to look at
SetString("level.mcpsync.plain", "srv-plain")
SetString("level.mcpsync.synced", "srv-synced", true)
SetInt("level.mcpsync.syncint", 7, true)
SetString("savegame.mod.mcpsync", "srv-save")
shared.mcpsync = { text = "hello", list = { 1, 2, 3 } }
local hs = Spawn('<body tags="mcpsync_srv" dynamic="false"><voxbox size="2 2 2"/></body>', Transform(Vec(0, 45, 0)))
SetTag(hs[1], "note", "from-server")
return "server set up", hs[1]
