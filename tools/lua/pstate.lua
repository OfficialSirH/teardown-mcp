local t = GetPlayerTransform(1)
local cam = GetPlayerCameraTransform(1)
local fwd = TransformToParentVec(cam, Vec(0, 0, -1))
return string.format("pos=%.2f,%.2f,%.2f yaw=%.0f pitch=%.0f tool=%s shots=%d menu_open=%s clicks=%d last=%s slider=%.2f dusk=%s",
	t.pos[1], t.pos[2], t.pos[3], math.deg(math.atan2(fwd[1], -fwd[3])), math.deg(math.asin(fwd[2])), GetPlayerTool(1), GetInt("level.blaster.shots"),
	tostring(GetBool("level.menu.open")), GetInt("level.menu.clicks"), GetString("level.menu.last"), GetFloat("level.menu.slider"), tostring(GetBool("level.director.dusk")))
