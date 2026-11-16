//! Teardown MCP server (stdio). Drives a running Teardown through the "MCP Bridge" mod.

mod bridge;
#[cfg(windows)]
mod shot;

use bridge::{Bridge, Cmd, Timeouts};
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::time::Duration;

const INSTRUCTIONS: &str = "Drives a running Teardown game through the in-game \"MCP Bridge\" mod, for developing Teardown mods. \
The game must be inside a level with the mod enabled. Mod scripts do not run in the pause menu or the main menu, and the bridge cannot leave them: never pause the game (SetPaused), quit to the menu, or load a level path you are not sure of; if the game stops answering, take a screenshot and ask the user to resume. \
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
    let side = json!({"type": "string", "enum": ["server", "client"], "description": "Which half of the bridge script runs it (default server)."});
    let vec3 = |d: &str| json!({"type": "array", "items": {"type": "number"}, "minItems": 3, "maxItems": 3, "description": d});
    let rot = json!({"type": "array", "items": {"type": "number"}, "description": "Rotation: quaternion [x,y,z,w] or Euler degrees [x,y,z]."});
    let timeout = json!({"type": "number", "description": "Seconds to wait for the new scene (default 600)."});
    let player = json!({"type": "integer", "description": "Player id (default 0: the host on the server, the local player on the client)."});
    json!([
        tool("ping", "Check that the game answers. Returns game version, level path, and scene counter.", json!({"side": side}), &[]),
        tool("bridge_status", "Bridge state without talking to the game: file paths and the mod's saved status keys.", json!({}), &[]),
        tool("run_lua", "Run Lua code in the game and return its return values and anything it print()s. An expression works too (\"GetPlayerHealth()\"). Full Teardown API available.",
            json!({"code": {"type": "string"}, "side": side}), &["code"]),
        tool("call_api", "Call one Teardown API function by name with JSON arguments, e.g. fn=\"SetPlayerHealth\", args=[1].",
            json!({"fn": {"type": "string"}, "args": {"type": "array"}, "side": side}), &["fn"]),
        tool("reload_scene", "Restart the current level so changes to mods on disk take effect, then wait until the new scene is running.",
            json!({"wait": {"type": "boolean", "description": "Wait for the new scene (default true)."}, "timeout_secs": timeout}), &[]),
        tool("start_level", "Load another level via StartLevel(mission, path, layers) and wait until it is running. RISKY: a path the engine cannot load drops the game to the main menu, where the bridge stops working until the user loads a level by hand. Built-in level paths are taken relative to data/level/ (\"data/level/hub.xml\" failed in testing). Prefer reload_scene; use this only with a path known to work.",
            json!({"path": {"type": "string"}, "mission": {"type": "string", "description": "Mission/level id (default empty)."}, "layers": {"type": "string", "description": "Space-separated layers to enable."},
                   "wait": {"type": "boolean"}, "timeout_secs": timeout}), &["path"]),
        tool("game_command", "Run an engine command via Command(), e.g. \"game.quicksave\", \"game.quickload\". Set wait_scene only if the command reloads the scene and its scripts.",
            json!({"command": {"type": "string"}, "args": {"type": "array"}, "wait_scene": {"type": "boolean"}, "timeout_secs": timeout, "side": side}), &["command"]),
        tool("batch", "Run several bridge actions in order in one go. Actions: ping, run_lua, call, restart, start_level, game_command, registry_get, registry_set, registry_clear, player_get, player_set, spawn, delete, find, describe, raycast, debug_print. Commands after a restart/start_level run in the new scene.",
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
        tool("player_get", "Player transform, camera, velocity, health, vehicle and tool.", json!({"player": player, "side": side}), &[]),
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
        tool("debug_print", "Print a line in the game's on-screen debug console.", json!({"text": {"type": "string"}, "side": side}), &["text"]),
        tool("screenshot", "Capture the Teardown window (shows the scene and red script-error text).",
            json!({"max_width": {"type": "integer", "description": "Downscale to this width (default 1280)."}}), &[]),
        tool("read_log", "Read the end of Teardown's log file (level loads, missing files/prefabs/sounds, disabled scripts). The game writes it in bursts, so the newest lines can lag behind. Lua runtime errors are not in it: use screenshot for those. Older sessions are in log_0.txt etc.",
            json!({"lines": {"type": "integer", "description": "Lines from the end (default 80)."}, "contains": {"type": "string", "description": "Only lines containing this text (case-insensitive)."},
                   "level": {"type": "string", "enum": ["all", "warning", "error"]}, "file": {"type": "string", "description": "Log file name (default log.txt)."}}), &[]),
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
        _ => return None,
    })
}

fn make_cmd(action: &str, a: &Value, wait_scene: bool) -> Cmd {
    let mut args: Map<String, Value> = a.as_object().cloned().unwrap_or_default();
    let side = match args.remove("side").as_ref().and_then(Value::as_str) {
        Some("client") => "client",
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
    response: Duration,
}

impl Server {
    fn timeouts(&self, a: &Value) -> Timeouts {
        let secs = a["timeout_secs"].as_f64().unwrap_or(600.0).clamp(5.0, 7200.0);
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
            "screenshot" => self.screenshot(a),
            "read_log" => self.read_log(a),
            _ => Err(format!("unknown tool: {name}")),
        }
    }

    #[cfg(windows)]
    fn screenshot(&self, a: &Value) -> Result<Vec<Value>, String> {
        use base64::Engine;
        let max = a["max_width"].as_u64().unwrap_or(1280).clamp(160, 4096) as u32;
        let s = shot::capture(max)?;
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
        let keep: Vec<&str> = content
            .lines()
            .filter(|l| match level {
                "error" => l.contains(" ERROR "),
                "warning" => l.contains(" ERROR ") || l.contains(" WARNING "),
                _ => true,
            })
            .filter(|l| needle.as_ref().map(|n| l.to_lowercase().contains(n)).unwrap_or(true))
            .collect();
        let from = keep.len().saturating_sub(want);
        Ok(text(format!("{} ({} matching lines, showing last {}):\n{}", path.display(), keep.len(), keep.len() - from, keep[from..].join("\n"))))
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

    let mut server = Server { bridge: Bridge::new(mod_dir, savegame), log_dir, response: Duration::from_secs_f64(response) };
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
