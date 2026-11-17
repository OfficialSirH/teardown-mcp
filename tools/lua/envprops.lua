local names = { "template", "skybox", "skyboxtint", "skyboxbrightness", "skyboxrot", "constant", "ambient", "ambientexponent", "fogColor", "fogcolor", "fogParams", "fogparams", "fogscale", "sunBrightness", "sunbrightness", "sunColorTint", "suncolortint", "sunDir", "sundir", "sunSpread", "sunspread", "sunLength", "sunlength", "sunFogScale", "sunfogscale", "sunGlare", "sunglare", "exposure", "brightness", "wetness", "puddleamount", "puddlesize", "rain", "nightlight", "ambience", "slippery", "waterhurt", "snowdir", "snowamount", "snowonground", "wind", "fogheight" }
local ok, bad = {}, {}
for _, n in ipairs(names) do
	local r = { pcall(GetEnvironmentProperty, n) }
	if r[1] then
		local vals = {}
		for i = 2, 6 do if r[i] ~= nil then vals[#vals + 1] = tostring(r[i]) end end
		ok[#ok + 1] = n .. "=" .. table.concat(vals, ",")
	else bad[#bad + 1] = n end
end
local pp, ppbad = {}, {}
for _, n in ipairs({ "saturation", "colorbalance", "brightness", "gamma", "bloom", "contrast", "vignette", "exposure" }) do
	local r = { pcall(GetPostProcessingProperty, n) }
	if r[1] then pp[#pp + 1] = n .. "=" .. tostring(r[2]) .. (r[3] and ("," .. tostring(r[3]) .. "," .. tostring(r[4])) or "") else ppbad[#ppbad + 1] = n end
end
return table.concat(ok, " | "), "unknown: " .. table.concat(bad, " "), table.concat(pp, " | "), "unknown: " .. table.concat(ppbad, " ")
