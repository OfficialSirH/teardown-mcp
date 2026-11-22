#version 2
-- Probe: cross-mod #include, form "base". Spawn from the bridge (run_lua, server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeinc_base.lua"/>', Transform())
-- Writes level.probeinc.base (server half) and level.probeinc.basec (client half, not synced)
-- when the script loaded; nothing when the include failed (the whole script does not load).
#include "target_missing_control.lua"
function server.init() SetString("level.probeinc.base", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or ""), true) end
function client.init() SetString("level.probeinc.basec", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or "")) end
