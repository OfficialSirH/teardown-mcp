#version 2
-- Probe: cross-mod #include, form "mod". Spawn from the bridge (run_lua, server):
--   Spawn('<script file="MOD/../mcp-playground/script/probeinc_mod.lua"/>', Transform())
-- Writes level.probeinc.mod (server half) and level.probeinc.modc (client half, not synced)
-- when the script loaded; nothing when the include failed (the whole script does not load).
#include "MOD/../teardown-screens/tests/probe/target.lua"
function server.init() SetString("level.probeinc.mod", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or ""), true) end
function client.init() SetString("level.probeinc.modc", tostring(PROBE_TARGET or PROBE_OUTER) .. " inner=" .. tostring(PROBE_INNER) .. " " .. (probeWhichMod and probeWhichMod() or "")) end
