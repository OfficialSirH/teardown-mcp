-- Probe voxscript (no #version: voxscripts are their own kind of script). Two instances are
-- in the playground's main.xml; tools/lua/voxscript_probe.lua reads what they made.
-- Parameters are read at file level, as the game's own voxscripts do.
size = GetInt("size", 10)
mat = GetString("mat", "wood", "material type")

function init()
	local m1 = CreateMaterial(mat, 0.8, 0.3, 0.2)
	local m2 = CreateMaterial("rock", 0.4, 0.4, 0.45)

	-- Shape 1: a box of size^3 from (0,0,0) to (size,size,size), hollowed out by Material(0).
	Vox(0, 0, 0)
	Material(m1)
	Box(0, 0, 0, size, size, size)
	Material(0)
	Box(2, 2, 2, size - 2, size - 2, size - 2)

	-- Shape 2: a second Vox 30 units along x (voxels or metres?), a 4^3 rock box and one
	-- single voxel set above it.
	Vox(30, 0, 0)
	Material(m2)
	Box(0, 0, 0, 4, 4, 4)
	Set(0, 10, 0)
end
