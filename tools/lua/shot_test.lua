local from, at = Vec(0, 1.5, -6), Vec(0, 0.8, -22)
local dir = VecNormalize(VecSub(at, from))
local hit, dist, normal, shape = QueryRaycast(from, dir, 50)
local before = {}
for i, b in ipairs(FindBodies("mcptarget", true)) do before[i] = GetShapeVoxelCount(GetBodyShapes(b)[1]) end
Shoot(from, dir, "bullet", STRENGTH or 1)
return string.format("raycast hit=%s dist=%.2f shape=%s body=%s tag=%s", tostring(hit), dist or -1, tostring(shape), tostring(hit and GetShapeBody(shape)), tostring(hit and HasTag(GetShapeBody(shape), "mcptarget"))), table.concat(before, ",")
