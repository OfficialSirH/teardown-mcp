-- Put the host player near a target and face it. Set TARGET (a Vec or an entity handle) and
-- optionally FROM (offset from the target, default 6 m back and up) before running. Server side.
local t = TARGET
if type(t) == "number" then
	local kind = GetEntityType(t)
	if kind == "vehicle" then t = GetVehicleTransform(t).pos
	elseif kind == "shape" then t = GetShapeWorldTransform(t).pos
	else t = GetBodyTransform(t).pos end
end
local from = FROM or Vec(-5, 2, 5)
local feet = VecAdd(t, from)
local eye = VecAdd(feet, Vec(0, 1.7, 0))
SetPlayerVelocity(Vec(0, 0, 0), 0)
SetPlayerTransformWithPitch(Transform(feet, QuatLookAt(eye, t)), 0)
return feet, t
