"""Build allcalls.lua: calls (almost) every documented API function once with no arguments.

The point is not the result of each call. A function that is not allowed on this side makes
the engine log "<fn> is not available in <side> script" (and stop this half of the script
after the callback), so one run per side followed by a look at the log gives the complete
side-restriction list. Functions that raise "only for internal usage" are returned.
"""
import json, os, re
HERE = os.path.dirname(os.path.abspath(__file__))
funcs = [n for n, _ in json.load(open(os.path.join(HERE, "defs_functions.json")))]
SKIP_RE = re.compile(r"^(NetSession|Activity|Ui|TSharedUserdata)|CPP$")
SKIP = set("""Restart StartLevel ResumeLevel SetPaused Menu Command ClearKey ClearQuickSave KillClient RejoinClient
RespawnPlayer RespawnPlayerAtTransform CompleteAchievement IndicateAchievementProgress SetPresence
PresenceOpenInviteOverlay SetClipboardText LoadLanguageTable SetSaveGamePlayerCharacter SetEnvironmentDefault
SetPostProcessingDefault DisableAllLights Delete DeleteShape ProhibitScreenRecord PermitScreenRecord
SetPlayerCharacter InputClear InputResetOnTransition GetEvent SetTimeScale SetGravity PlayMusic StopMusic
DisablePlayer DisablePlayerInput SetPlayerHidden""".split())
names = [n for n in funcs if not SKIP_RE.search(n) and n not in SKIP]
skipped = [n for n in funcs if n not in names]
lua = ["local names = {" + ",".join(f'"{n}"' for n in names) + "}",
       "local internal, missing, other = {}, {}, 0",
       "for _, n in ipairs(names) do",
       "\tlocal f = _G[n]",
       '\tif type(f) ~= "function" then missing[#missing + 1] = n else',
       "\t\tlocal ok, err = pcall(f)",
       '\t\tif not ok and string.find(tostring(err), "internal usage", 1, true) then internal[#internal + 1] = n',
       "\t\telseif not ok then other = other + 1 end",
       "\tend",
       "end",
       'return { internal = table.concat(internal, " "), missing = table.concat(missing, " "), errors = other, called = #names }']
open(os.path.join(HERE, "allcalls.lua"), "w", newline="\n").write("\n".join(lua) + "\n")
json.dump(skipped, open(os.path.join(HERE, "allcalls_skipped.json"), "w"))
print(len(names), "called,", len(skipped), "skipped")
