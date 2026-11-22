#version 2
-- Probe: cross-mod #include, form "rel". Spawn from the bridge (run_lua, server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeinc_rel.lua"/>', Transform())
-- Writes level.probeinc.rel (server half) and level.probeinc.relc (client half, not synced)
-- when the script loaded; nothing when the include failed (the whole script does not load).
#include "../../teardown-screens/tests/probe/target.lua"
function server.init() SetString("level.probeinc.rel", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or ""), true) end
function client.init() SetString("level.probeinc.relc", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or "")) end
