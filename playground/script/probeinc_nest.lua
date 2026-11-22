#version 2
-- Probe: cross-mod #include, form "nest". Spawn from the bridge (run_lua, server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeinc_nest.lua"/>', Transform())
-- Writes level.probeinc.nest (server half) and level.probeinc.nestc (client half, not synced)
-- when the script loaded; nothing when the include failed (the whole script does not load).
#include "../../teardown-screens/tests/probe/outer.lua"
function server.init() SetString("level.probeinc.nest", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or ""), true) end
function client.init() SetString("level.probeinc.nestc", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or "")) end
