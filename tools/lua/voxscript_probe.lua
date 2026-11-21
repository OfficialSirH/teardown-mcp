-- Reads what the playground's probe voxscripts made: every shape in the area x -60..0,
-- z -40..-20, with body, voxel count, size, position, material at its first solid voxel and
-- the tags it carries. Run on server, client and remote to compare.
QueryRequire("")
local shapes = QueryAabbShapes(Vec(-60, -1, -40), Vec(0, 10, -20))
local out = {}
for _, s in ipairs(shapes) do
	local b = GetShapeBody(s)
	local t = GetShapeWorldTransform(s)
	local x, y, z, scale = GetShapeSize(s)
	local lo, hi = GetShapeBounds(s)
	local mat = GetShapeMaterialAtPosition(s, VecLerp(lo, hi, 0.05))
	local tags = {}
	for _, tg in ipairs(ListTags and ListTags(s) or {}) do tags[#tags + 1] = tg end
	out[#out + 1] = string.format("shape %d body %d%s voxels %d size %dx%dx%d scale %.2f pos %.2f %.2f %.2f bounds %.2f..%.2f %.2f..%.2f %.2f..%.2f mat %s tags [%s] bodytags [%s]",
		s, b, b == GetWorldBody() and " (world)" or "", GetShapeVoxelCount(s), x, y, z, scale or 0,
		t.pos[1], t.pos[2], t.pos[3], lo[1], hi[1], lo[2], hi[2], lo[3], hi[3], tostring(mat),
		table.concat(tags, " "), table.concat(ListTags and ListTags(b) or {}, " "))
end
return { count = #shapes, shapes = out, tagged = #FindShapes("mcpvox", true), tagged2 = #FindShapes("mcpvox2", true) }
