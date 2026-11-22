#version 2
-- Probe: cross-mod #include, form "id". Spawn from the bridge (run_lua, server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeinc_id.lua"/>', Transform())
-- Writes level.probeinc.id (server half) and level.probeinc.idc (client half, not synced)
-- when the script loaded; nothing when the include failed (the whole script does not load).
#include "local-teardown-screens:tests/probe/target.lua"
function server.init() SetString("level.probeinc.id", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or ""), true) end
function client.init() SetString("level.probeinc.idc", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or "")) end
