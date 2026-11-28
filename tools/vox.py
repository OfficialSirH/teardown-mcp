"""Write (and read) MagicaVoxel .vox files for Teardown, with materials chosen by name.

In Teardown a voxel's material is decided by its palette index, not its colour (ranges measured
in 2.2.0, skill quirks.md "Materials and .vox files"). This module hands out palette indices from
the right range, so a model is built from materials:

    import vox
    v = vox.Vox()
    wood = v.color("wood", (120, 80, 40))           # a palette index in the wood range
    glow = v.color("plaster", (255, 220, 120), emit=1.0)  # emissive (MATL _emit)
    m = v.model("crate", (10, 10, 10))              # x, y, z in voxels; z is up in .vox
    m.box((0, 0, 0), (9, 9, 9), wood)               # inclusive corners
    m.set(5, 5, 9, glow)
    v.save("crate.vox")

Each model becomes a named object: `<vox file="MOD/crate.vox" object="crate"/>` loads one,
with its bottom centre on the body origin (`pos` moves it from there; a whole file loaded
without `object` is centred on all three axes instead: quirks.md). Axes: .vox (x, y, z-up)
becomes Teardown (x, y-up, -z). Voxel size is 0.1 m unless the `<vox>` has a `scale` (a scaled
vox makes its body static). Glass gets a MATL glass entry automatically (without one it is
opaque); `emit` makes a colour glow.

    python -I tools/vox.py FILE.vox     # print models, sizes, voxel counts, materials used
"""
import struct, sys

# Palette index ranges per material, measured in Teardown 2.2.0 with GetShapeMaterial.
MATERIALS = {
    "glass": (1, 8), "foliage": (9, 24), "dirt": (25, 40), "rock": (41, 56), "wood": (57, 72),
    "masonry": (73, 104), "plaster": (105, 120), "metal": (121, 136), "heavymetal": (137, 152),
    "plastic": (153, 168), "hardmetal": (169, 176), "hardmasonry": (177, 184), "ice": (185, 192),
    "none": (193, 224), "unphysical": (225, 240),
}


def material_of(index):
    for name, (a, b) in MATERIALS.items():
        if a <= index <= b:
            return name
    return "unphysical" if index == 254 else "none"


class Model:
    def __init__(self, name, size, pos=(0, 0, 0)):
        if not all(1 <= s <= 256 for s in size):
            raise ValueError(f"{name}: each side must be 1..256 voxels, got {size}")
        self.name, self.size, self.pos = name, tuple(size), tuple(pos)
        self.voxels = {}

    def set(self, x, y, z, index):
        sx, sy, sz = self.size
        if not (0 <= x < sx and 0 <= y < sy and 0 <= z < sz):
            raise ValueError(f"{self.name}: voxel {(x, y, z)} outside {self.size}")
        if index:
            self.voxels[(x, y, z)] = index
        else:
            self.voxels.pop((x, y, z), None)

    def box(self, a, b, index):
        for x in range(min(a[0], b[0]), max(a[0], b[0]) + 1):
            for y in range(min(a[1], b[1]), max(a[1], b[1]) + 1):
                for z in range(min(a[2], b[2]), max(a[2], b[2]) + 1):
                    self.set(x, y, z, index)


class Vox:
    def __init__(self):
        self.models = []
        self.palette = {}          # index -> (r, g, b, a)
        self.materials = {}        # index -> MATL dict (strings)
        self._next = {m: a for m, (a, b) in MATERIALS.items()}

    def model(self, name, size, pos=(0, 0, 0)):
        """A new named object. pos: its centre in the file's scene, in voxels (MagicaVoxel axes)."""
        m = Model(name, size, pos)
        self.models.append(m)
        return m

    def color(self, material, rgb, alpha=255, emit=None, matl=None):
        """Next free palette index of `material`, set to this colour. emit: 0..1 glow (MATL _emit);
        matl: extra MATL keys, e.g. {"_type": "_glass", "_trans": "0.5"}."""
        a, b = MATERIALS[material]
        i = self._next[material]
        if i > b:
            raise ValueError(f"no free palette slots left for {material} ({a}-{b})")
        self._next[material] = i + 1
        self.palette[i] = (rgb[0], rgb[1], rgb[2], alpha)
        d = dict(matl or {})
        if material == "glass" and not d:
            # Glass is see-through only with a MATL glass entry; palette alpha alone stays opaque
            # (measured 2026-11-28).
            d = {"_type": "_glass", "_trans": "0.5"}
        if emit is not None:
            d.update({"_type": "_emit", "_emit": f"{emit:g}", "_flux": "1"})
        if d:
            self.materials[i] = {k: str(v) for k, v in d.items()}
        return i

    # -- writing
    @staticmethod
    def _dict(d):
        out = struct.pack("<I", len(d))
        for k, v in d.items():
            kb, vb = k.encode(), v.encode()
            out += struct.pack("<I", len(kb)) + kb + struct.pack("<I", len(vb)) + vb
        return out

    @staticmethod
    def _chunk(cid, content, children=b""):
        return cid + struct.pack("<II", len(content), len(children)) + content + children

    def save(self, path):
        if not self.models:
            raise ValueError("no models")
        body = b""
        for m in self.models:
            body += self._chunk(b"SIZE", struct.pack("<III", *m.size))
            xyzi = struct.pack("<I", len(m.voxels)) + b"".join(
                struct.pack("<BBBB", x, y, z, i) for (x, y, z), i in sorted(m.voxels.items()))
            body += self._chunk(b"XYZI", xyzi)
        # Scene graph: root transform -> group -> (transform -> shape) per model, so each model
        # is a named object.
        n = len(self.models)
        body += self._chunk(b"nTRN", struct.pack("<I", 0) + self._dict({}) + struct.pack("<iiii", 1, -1, -1, 1) + self._dict({}))
        body += self._chunk(b"nGRP", struct.pack("<I", 1) + self._dict({}) + struct.pack("<I", n)
                            + b"".join(struct.pack("<i", 2 + 2 * k) for k in range(n)))
        for k, m in enumerate(self.models):
            t = f"{m.pos[0]} {m.pos[1]} {m.pos[2]}"
            body += self._chunk(b"nTRN", struct.pack("<I", 2 + 2 * k) + self._dict({"_name": m.name})
                                + struct.pack("<iiii", 3 + 2 * k, -1, 0, 1) + self._dict({"_t": t}))
            body += self._chunk(b"nSHP", struct.pack("<I", 3 + 2 * k) + self._dict({}) + struct.pack("<Ii", 1, k) + self._dict({}))
        pal = b""
        for i in range(1, 257):
            r, g, b, a = self.palette.get(i, (128, 128, 128, 255)) if i < 256 else (0, 0, 0, 0)
            pal += struct.pack("<BBBB", r, g, b, a)
        body += self._chunk(b"RGBA", pal)
        for i, d in sorted(self.materials.items()):
            body += self._chunk(b"MATL", struct.pack("<i", i) + self._dict(d))
        with open(path, "wb") as f:
            f.write(b"VOX " + struct.pack("<I", 150) + self._chunk(b"MAIN", b"", body))


def read(path):
    """Models of a .vox file: [(name, size, {index: count})], in file order."""
    d = open(path, "rb").read()
    assert d[:4] == b"VOX ", "not a .vox file"
    o, sizes, counts, names, shp_model, trn_child, trn_name = 20, [], [], {}, {}, {}, {}

    def rdict(p):
        n = struct.unpack_from("<I", d, p)[0]; p += 4; out = {}
        for _ in range(n):
            kl = struct.unpack_from("<I", d, p)[0]; k = d[p + 4:p + 4 + kl].decode(); p += 4 + kl
            vl = struct.unpack_from("<I", d, p)[0]; v = d[p + 4:p + 4 + vl].decode(); p += 4 + vl
            out[k] = v
        return out, p

    while o < len(d):
        cid = d[o:o + 4]; n, ch = struct.unpack_from("<II", d, o + 4); c = o + 12
        if cid == b"SIZE":
            sizes.append(struct.unpack_from("<III", d, c))
        elif cid == b"XYZI":
            k = struct.unpack_from("<I", d, c)[0]; cnt = {}
            for j in range(k):
                i = d[c + 4 + 4 * j + 3]; cnt[i] = cnt.get(i, 0) + 1
            counts.append(cnt)
        elif cid == b"nTRN":
            nid = struct.unpack_from("<I", d, c)[0]; attrs, p = rdict(c + 4)
            trn_child[nid] = struct.unpack_from("<i", d, p)[0]; trn_name[nid] = attrs.get("_name")
        elif cid == b"nSHP":
            nid = struct.unpack_from("<I", d, c)[0]; _, p = rdict(c + 4)
            shp_model[nid] = struct.unpack_from("<i", d, p + 4)[0]
        o = c + n + ch
    for t, child in trn_child.items():
        if child in shp_model and trn_name[t]:
            names[shp_model[child]] = trn_name[t]
    return [(names.get(k, f"model{k}"), sizes[k], counts[k]) for k in range(len(sizes))]


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1].startswith("-"):
        sys.exit(__doc__)
    for p in sys.argv[1:]:
        for name, size, cnt in read(p):
            mats = {}
            for i, c in cnt.items():
                mats[material_of(i)] = mats.get(material_of(i), 0) + c
            print(f"{name}: size {size}, {sum(cnt.values())} voxels, " + ", ".join(f"{m} {c}" for m, c in sorted(mats.items())))
