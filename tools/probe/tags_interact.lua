-- Tag probe: run with side=client after tags_a.lua placed the host in front of the box tagged
-- interact=Probe (and side=remote for the joined client's view of its own player).
local box = GetInt("level.probetag.interact")
return {
	box = box,
	interact_shape = GetPlayerInteractShape(),
	interact_body = GetPlayerInteractBody(),
	caninteract = GetString("game.player.caninteract"),
}
