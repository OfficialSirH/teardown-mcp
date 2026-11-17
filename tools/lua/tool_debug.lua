local b = GetToolBody(1)
local out = { "toolbody=" .. tostring(b), GetString("level.blaster.last") }
if b ~= 0 then
	local t = GetBodyTransform(b)
	local cam = GetPlayerCameraTransform(1)
	out[#out + 1] = string.format("body pos rel. camera = %s", VecStr(TransformToLocalPoint(cam, t.pos)))
	for i, s in ipairs(GetBodyShapes(b)) do
		local x, y, z, scale = GetShapeSize(s)
		local mi, ma = GetShapeBounds(s)
		out[#out + 1] = string.format("shape %d: %dx%dx%d voxels, voxel scale %.3f, bounds size %s", i, x, y, z, scale, VecStr(VecSub(ma, mi)))
	end
end
local m = GetToolLocationWorldTransform("muzzle", 1)
out[#out + 1] = "muzzle=" .. (m and VecStr(TransformToLocalPoint(GetPlayerCameraTransform(1), m.pos)) or "nil")
return table.concat(out, "\n")
