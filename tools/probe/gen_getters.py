"""Build getters.lua: calls every read-only API function with plausible arguments and reports,
per function, whether the call worked on this side and what came back.

Run it in the game on both halves (server, client, draw) and compare.
"""
import json
import os
import re

HERE = os.path.dirname(__file__)
funcs = json.load(open(os.path.join(HERE, "defs_functions.json")))

READ = re.compile(r"^(Get|Is|Has|Find|Query|List|Ui(Get|Is|Has|Width|Height|Center|Middle|Measure|WorldToPixel|PixelToWorld))")
SKIP = {
    # need a set-up the probe does not have, or block / change state
    "GetEvent", "QueryPath", "GetPathState", "GetPathLength", "GetPathPoint", "IsRunningOnPlaystation",
}

HANDLE_BY_NAME = [
    ("Vehicle", "VEH"), ("Wheel", "WHEEL"), ("Shape", "SHAPE"), ("Body", "BODY"), ("Light", "LIGHT"),
    ("Joint", "JOINT"), ("Rope", "JOINT"), ("Trigger", "TRIG"), ("Location", "LOC"), ("Screen", "SCREEN"),
    ("Water", "WATER"), ("Animator", "0"),
]


def arg_for(fn, param):
    p = param.strip().lower().lstrip("_").rstrip("_")
    if not p or p == "...":
        return None
    if "player" in p:
        return "0"
    for word, expr in [("vehicle", "VEH"), ("wheel", "WHEEL"), ("shape", "SHAPE"), ("body", "BODY"),
                       ("light", "LIGHT"), ("joint", "JOINT"), ("trigger", "TRIG"), ("location", "LOC"),
                       ("screen", "SCREEN"), ("water", "WATER"), ("animator", "0")]:
        if word in p:
            return expr
    if p in ("handle", "entity", "h", "parent", "child"):
        for word, expr in HANDLE_BY_NAME:
            if word in fn:
                return expr
        return "BODY"
    if p in ("pos", "origin", "point", "min", "max", "a", "b", "vec", "v", "center", "velocity", "p",
             "position", "start", "end", "target", "worldpos", "mi", "ma", "c"):
        return "Vec(0, 5, 0)"
    if p in ("dir", "direction", "normal", "axis"):
        return "Vec(0, -1, 0)"
    if p in ("transform", "t", "tr"):
        return "Transform()"
    if p in ("rot", "quat", "q"):
        return "Quat()"
    if p == "key" or p == "parent_key":
        return '"game.levelpath"'
    if p == "tag":
        return '""'
    if p in ("path", "file", "filepath"):
        return '"MOD/main.lua"'
    if p in ("name", "id", "tool", "toolid", "str", "text", "string", "type", "input", "action", "button",
             "command", "eventname", "property", "bone", "bonename", "s", "message", "layer"):
        return '"sledge"'
    if p in ("global", "enabled", "value", "includefriction", "rejecttransparent"):
        return "true"
    return "1"


rows = []
for name, params in funcs:
    if "." in name or not READ.match(name) or name in SKIP:
        continue
    args = [arg_for(name, x) for x in params.split(",")] if params.strip() else []
    args = [a for a in args if a is not None]
    rows.append((name, ", ".join(args)))

lua = ["local function first(list) return (list and list[1]) or 0 end",
       'local BODY = first(FindBodies("", true))',
       'local SHAPE = first(FindShapes("", true))',
       'local VEH = first(FindVehicles("", true))',
       'local LIGHT = first(FindLights("", true))',
       'local JOINT = first(FindJoints("", true))',
       'local TRIG = first(FindTriggers("", true))',
       'local LOC = first(FindLocations("", true))',
       'local SCREEN = first(FindScreens("", true))',
       'local WATER = first(FindEntities("", true, "water"))',
       "local WHEEL = (VEH ~= 0 and GetVehicleWheels and first(GetVehicleWheels(VEH))) or 0",
       "local function kind(v)",
       "\tlocal t = type(v)",
       '\tif t == "table" then',
       '\t\tif v.pos and v.rot then return "transform" end',
       '\t\tif #v == 3 and type(v[1]) == "number" then return "vec" end',
       '\t\tif #v == 4 and type(v[1]) == "number" then return "quat" end',
       '\t\treturn "table#" .. #v',
       "\tend",
       '\tif t == "string" then return "string" end',
       "\treturn t",
       "end",
       "local out = {}",
       "local function probe(name, fn)",
       '\tif type(_G[name]) ~= "function" then out[#out + 1] = name .. "|missing" return end',
       "\tlocal r = { pcall(fn) }",
       "\tif not r[1] then",
       '\t\tout[#out + 1] = name .. "|err|" .. tostring(r[2])',
       "\t\treturn",
       "\tend",
       "\tlocal n = 0",
       "\tfor k in pairs(r) do if k > n then n = k end end",
       "\tlocal kinds = {}",
       "\tfor i = 2, n do kinds[#kinds + 1] = kind(r[i]) end",
       '\tout[#out + 1] = name .. "|ok|" .. table.concat(kinds, ",")',
       "end",
       'out[#out + 1] = "handles|" .. table.concat({ BODY, SHAPE, VEH, LIGHT, JOINT, TRIG, LOC, SCREEN, WATER, WHEEL }, ",")']
for name, args in rows:
    lua.append(f'probe("{name}", function() return {name}({args}) end)')
lua.append('return table.concat(out, "\\n")')
open(os.path.join(HERE, "getters.lua"), "w", newline="\n").write("\n".join(lua) + "\n")
print(len(rows), "functions")
