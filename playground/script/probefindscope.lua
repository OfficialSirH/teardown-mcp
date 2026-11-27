#version 2
-- probe (2026-11-26): what do non-global Find* calls return in a v2 script whose prefab nests entities under it?
function server.tick()
	if done then return end
	done = true
	SetString("level.mcpscope", string.format(
		"FindJoints() %d (all %d) | FindBodies('') %d (all %d) | FindShapes('') %d | FindVehicles('') %d | FindShapes('scopetag') %d (all %d)",
		#FindJoints(), #FindJoints("", true), #FindBodies(""), #FindBodies("", true), #FindShapes(""),
		#FindVehicles(""), #FindShapes("scopetag"), #FindShapes("scopetag", true)))
end
