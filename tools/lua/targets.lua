local out = {}
for i, b in ipairs(FindBodies("mcptarget", true)) do
	local shapes = GetBodyShapes(b)
	local n = 0
	for _, s in ipairs(shapes) do n = n + GetShapeVoxelCount(s) end
	local p = GetBodyTransform(b).pos
	out[#out + 1] = string.format("body %d valid=%s broken=%s shapes=%d voxels=%d pos=%.1f,%.1f,%.1f", b, tostring(IsHandleValid(b)), tostring(IsBodyBroken(b)), #shapes, n, p[1], p[2], p[3])
end
return table.concat(out, "\n"), GetInt("level.range.score"), GetInt("level.range.shots")
