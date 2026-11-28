"""Read and write Teardown animation files (`<animator file="...">`, the `animdata.bin` that
VoxTool produces), so a skeleton and its clips can be made by a script instead of VoxTool.

Format ("TD_ANIM" version 100, little-endian), worked out from the game's own files (all 20
that ship with 2.2.0 parse to their last byte):

    "TD_ANIM\\0"  u32 version (100)  u32 bone count
    per bone:  name\\0  i32 parent index (-1 for the root)  f32 pos[3]  f32 quat[4] (x y z w)
               -- the T-pose, local to the parent bone
    u32 clip count
    per clip:  name\\0  u32 flags (0)  f32 frames per second  u32 track count
      per track:  u32 bone index  u32 key count
        per key:  f32 pos[3]  f32 quat[4]  f32 time (seconds)   -- local to the parent bone

Usage:

    import td_anim
    bones = [("root", -1, (0, 0, 0), (0, 0, 0, 1)), ("arm", 0, (0, 0.4, 0), (0, 0, 0, 1))]
    keys = [((0, 0.4, 0), td_anim.quat_axis_angle((0, 0, 1), 30 * math.sin(t)), t) for t in ...]
    td_anim.write("anim.bin", bones, [("wave", 30.0, {1: keys})])

Bones without a track in a clip get one holding their T-pose (a clip must have a track for
every bone, or the game crashes when it plays). The prefab that uses the file needs a body
per bone, named like it and tagged `bone=<name>` (see the skill's authoring.md, "Animated
models").

    python -I tools/td_anim.py FILE.bin   # print bones and clips
"""
import math, struct, sys


def quat_axis_angle(axis, degrees):
    """Quaternion (x, y, z, w) for a rotation about axis by degrees (like the API's QuatAxisAngle)."""
    l = math.sqrt(sum(a * a for a in axis)) or 1.0
    h = math.radians(degrees) / 2
    s = math.sin(h) / l
    return (axis[0] * s, axis[1] * s, axis[2] * s, math.cos(h))


def write(path, bones, clips, version=100):
    """bones: [(name, parent_index, pos3, quat4)]; clips: [(name, fps, {bone_index: [(pos3, quat4, time)]})].

    Every clip gets a track for every bone: a bone without one is given its T-pose at the times
    of the clip's other keys. The game's own files always have all tracks, and a clip missing
    some crashed the game (access violation) as soon as it played (measured 2026-11-28)."""
    out = b"TD_ANIM\0" + struct.pack("<II", version, len(bones))
    for name, parent, pos, quat in bones:
        out += name.encode() + b"\0" + struct.pack("<i3f4f", parent, *pos, *quat)
    out += struct.pack("<I", len(clips))
    for name, fps, tracks in clips:
        tracks = dict(tracks)
        times = sorted({k[2] for keys in tracks.values() for k in keys}) or [0.0]
        for bi, (_, _, pos, quat) in enumerate(bones):
            if bi not in tracks:
                tracks[bi] = [(pos, quat, t) for t in times]
        out += name.encode() + b"\0" + struct.pack("<IfI", 0, fps, len(tracks))
        for bi, keys in sorted(tracks.items()):
            out += struct.pack("<II", bi, len(keys))
            for pos, quat, t in keys:
                out += struct.pack("<3f4ff", *pos, *quat, t)
    with open(path, "wb") as f:
        f.write(out)


def read(path):
    """(version, bones, clips) in the shapes write() takes."""
    d = open(path, "rb").read()
    if d[:8] != b"TD_ANIM\0":
        raise ValueError("not a TD_ANIM file")
    ver, nb = struct.unpack_from("<II", d, 8)
    o = 16

    def cstr():
        nonlocal o
        e = d.index(b"\0", o)
        s = d[o:e].decode()
        o = e + 1
        return s

    bones = []
    for _ in range(nb):
        name = cstr()
        v = struct.unpack_from("<i7f", d, o)
        o += 32
        bones.append((name, v[0], v[1:4], v[4:8]))
    clips = []
    nclips = struct.unpack_from("<I", d, o)[0]
    o += 4
    for _ in range(nclips):
        name = cstr()
        _, fps, ntr = struct.unpack_from("<IfI", d, o)
        o += 12
        tracks = {}
        for _ in range(ntr):
            bi, nk = struct.unpack_from("<II", d, o)
            o += 8
            keys = []
            for k in range(nk):
                v = struct.unpack_from("<8f", d, o + 32 * k)
                keys.append((v[0:3], v[3:7], v[7]))
            o += 32 * nk
            tracks[bi] = keys
        clips.append((name, fps, tracks))
    if o != len(d):
        raise ValueError(f"parsed {o} of {len(d)} bytes")
    return ver, bones, clips


if __name__ == "__main__":
    if len(sys.argv) < 2 or sys.argv[1].startswith("-"):
        sys.exit(__doc__)
    for p in sys.argv[1:]:
        ver, bones, clips = read(p)
        print(f"{p}: version {ver}, {len(bones)} bones, {len(clips)} clips")
        print("  bones:", ", ".join(f"{n}<{bones[pi][0] if pi >= 0 else '-'}" for n, pi, _, _ in bones))
        for name, fps, tracks in clips:
            length = max((k[-1][2] for k in tracks.values() if k), default=0)
            print(f"  {name}: {fps:g} fps, {len(tracks)} tracks, {length:.2f} s")
