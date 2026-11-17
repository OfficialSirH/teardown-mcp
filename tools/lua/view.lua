-- Stand at FEET (Vec) and look at AT (Vec). Server side.
SetPlayerVelocity(Vec(0, 0, 0), 0)
SetPlayerTransformWithPitch(Transform(FEET, QuatLookAt(VecAdd(FEET, Vec(0, 1.7, 0)), AT)), 0)
return "ok"
