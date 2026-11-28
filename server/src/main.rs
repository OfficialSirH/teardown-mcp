//! Teardown MCP server (stdio). Drives a running Teardown through the "MCP Bridge" mod.

mod bridge;
mod mptest;
#[cfg(windows)]
mod diag;
mod exe_names;
mod name_tables;
#[cfg(windows)]
mod input;
#[cfg(windows)]
mod shot;

use bridge::{Bridge, Cmd, Timeouts};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

const INSTRUCTIONS: &str = "Drives a running Teardown game through the in-game \"MCP Bridge\" mod, for developing Teardown mods. \
The game must be inside a level with the mod enabled. Mod scripts do not run in the main menu, and in the single-player pause menu only side=client commands run (any other side answers at once that the game is paused; run_lua side=client with SetPaused(false) resumes it). Do not quit to the menu or load a level path you are not sure of. The game may have been closed since your last call: after 5 minutes without an answer the server first checks the game windows and says at once if none is open (then ask the user or use mp_test_start; do not retry). If the game stops answering, take a screenshot; in the pause menu, send_input can click Continue, otherwise ask the user. \
After editing another mod's scripts, XML or assets, call reload_scene: it restarts the level and returns only once the new scene is running (this can take minutes for heavy mods). \
Scripts are #version 2: the host has a server half and a client half with separate Lua globals; pick the half with `side`. \
World changes (spawn, delete, moving bodies, player health) belong on the server; Ui* and local-player camera/input belong on the client. \
run_lua runs in the bridge mod's own script, so it sees the engine API and the registry but not other scripts' Lua globals. \
Lua runtime errors of other mods show as red text on screen: use screenshot to see them.";

fn send(msg: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", msg);
    let _ = out.flush();
}

#[cfg(windows)]
fn host_test_window() -> bool {
    shot::window_exists("TD - Host")
}

#[cfg(not(windows))]
fn host_test_window() -> bool {
    false
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": { "type": "object", "properties": props, "required": required }
    })
}

fn tools() -> Value {
    let side = json!({"type": "string", "enum": ["server", "client", "draw", "remote"], "description": "Where it runs (default server). server / client: the host game's two script halves. draw: the host's client.draw callback, the only place Ui* functions exist. remote: the joined client's game (local multiplayer test only), to see what a non-host player's game sees."});
    let vec3 = |d: &str| json!({"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": d});
    let rot = json!({"type": "array", "items": {"type": "number"}, "description": "Rotation: quaternion [x,y,z,w] or Euler degrees [x,y,z]."});
    let timeout = json!({"type": "number", "description": "Seconds to wait for the new scene (default 1800; heavy mods can take minutes)."});
    let player = json!({"type": "integer", "description": "Player id (default 0: the host on the server, the local player on the client)."});
    json!([
        tool("ping", "Check that the game answers. Returns game version, level path, scene counter, the players (on the server side each with `ready`: that player's own game has loaded the level and runs its scripts), `state` (\"PLAY\" in a level), `paused` (a pause menu is open; in the multiplayer test nothing stops) and `paused_sp` (single-player pause: only side=client commands run).", json!({"side": side}), &[]),
        tool("bridge_status", "Bridge state without talking to the game: file paths and the mod's saved status keys.", json!({}), &[]),
        tool("run_lua", "Run Lua code in the game and return its return values and anything it print()s. An expression works too (\"GetPlayerHealth()\"). Full Teardown API available.",
            json!({"code": {"type": "string"}, "file": {"type": "string", "description": "Absolute path of a Lua file to run instead of `code` (forward slashes). Use it for anything longer than a few lines."}, "side": side}), &[]),
        tool("call_api", "Call one Teardown API function by name with JSON arguments, e.g. fn=\"SetPlayerHealth\", args=[1].",
            json!({"fn": {"type": "string"}, "args": {"type": "array"}, "side": side}), &["fn"]),
        tool("reload_scene", "Restart the current level so changes to mods on disk take effect, then wait until the new scene is running.",
            json!({"wait": {"type": "boolean", "description": "Wait for the new scene (default true)."}, "timeout_secs": timeout,
                   "side": {"type": "string", "enum": ["server", "client"], "description": "Half that calls Restart() (default server). Use client when the server half has stopped answering."}}), &[]),
        tool("start_level", "Load another level via StartLevel(mission, path, layers) and wait until it is running. RISKY: a path the engine cannot load drops the game to the main menu, where the bridge stops working until the user loads a level by hand. Built-in level paths are taken relative to data/level/ (\"data/level/hub.xml\" failed in testing). Prefer reload_scene; use this only with a path known to work.",
            json!({"path": {"type": "string"}, "mission": {"type": "string", "description": "Mission/level id (default empty)."}, "layers": {"type": "string", "description": "Space-separated layers to enable."},
                   "wait": {"type": "boolean"}, "timeout_secs": timeout}), &["path"]),
        tool("game_command", "Run an engine command via Command(), e.g. \"game.quicksave\", \"game.quickload\". Set wait_scene only if the command reloads the scene and its scripts.",
            json!({"command": {"type": "string"}, "args": {"type": "array"}, "wait_scene": {"type": "boolean"}, "timeout_secs": timeout, "side": side}), &["command"]),
        tool("batch", "Run several bridge actions in order in one go. Actions: ping, run_lua, call, restart, start_level, game_command, registry_get, registry_set, registry_clear, player_get, player_set, look_at, spawn, delete, find, describe, raycast, debug_print, perf (side client: frame counter; args.since = an earlier frame for frame-time stats), scene_stats (entity counts). Commands after a restart/start_level run in the new scene.",
            json!({"commands": {"type": "array", "items": {"type": "object", "properties": {
                        "action": {"type": "string"}, "args": {"type": "object"}, "side": side,
                        "wait_scene": {"type": "boolean", "description": "Wait for a new scene after this command (default true for restart and start_level)."}},
                        "required": ["action"]}},
                   "timeout_secs": timeout}), &["commands"]),
        tool("registry_get", "Read a registry key and its children (e.g. \"game\", \"level\", \"options\"). \"savegame.mod\" is the bridge mod's own save data, not another mod's.",
            json!({"key": {"type": "string"}, "depth": {"type": "integer", "description": "Levels of children to include (default 1)."}, "max_keys": {"type": "integer"}, "side": side}), &["key"]),
        tool("registry_set", "Write a registry key.",
            json!({"key": {"type": "string"}, "value": {}, "type": {"type": "string", "enum": ["string", "int", "float", "bool"]}, "sync": {"type": "boolean", "description": "Sync to clients (server side only)."}, "side": side}), &["key", "value"]),
        tool("registry_clear", "Delete a registry key and its children.", json!({"key": {"type": "string"}, "side": side}), &["key"]),
        tool("player_get", "Player transform, camera, velocity, health, vehicle and tool. For the game's own local player (player 0 or that id) also `local_state`: the engine's game.player.* keys (canusetool, cangrab, caninteract, grabbing, disableinput, interactive, usevehicle, usescreen, tool, character, ...), handy to see why an input test does nothing.", json!({"player": player, "side": side}), &[]),
        tool("player_set", "Teleport a player and/or set velocity and health.",
            json!({"pos": vec3("World position."), "rot": rot, "velocity": vec3("Velocity."), "health": {"type": "number", "description": "0..1"}, "player": player, "side": side}), &[]),
        tool("spawn", "Spawn a prefab XML file or an XML string at a position. Returns the new entity handles.",
            json!({"xml": {"type": "string", "description": "Prefab path or XML content."}, "pos": vec3("World position."), "rot": rot,
                   "allow_static": {"type": "boolean"}, "joint_existing": {"type": "boolean"}, "side": side}), &["xml"]),
        tool("delete_entities", "Delete entities by handle.", json!({"handles": {"type": "array", "items": {"type": "integer"}}, "side": side}), &["handles"]),
        tool("find_entities", "List entities in the scene by tag and/or type (body, shape, vehicle, light, location, joint, trigger, screen, ...).",
            json!({"tag": {"type": "string"}, "type": {"type": "string"}, "limit": {"type": "integer", "description": "Default 100."}, "side": side}), &[]),
        tool("describe_entity", "Details of one entity: type, tags, transform, and type-specific data.", json!({"handle": {"type": "integer"}, "side": side}), &["handle"]),
        tool("raycast", "Cast a ray. Without origin and dir it goes from the player's camera forward.",
            json!({"origin": vec3("Ray start."), "dir": vec3("Ray direction."), "max_dist": {"type": "number"}, "radius": {"type": "number"}, "player": player, "side": side}), &[]),
        tool("look_at", "Teleport a player to a spot from which they see a target, facing it: use it before screenshot. target is a world position [x,y,z] or an entity handle; from is the offset of the player's feet from the target (default [-5, 2, 5]). A player in mid-air falls, so keep the offset near ground level or take the screenshot right away.",
            json!({"target": {"description": "World position [x,y,z] or entity handle."}, "from": vec3("Offset of the player from the target."), "player": player}), &["target"]),
        tool("debug_print", "Print a line in the game's on-screen debug console.", json!({"text": {"type": "string"}, "side": side}), &["text"]),
        tool("mp_test_start", "Start Teardown's local multiplayer test: two game windows on this PC (\"TD - Host\" and \"TD - Client1\") running the given mod's level, with the MCP Bridge mod loaded, then wait until the host answers and the client's player is in the level. Use this to test a mod in multiplayer; commands then go to the host window.",
            json!({"mod": {"type": "string", "description": "Mod to start: its id (\"local-my-mod\", \"steam-<workshop id>\", \"builtin-<name>\") or its folder name under Documents/Teardown/mods (\"My Mod\")."},
                   "level": {"type": "string", "description": "Level XML, relative to the mod folder or a full path (default main.xml)."},
                   "extra_mods": {"type": "array", "items": {"type": "string"}, "description": "Ids of further global mods to load besides the bridge, e.g. [\"builtin-mpclassics\"] for its game modes."},
                   "game_mode": {"type": "string", "description": "Start with this game mode, as the mod manager's game mode choice does: \"<mod id>.<key>\", the key being the gamemodes.txt section name in lower case with spaces as '-' (\"builtin-mpclassics.deathmatch\", \"local-my-mod.my-mode\"). A content mode's `layers` are read from its mod's gamemodes.txt; a global mode needs its mod in `extra_mods` (default none)."},
                   "layers": {"type": "string", "description": "Level layers to enable. Default: the game_mode's `layers` from gamemodes.txt, else none."},
                   "players": {"type": "integer", "enum": [1, 2], "description": "2 (default): host and client windows. 1: the host window alone, still a multiplayer session (server and client halves, multiplayer rules), quicker to start and lighter."},
                   "restart": {"type": "boolean", "description": "Close a multiplayer test that is already running and start a new one (default false)."},
                   "wait_for_client": {"type": "boolean", "description": "Also wait until the client window's player is in the level (default true with 2 players)."},
                   "timeout_secs": {"type": "number", "description": "Seconds to wait for the level to load (default 1800)."}}), &["mod"]),
        tool("wait_for_players", "Wait until at least `count` players are in the level with their game's scripts running (each client's game reports to the host once it is really in). Use this instead of guessing from logs or screenshots whether a client has finished joining. Returns the player list.",
            json!({"count": {"type": "integer", "description": "Players needed (default 2)."},
                   "timeout_secs": {"type": "number", "description": "Seconds to wait (default 300)."}}), &[]),
        tool("mp_test_stop", "Close the local multiplayer test windows. With client_only, close only the client window: its player leaves and the host keeps running.",
            json!({"client_only": {"type": "boolean", "description": "Close only \"TD - Client1\" (a player leaving; default false)."}}), &[]),
        tool("mp_test_join", "A late join: start the multiplayer test's client window alone, to join a host that is already running (e.g. one started with mp_test_start players: 1), and wait until its player is in the level. Returns the player list.",
            json!({"timeout_secs": {"type": "number", "description": "Seconds to wait for the player (default 300)."}}), &[]),
        tool("send_input", "Press keys and use the mouse in the game window, as a player would: for testing tools, vehicles driven by hand, and clickable UI. Brings the game window to the front for the duration (the user loses keyboard focus for that time) and sends nothing unless that window really has focus. Avoid esc unless you mean to open the pause menu: scripts stop while it is open.",
            json!({"actions": {"type": "array", "description": "Run in order. Each is one of: {\"key\": \"w\", \"hold_ms\": 500} (letters, digits, space, enter, esc, tab, shift, ctrl, alt, arrows, f1-f12; combos held together with +, e.g. \"shift+alt+f3\"); {\"click\": \"left\"|\"right\"|\"middle\", \"x\": 0.5, \"y\": 0.5, \"hold_ms\": 60} (x, y are fractions of the window, as in a screenshot; omit them to click where the cursor or crosshair is); {\"drag\": [x0, y0], \"to\": [x1, y1], \"hold_ms\": 300} (press at one window fraction, move, release at the other: sliders); {\"move\": [dx, dy]} (relative mouse movement in pixels: turns the camera; about 0.115 degrees per pixel at default sensitivity); {\"scroll\": 1}; {\"wait_ms\": 200}; {\"action\": \"interact\", \"hold_ms\": 60} (a game input action: the key or mouse button the player has bound to it is read from the game's registry, game.input.<action>; e.g. up, down, left, right, jump, crouch, interact, usetool, grab, flashlight, map, handbrake, vehicle_raise, vehicle_lower, vehicle_action, camera_view, pause).", "items": {"type": "object"}},
                   "window": {"type": "string", "description": "Window title (default \"TD - Host\" if present, else \"Teardown\")."},
                   "restore_focus": {"type": "boolean", "description": "Give focus back to the window that had it (default true)."}}), &["actions"]),
        tool("screenshot", "Capture a Teardown window (shows the scene and red script-error text). By default the multiplayer test's host window (\"TD - Host\") if there is one, else the normal game window (\"Teardown\").",
            json!({"max_width": {"type": "integer", "description": "Downscale to this width (default 1280)."},
                   "window": {"type": "string", "description": "Exact window title, e.g. \"TD - Host\", \"TD - Client1\", \"Teardown\"."}}), &[]),
        tool("game_health", "Check the game from outside, without its scripts: each game window's process (responding or hung, CPU, memory, threads, uptime), game processes that ended since the last check with their exit code (a crash shows as access violation etc.), the game's own recent crash reports, and GPU load. Works while the game is hung, paused, in a menu or gone.",
            json!({"window": {"type": "string", "description": "Only this window title (default: all of \"TD - Host\", \"TD - Client1\", \"Teardown\")."}}), &[]),
        tool("crash_report", "Read one of the game's own crash reports (written to %LOCALAPPDATA%/Teardown/crash/ when it crashes): message, Lua call stack, native call stack, memory use (registry, video memory, voxels), active mods, and the end of the game log up to the crash.",
            json!({"index": {"type": "integer", "description": "0 = newest (default)."}, "log_lines": {"type": "integer", "description": "Lines of the log to include (default 60)."}}), &[]),
        tool("dump_game", "For a hung or very slow game: the call stacks of its main thread and busiest other threads, read from outside (each thread is paused for a moment), and optionally a minidump file for a debugger. Use game_health first to see whether it is hung.",
            json!({"window": {"type": "string", "description": "Window title (default \"TD - Host\" if present, else \"Teardown\")."},
                   "write_dump": {"type": "boolean", "description": "Also write a .dmp file to %LOCALAPPDATA%/Teardown/mcp_dumps (default false)."},
                   "full": {"type": "boolean", "description": "With write_dump: all of the game's memory (gigabytes; default false: stacks and what they point to, a few MB)."},
                   "other_threads": {"type": "integer", "description": "How many other threads to show, busiest first (default 6)."}}), &[]),
        tool("save_game_code", "Save the running game exe's code as it is in memory (the game decrypts it at start; the file on disk is encrypted) to %LOCALAPPDATA%/Teardown/mcp_code/<exe>-<build>.exe. For tools/port_names.py, which ports engine function names to a new game build more reliably from such copies, and for disassemblers. The copy is the game's code: keep it on this machine.",
            json!({"window": {"type": "string", "description": "Window title (default \"TD - Host\" if present, else \"Teardown\"). The multiplayer test exe and the normal game exe are different builds: save each."}}), &[]),
        tool("perf_sample", "Measure performance over a few seconds while the game runs: frame times from inside the game (average, percentiles, frames over 33 / 50 ms), the game process's CPU and memory from outside, entity counts, and GPU load. Needs a running level. For which script costs what, open the engine profiler (see the skill).",
            json!({"seconds": {"type": "number", "description": "Length of the sample (default 5, 1..30)."},
                   "window": {"type": "string", "description": "\"TD - Client1\" measures the joined client's game; default the host / single-player game."}}), &[]),
        tool("read_log", "Read the end of Teardown's log file (level loads, missing files/prefabs/sounds, disabled scripts). log.txt is written out completely only at a normal exit, so it can lag far behind the running game; log_sentry.txt is the log of the last session that crashed (written by the crash handler, with times and tags). In the multiplayer test the windows log to log_0.txt (host) and log_1.txt (client), written in 4 KB blocks. A Lua runtime error logs only a WARNING line with the script's path, every time it happens; its message and line number show only on screen: use screenshot for those. Older sessions are in log_0.txt etc.",
            json!({"lines": {"type": "integer", "description": "Lines from the end (default 80)."}, "contains": {"type": "string", "description": "Only lines containing this text (case-insensitive)."},
                   "level": {"type": "string", "enum": ["all", "warning", "error"]}, "file": {"type": "string", "description": "Log file name (default log.txt)."},
                   "bridge_lines": {"type": "boolean", "description": "Also show the bridge's own reply lines (\"Couldn't find sound bank .../mcpr_...\", hundreds a minute in the multiplayer test; hidden by default)."}}), &[]),
    ])
}

/// Tool name -> bridge action, for tools that pass their arguments straight through.
fn simple_action(tool: &str) -> Option<&'static str> {
    Some(match tool {
        "ping" => "ping",
        "run_lua" => "run_lua",
        "call_api" => "call",
        "registry_get" => "registry_get",
        "registry_set" => "registry_set",
        "registry_clear" => "registry_clear",
        "player_get" => "player_get",
        "player_set" => "player_set",
        "spawn" => "spawn",
        "delete_entities" => "delete",
        "find_entities" => "find",
        "describe_entity" => "describe",
        "raycast" => "raycast",
        "debug_print" => "debug_print",
        "look_at" => "look_at",
        _ => return None,
    })
}

fn make_cmd(action: &str, a: &Value, wait_scene: bool) -> Cmd {
    let mut args: Map<String, Value> = a.as_object().cloned().unwrap_or_default();
    let side = match args.remove("side").as_ref().and_then(Value::as_str) {
        Some("client") => "client",
        Some("draw") => "draw",
        Some("remote") => "remote",
        _ => "server",
    };
    for k in ["wait", "wait_scene", "timeout_secs"] {
        args.remove(k);
    }
    if action == "call" {
        // JSON nulls would cut the Lua argument list short: pass the count.
        let n = args.get("args").and_then(Value::as_array).map(|x| x.len()).unwrap_or(0);
        args.insert("argc".into(), json!(n));
    }
    Cmd { action: action.into(), args: Value::Object(args), side: side.into(), wait_scene }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string())
}

fn text(s: String) -> Vec<Value> {
    vec![json!({"type": "text", "text": s})]
}

/// One command's `{ok, result | error}` as a Result.
fn unwrap_result(action: &str, r: &Value) -> Result<Value, String> {
    if r["ok"] != json!(true) {
        return Err(r["error"].as_str().unwrap_or("unknown error").to_string());
    }
    let res = r["result"].clone();
    if action == "run_lua" && res["failed"] == json!(true) {
        let mut msg = format!("Lua error: {}", res["error"].as_str().unwrap_or("?"));
        if let Some(out) = res["output"].as_str() {
            msg.push_str("\noutput before the error:\n");
            msg.push_str(out);
        }
        return Err(msg);
    }
    Ok(res)
}

struct Server {
    bridge: Bridge,
    log_dir: PathBuf,
    game_dir: PathBuf,
    response: Duration,
    /// Process id of the multiplayer test client this server last started: a wait for it to
    /// join ends as soon as that process exits.
    client_pid: Option<u32>,
}

impl Server {
    fn timeouts(&self, a: &Value) -> Timeouts {
        let secs = a["timeout_secs"].as_f64().unwrap_or(1800.0).clamp(5.0, 7200.0);
        Timeouts { response: self.response, scene: Duration::from_secs_f64(secs) }
    }

    fn call(&mut self, name: &str, a: &Value, progress: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        if let Some(action) = simple_action(name) {
            let t = self.timeouts(a);
            let out = self.bridge.send(vec![make_cmd(action, a, false)], &t, progress)?;
            let res = unwrap_result(action, &out.results[0])?;
            return Ok(text(if res.is_null() { "ok".into() } else { pretty(&res) }));
        }
        match name {
            "reload_scene" | "start_level" | "game_command" => {
                let action = match name {
                    "reload_scene" => "restart",
                    "start_level" => "start_level",
                    _ => "game_command",
                };
                let wait = if name == "game_command" {
                    a["wait_scene"].as_bool().unwrap_or(false)
                } else {
                    a["wait"].as_bool().unwrap_or(true)
                };
                let t = self.timeouts(a);
                let out = self.bridge.send(vec![make_cmd(action, a, wait)], &t, progress)?;
                unwrap_result(action, &out.results[0])?;
                Ok(text(match out.scene_wait {
                    Some(d) => format!("Scene is running again after {:.1} s. Level: {}", d.as_secs_f64(), out.level),
                    None if wait => format!("Scene is running. Level: {}", out.level),
                    None => "Requested; not waiting for the scene.".into(),
                }))
            }
            "batch" => {
                let list = a["commands"].as_array().ok_or("commands must be an array")?;
                let mut cmds = Vec::new();
                let mut actions = Vec::new();
                for c in list {
                    let action = c["action"].as_str().ok_or("each command needs an action")?;
                    let default_wait = matches!(action, "restart" | "start_level");
                    let wait = c["wait_scene"].as_bool().unwrap_or(default_wait);
                    let mut args = c["args"].as_object().cloned().unwrap_or_default();
                    if let Some(s) = c.get("side") {
                        args.insert("side".into(), s.clone());
                    }
                    cmds.push(make_cmd(action, &Value::Object(args), wait));
                    actions.push(action.to_string());
                }
                let t = self.timeouts(a);
                let out = self.bridge.send(cmds, &t, progress)?;
                let mut failed = false;
                let items: Vec<Value> = actions
                    .iter()
                    .zip(&out.results)
                    .map(|(action, r)| match unwrap_result(action, r) {
                        Ok(v) => json!({"action": action, "ok": true, "result": v}),
                        Err(e) => {
                            failed = true;
                            json!({"action": action, "ok": false, "error": e})
                        }
                    })
                    .collect();
                let mut s = pretty(&Value::Array(items));
                if let Some(d) = out.scene_wait {
                    s.push_str(&format!("\nScene load(s) took {:.1} s. Level: {}", d.as_secs_f64(), out.level));
                }
                if failed {
                    Err(s)
                } else {
                    Ok(text(s))
                }
            }
            "bridge_status" => {
                let install = self.bridge.install();
                let age = std::fs::metadata(&self.bridge.savegame)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_secs());
                let mut keys = Map::new();
                if let Some(st) = self.bridge.read_state() {
                    let mut names: Vec<_> = st.keys().filter(|k| !k.starts_with('r')).collect();
                    names.sort();
                    for k in names {
                        keys.insert(k.clone(), json!(st[k]));
                    }
                }
                Ok(text(pretty(&json!({
                    "mod_dir": self.bridge.mod_dir,
                    "mod_installed": install.is_ok(),
                    "mod_problem": install.err(),
                    "savegame": self.bridge.savegame,
                    "savegame_last_written_seconds_ago": age,
                    "saved_bridge_keys": keys,
                    "note": "Keys are from the last time the game saved; use ping to check the game answers now."
                }))))
            }
            "mp_test_start" => {
                let m = a["mod"].as_str().ok_or("mod is required")?;
                let local_mods = self.bridge.mod_dir.parent().map(PathBuf::from).unwrap_or_default();
                let (mod_id, level) = mptest::resolve(m, a["level"].as_str(), &self.game_dir, &local_mods, &self.log_dir)?;
                self.bridge.install()?;
                if mptest::running() {
                    if a["restart"].as_bool().unwrap_or(false) {
                        mptest::stop();
                    } else {
                        return Err("a multiplayer test is already running; pass restart=true to close it and start a new one".into());
                    }
                }
                let bridge_folder = self.bridge.mod_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let mut global_mods = vec![format!("local-{}", mptest::slug(&bridge_folder))];
                for x in a["extra_mods"].as_array().into_iter().flatten().filter_map(Value::as_str) {
                    global_mods.push(x.to_string());
                }
                let client = a["players"].as_u64().unwrap_or(2) >= 2;
                let game_mode = a["game_mode"].as_str().unwrap_or("").to_string();
                // A content mode needs its layers; look them up like the mod manager does.
                let layers = match a["layers"].as_str() {
                    Some(l) => l.to_string(),
                    None if !game_mode.is_empty() => {
                        mptest::mode_layers(&game_mode, &self.game_dir, &local_mods)?.unwrap_or_default()
                    }
                    None => String::new(),
                };
                let launch = mptest::Launch {
                    mod_id,
                    level,
                    layers,
                    game_mode,
                    global_mods,
                    client,
                };
                self.client_pid = mptest::start(&launch, &self.game_dir, &self.log_dir)?;
                #[cfg(windows)]
                if let Some(pid) = self.client_pid {
                    diag::watch(pid, "TD - Client1");
                }
                // The windows take a few seconds to get their titles; the loop below checks.
                self.bridge.set_alive(true);

                // The level is up when the bridge in the host window answers.
                let total = self.timeouts(a).scene;
                let started = std::time::Instant::now();
                let quick = Timeouts { response: Duration::from_secs(8), scene: total };
                loop {
                    std::thread::sleep(Duration::from_secs(2));
                    if !mptest::running() {
                        return Err(format!(
                            "the test windows closed after {} s without the level coming up (mod {}, level {}). read_log with file=log_0.txt shows the host's log.",
                            started.elapsed().as_secs(), launch.mod_id, launch.level
                        ));
                    }
                    match self.bridge.send(vec![make_cmd("ping", &json!({}), false)], &quick, progress) {
                        Ok(out) => {
                            let mut info = unwrap_result("ping", &out.results[0]).unwrap_or(Value::Null);
                            let host_secs = started.elapsed().as_secs_f64();
                            let mut note = String::new();
                            if client && a["wait_for_client"].as_bool().unwrap_or(true) {
                                let left = total.saturating_sub(started.elapsed()).max(Duration::from_secs(30));
                                match self.wait_players(2, left, progress) {
                                    Ok(v) => {
                                        info = v;
                                        note = format!(" Client in the level after {:.0} s.", started.elapsed().as_secs_f64());
                                    }
                                    Err(e) => {
                                        return Err(format!(
                                            "the host is up after {host_secs:.0} s (mod {}, level {}), but the client did not join: {e}",
                                            launch.mod_id, launch.level
                                        ));
                                    }
                                }
                            }
                            let mode = if launch.game_mode.is_empty() {
                                String::new()
                            } else {
                                format!(" Game mode {}, layers \"{}\".", launch.game_mode, launch.layers)
                            };
                            return Ok(text(format!(
                                "Multiplayer test: host running after {host_secs:.0} s.{note} Mod {}, level {}, global mods {:?}.{mode}\n{}",
                                launch.mod_id, launch.level, launch.global_mods, pretty(&info)
                            )));
                        }
                        Err(e) if started.elapsed() > total => {
                            return Err(format!("the test windows started but the bridge did not answer within {} s: {e}", total.as_secs()));
                        }
                        Err(_) => {}
                    }
                }
            }
            "wait_for_players" => {
                let count = a["count"].as_u64().unwrap_or(2).max(1) as usize;
                let secs = a["timeout_secs"].as_f64().unwrap_or(300.0).clamp(1.0, 7200.0);
                let info = self.wait_players(count, Duration::from_secs_f64(secs), progress)?;
                Ok(text(pretty(&info)))
            }
            "mp_test_stop" if a["client_only"].as_bool().unwrap_or(false) => self.mp_client_leave(),
            "mp_test_stop" => Ok(text(if mptest::stop() {
                self.bridge.set_alive(false);
                "Multiplayer test windows closed.".into()
            } else {
                "No multiplayer test was running.".into()
            })),
            "mp_test_join" => self.mp_client_join(a, progress),
            "game_health" | "crash_report" | "dump_game" | "perf_sample" | "save_game_code" => self.diag(name, a, progress),
            "send_input" => self.send_input(a, progress),
            "screenshot" => self.screenshot(a),
            "read_log" => self.read_log(a),
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    /// Ping the host until `count` players are in the level with their scripts running.
    /// Returns the last ping.
    fn wait_players(&mut self, count: usize, limit: Duration, progress: &mut dyn FnMut(&str)) -> Result<Value, String> {
        self.wait_players_except(count, &[], limit, progress)
    }

    /// Wait until `count` ready players are listed whose ids are not in `known`.
    fn wait_players_except(
        &mut self,
        count: usize,
        known: &[Value],
        limit: Duration,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Value, String> {
        let started = std::time::Instant::now();
        let t = Timeouts { response: self.response, scene: limit };
        // Waiting for a joiner: once the test's client window has been seen, its closing (a
        // crash while loading) ends the wait at once. Online clients have no window here.
        let joiner = count >= 2 || !known.is_empty();
        let mut client_seen = false;
        loop {
            #[cfg(windows)]
            if let Some(code) = self.client_pid.filter(|_| joiner).and_then(diag::exit_code) {
                self.client_pid = None;
                return Err(format!(
                    "the multiplayer test's client (\"TD - Client1\") exited with {} after {} s of waiting for it to join. read_log with file log_1.txt shows its log, crash_report the game's own crash report if it wrote one; mp_test_join starts a new client (the host keeps running)",
                    diag::exit_text(code),
                    started.elapsed().as_secs()
                ));
            }
            if joiner {
                match (self.bridge.windows_check)(true) {
                    Ok(()) => client_seen = true,
                    Err(e) if client_seen => {
                        return Err(format!("{e}. It closed after {} s of waiting for it to join.", started.elapsed().as_secs()));
                    }
                    Err(_) => {}
                }
            }
            let out = self.bridge.send(vec![make_cmd("ping", &json!({}), false)], &t, progress)?;
            let info = unwrap_result("ping", &out.results[0])?;
            let players = info["players"].as_array().cloned().unwrap_or_default();
            let ready = players
                .iter()
                .filter(|p| p["ready"] == json!(true) && !known.contains(&p["id"]))
                .count();
            if ready >= count {
                return Ok(info);
            }
            if started.elapsed() > limit {
                let what = if known.is_empty() { "players" } else { "new players" };
                return Err(format!(
                    "{ready} of {count} {what} ready after {} s; players: {}",
                    limit.as_secs(),
                    Value::Array(players)
                ));
            }
            std::thread::sleep(Duration::from_millis(1000));
        }
    }

    #[cfg(windows)]
    fn diag(&mut self, name: &str, a: &Value, progress: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        match name {
            "game_health" => Ok(text(pretty(&diag::health(&self.log_dir, a["window"].as_str())))),
            "crash_report" => {
                let index = a["index"].as_u64().unwrap_or(0) as usize;
                let lines = a["log_lines"].as_u64().unwrap_or(60).clamp(0, 2000) as usize;
                Ok(text(pretty(&diag::crash_report(&self.log_dir, index, lines)?)))
            }
            "dump_game" => {
                let g = self.pick_game(a["window"].as_str())?;
                let dir = a["write_dump"].as_bool().unwrap_or(false).then(|| self.log_dir.join("mcp_dumps"));
                let others = a["other_threads"].as_u64().unwrap_or(6).min(64) as usize;
                Ok(text(pretty(&diag::dump(&g, dir.as_deref(), a["full"].as_bool().unwrap_or(false), others)?)))
            }
            "save_game_code" => {
                let g = self.pick_game(a["window"].as_str())?;
                Ok(text(pretty(&diag::save_code(&g, &self.log_dir.join("mcp_code"))?)))
            }
            _ => self.perf_sample(a, progress),
        }
    }

    #[cfg(windows)]
    fn pick_game(&self, window: Option<&str>) -> Result<diag::Game, String> {
        let mut gs = diag::games(window);
        if gs.is_empty() {
            return Err(match window {
                Some(w) => format!("no Teardown window titled \"{w}\""),
                None => "no Teardown window found (\"TD - Host\", \"TD - Client1\" or \"Teardown\")".into(),
            });
        }
        // Default: the host of a multiplayer test, else the normal game.
        let i = if window.is_some() { 0 } else { gs.iter().position(|g| g.title != "TD - Client1").unwrap_or(0) };
        Ok(gs.swap_remove(i))
    }

    #[cfg(windows)]
    fn perf_sample(&mut self, a: &Value, progress: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        let secs = a["seconds"].as_f64().unwrap_or(5.0).clamp(1.0, 30.0);
        let g = self.pick_game(a["window"].as_str())?;
        let side = if g.title == "TD - Client1" { "remote" } else { "client" };
        let t = Timeouts { response: self.response, scene: self.response };
        let c0 = diag::counters(g.pid).ok_or("cannot read the game process")?;
        let out = self.bridge.send(vec![make_cmd("perf", &json!({"side": side}), false)], &t, progress)?;
        let r0 = unwrap_result("perf", &out.results[0])?;
        let wall = std::time::Instant::now();
        std::thread::sleep(Duration::from_secs_f64(secs));
        let since = r0["frame"].as_u64().unwrap_or(0);
        let out = self.bridge.send(
            vec![make_cmd("perf", &json!({"side": side, "since": since}), false), make_cmd("scene_stats", &json!({}), false)],
            &t,
            progress,
        )?;
        let r1 = unwrap_result("perf", &out.results[0])?;
        let scene = unwrap_result("scene_stats", &out.results[1]).unwrap_or_else(|e| json!({"error": e}));
        let wall_s = wall.elapsed().as_secs_f64();
        let c1 = diag::counters(g.pid).ok_or("cannot read the game process")?;
        let (core, all) = diag::cpu_between(&c0, &c1);
        let frames = r1["frames"].as_f64().unwrap_or(0.0);
        let sum = r1["frame_time_sum_s"].as_f64().unwrap_or(0.0);
        let round = |x: f64, d: i32| (x * 10f64.powi(d)).round() / 10f64.powi(d);
        let mut ft = r1.clone();
        if let Some(o) = ft.as_object_mut() {
            o.remove("frame");
            o.remove("time");
            for v in o.values_mut() {
                if let Some(x) = v.as_f64() {
                    *v = json!(round(x, 2));
                }
            }
        }
        Ok(text(pretty(&json!({
            "window": g.title,
            "fps_avg": if sum > 0.0 { round(frames / sum, 1) } else { 0.0 },
            "frame_times": ft,
            "game_seconds": round(r1["time"].as_f64().unwrap_or(0.0) - r0["time"].as_f64().unwrap_or(0.0), 2),
            "wall_seconds": round(wall_s, 2),
            "process": {
                "cpu_percent_of_one_core": core, "cpu_percent_of_machine": all,
                "working_set_mb": round(c1.working_set as f64 / 1048576.0, 1),
                "private_mb": round(c1.private as f64 / 1048576.0, 1),
                "private_change_mb": round((c1.private as f64 - c0.private as f64) / 1048576.0, 1),
            },
            "scene": scene,
            "gpu": diag::gpu(),
            "note": "Frame times are dt as client.tick sees it, one per rendered frame. game_seconds < wall_seconds means the game ran slower than real time. With the engine profiler open the frame rate is capped at 60. A single-player pause stops the count."
        }))))
    }

    /// A late join: start the client window alone and wait until its player is in the level.
    #[cfg(windows)]
    fn mp_client_join(&mut self, a: &Value, progress: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        if diag::games(Some("TD - Host")).is_empty() {
            return Err("no multiplayer test host window (\"TD - Host\") is open: start one with mp_test_start (players: 1 for the host alone)".into());
        }
        if !diag::games(Some("TD - Client1")).is_empty() {
            return Err("the client window (\"TD - Client1\") is already open; mp_test_stop with client_only: true closes it".into());
        }
        let t = Timeouts { response: self.response, scene: self.response };
        let before = self.bridge.send(vec![make_cmd("ping", &json!({}), false)], &t, progress)?;
        // Wait for a player id that was not listed before: right after mp_test_stop client_only
        // the host can still list the player who left, and ids are never reused (a rejoin gets
        // a new one), so counting players would wait for one too many.
        let known: Vec<Value> = unwrap_result("ping", &before.results[0])?["players"]
            .as_array()
            .map(|ps| ps.iter().map(|p| p["id"].clone()).collect())
            .unwrap_or_default();
        let pid = mptest::start_client(&self.game_dir, &self.log_dir)?;
        diag::watch(pid, "TD - Client1");
        self.client_pid = Some(pid);
        let started = std::time::Instant::now();
        let secs = a["timeout_secs"].as_f64().unwrap_or(300.0).clamp(10.0, 7200.0);
        let info = self.wait_players_except(1, &known, Duration::from_secs_f64(secs), progress)?;
        Ok(text(format!(
            "Client joined after {:.0} s.\n{}",
            started.elapsed().as_secs_f64(),
            pretty(&info)
        )))
    }

    /// Close only the client window: its player leaves, the host keeps running.
    #[cfg(windows)]
    fn mp_client_leave(&mut self) -> Result<Vec<Value>, String> {
        let gs = diag::games(Some("TD - Client1"));
        let Some(g) = gs.first() else {
            return Ok(text("No client window (\"TD - Client1\") was open.".into()));
        };
        mptest::kill_pid(g.pid);
        for _ in 0..50 {
            if diag::games(Some("TD - Client1")).is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        Ok(text("Client window closed; the host keeps running.".into()))
    }

    #[cfg(not(windows))]
    fn mp_client_join(&mut self, _a: &Value, _p: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        Err("the multiplayer test is only available on Windows".into())
    }

    #[cfg(not(windows))]
    fn mp_client_leave(&mut self) -> Result<Vec<Value>, String> {
        Err("the multiplayer test is only available on Windows".into())
    }

    #[cfg(not(windows))]
    fn diag(&mut self, _name: &str, _a: &Value, _p: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        Err("diagnostics are only available on Windows".into())
    }

    #[cfg(windows)]
    fn send_input(&mut self, a: &Value, progress: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        let actions = a["actions"].as_array().ok_or("actions must be an array")?;
        let window = a["window"].as_str();
        let mut note = String::new();
        let resolved;
        let actions = if actions.iter().any(|x| x["action"].is_string()) {
            let (r, n) = self.resolve_input_actions(actions, window, progress)?;
            resolved = r;
            note = n;
            &resolved
        } else {
            actions
        };
        let report = input::send(actions, window, a["restore_focus"].as_bool().unwrap_or(true))?;
        Ok(text(format!("Sent: {report}{note}")))
    }

    /// Replace {"action": "<game input>"} entries by the key or mouse button the player has bound
    /// to it, read from the game's registry (game.input.<action>, e.g. interact = "e",
    /// usetool = "lmb"). The bindings are read from the game behind the window the input goes to.
    #[cfg(windows)]
    fn resolve_input_actions(
        &mut self,
        actions: &[Value],
        window: Option<&str>,
        progress: &mut dyn FnMut(&str),
    ) -> Result<(Vec<Value>, String), String> {
        let side = if window == Some("TD - Client1") { "remote" } else { "client" };
        let t = Timeouts { response: self.response, scene: self.response };
        let out = self.bridge.send(
            vec![make_cmd("registry_get", &json!({"key": "game.input", "depth": 1, "side": side}), false)],
            &t,
            progress,
        )?;
        let reg = unwrap_result("registry_get", &out.results[0])?;
        let binds = &reg["children"];
        let mut mapped = Vec::new();
        let mut out_actions = Vec::with_capacity(actions.len());
        for x in actions {
            let Some(name) = x["action"].as_str() else {
                out_actions.push(x.clone());
                continue;
            };
            // curdevice holds the current input device ("mouse"), not a binding.
            let bound = if name == "curdevice" { "" } else { binds[name]["value"].as_str().unwrap_or("") };
            if bound.is_empty() {
                let known: Vec<&str> = binds
                    .as_object()
                    .map(|o| {
                        o.iter()
                            .filter(|(k, v)| *k != "curdevice" && v["value"].as_str().is_some_and(|s| !s.is_empty()))
                            .map(|(k, _)| k.as_str())
                            .collect()
                    })
                    .unwrap_or_default();
                return Err(format!(
                    "the game has no key bound to input action \"{name}\" (game.input.{name}); bound actions: {}",
                    known.join(", ")
                ));
            }
            let mut y = x.as_object().cloned().unwrap_or_default();
            y.remove("action");
            match bound {
                "lmb" => { y.insert("click".into(), json!("left")); }
                "rmb" => { y.insert("click".into(), json!("right")); }
                "mmb" => { y.insert("click".into(), json!("middle")); }
                "wheel" => { y.insert("scroll".into(), json!(if name.contains("down") { -1 } else { 1 })); }
                k => {
                    let key = match k {
                        "uparrow" => "up",
                        "downarrow" => "down",
                        "leftarrow" => "left",
                        "rightarrow" => "right",
                        "pgup" => "pageup",
                        "pgdown" => "pagedown",
                        other => other,
                    };
                    y.insert("key".into(), json!(key));
                }
            }
            mapped.push(format!("{name} = {bound}"));
            out_actions.push(Value::Object(y));
        }
        Ok((out_actions, format!(" (bindings: {})", mapped.join(", "))))
    }

    #[cfg(not(windows))]
    fn send_input(&mut self, _a: &Value, _p: &mut dyn FnMut(&str)) -> Result<Vec<Value>, String> {
        Err("send_input is only available on Windows".into())
    }

    #[cfg(windows)]
    fn screenshot(&self, a: &Value) -> Result<Vec<Value>, String> {
        use base64::Engine;
        let max = a["max_width"].as_u64().unwrap_or(1280).clamp(160, 4096) as u32;
        let s = shot::capture(max, a["window"].as_str())?;
        Ok(vec![
            json!({"type": "image", "mimeType": "image/jpeg",
                   "data": base64::engine::general_purpose::STANDARD.encode(&s.jpeg)}),
            json!({"type": "text", "text": format!("{}x{}, captured from the {}", s.width, s.height, s.method)}),
        ])
    }

    #[cfg(not(windows))]
    fn screenshot(&self, _a: &Value) -> Result<Vec<Value>, String> {
        Err("screenshot is only available on Windows".into())
    }

    fn read_log(&self, a: &Value) -> Result<Vec<Value>, String> {
        let file = a["file"].as_str().unwrap_or("log.txt");
        if !file.starts_with("log") || !file.ends_with(".txt") || file.contains(['/', '\\']) {
            return Err("file must be a log*.txt name".into());
        }
        let path = self.log_dir.join(file);
        let bytes = std::fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if bytes.is_empty() {
            return Ok(text(format!(
                "{} is empty for now: the game writes it in bursts. Try again later, log_0.txt / log_1.txt for earlier sessions, or screenshot for on-screen errors.",
                path.display()
            )));
        }
        let content = String::from_utf8_lossy(&bytes);
        let needle = a["contains"].as_str().map(str::to_lowercase);
        let level = a["level"].as_str().unwrap_or("all");
        let want = a["lines"].as_u64().unwrap_or(80).clamp(1, 2000) as usize;
        let bridge = a["bridge_lines"].as_bool().unwrap_or(false);
        let mut hidden = 0usize;
        let keep: Vec<&str> = content
            .lines()
            .filter(|l| {
                // The bridge's replies in the multiplayer test are failed LoadSound calls on
                // <bridge folder>/mcps_..., mcpr_..., mcpp_... (mod/main.lua): hundreds a minute.
                let own = !bridge && l.contains("Couldn't find sound")
                    && ["/mcps_", "/mcpr_", "/mcpp_"].iter().any(|p| l.contains(p));
                hidden += own as usize;
                !own
            })
            .filter(|l| match level {
                "error" => l.contains(" ERROR "),
                "warning" => l.contains(" ERROR ") || l.contains(" WARNING "),
                _ => true,
            })
            .filter(|l| needle.as_ref().map(|n| l.to_lowercase().contains(n)).unwrap_or(true))
            .collect();
        let from = keep.len().saturating_sub(want);
        let note = if hidden > 0 { format!("; {hidden} bridge reply lines hidden") } else { String::new() };
        Ok(text(format!("{} ({} matching lines, showing last {}{note}):\n{}", path.display(), keep.len(), keep.len() - from, keep[from..].join("\n"))))
    }
}

fn main() {
    let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    let mod_dir = env_path("TEARDOWN_MCP_MOD_DIR")
        .unwrap_or_else(|| home.join("Documents").join("Teardown").join("mods").join("mcp-bridge"));
    let savegame = env_path("TEARDOWN_SAVEGAME").unwrap_or_else(|| local.join("Teardown").join("savegame.xml"));
    let log_dir = savegame.parent().map(PathBuf::from).unwrap_or_default();
    let response = std::env::var("TEARDOWN_MCP_RESPONSE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(20.0);

    let game_dir = env_path("TEARDOWN_DIR")
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\Teardown"));
    let mut server = Server {
        bridge: Bridge::new(mod_dir, savegame),
        log_dir,
        game_dir,
        response: Duration::from_secs_f64(response),
        client_pid: None,
    };
    // Where results come back: "save" (savegame.xml), "log" (the game's log file), or by
    // default the log whenever the multiplayer test's host window is open.
    server.bridge.detect_log = match std::env::var("TEARDOWN_MCP_REPLY").as_deref() {
        Ok("log") => || true,
        Ok("save") => || false,
        _ => host_test_window,
    };
    #[cfg(windows)]
    {
        server.bridge.alive_check = diag::alive_check;
        server.bridge.windows_check = diag::windows_check;
    }
    if let Err(e) = server.bridge.install() {
        eprintln!("teardown-mcp: {e}");
    }

    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                send(&json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}}));
                continue;
            }
        };
        let Some(id) = msg.get("id").cloned() else { continue }; // notification
        let method = msg["method"].as_str().unwrap_or("");
        let params = &msg["params"];
        let reply = |result: Value| send(&json!({"jsonrpc": "2.0", "id": id, "result": result}));
        match method {
            "initialize" => reply(json!({
                "protocolVersion": params["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "teardown-mcp", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS
            })),
            "ping" => reply(json!({})),
            "tools/list" => reply(json!({"tools": tools()})),
            "tools/call" => {
                let name = params["name"].as_str().unwrap_or("");
                let empty = json!({});
                let args = if params["arguments"].is_object() { &params["arguments"] } else { &empty };
                let token = params["_meta"]["progressToken"].clone();
                let mut n = 0;
                let mut progress = |m: &str| {
                    if !token.is_null() {
                        n += 1;
                        send(&json!({"jsonrpc": "2.0", "method": "notifications/progress",
                                     "params": {"progressToken": token, "progress": n, "message": m}}));
                    }
                };
                match server.call(name, args, &mut progress) {
                    Ok(content) => reply(json!({"content": content, "isError": false})),
                    Err(e) => reply(json!({"content": [{"type": "text", "text": e}], "isError": true})),
                }
            }
            _ => send(&json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("method not found: {method}")}})),
        }
    }
}
