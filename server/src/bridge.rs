//! File bridge to the in-game mod.
//!
//! Commands go out as a Lua file in the mod folder (`mcp_inbox.lua`), which the mod polls.
//! Results come back as `savegame.mod.mcp_*` registry keys, read here from savegame.xml.

use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct Cmd {
    pub action: String,
    pub args: Value,
    pub side: String,
    /// The command reloads the scene; wait until the mod reports the new scene running.
    pub wait_scene: bool,
}

pub struct Outcome {
    pub results: Vec<Value>,
    /// How long the scene load(s) took, if any command waited for one.
    pub scene_wait: Option<Duration>,
    pub level: String,
}

pub struct Timeouts {
    /// No answer from the game at all for this long: give up.
    pub response: Duration,
    /// A scene load may take this long.
    pub scene: Duration,
}

pub struct Bridge {
    pub mod_dir: PathBuf,
    pub savegame: PathBuf,
    session: String,
    next_id: u64,
    done: u64,
    pending: Vec<(u64, Cmd)>,
    key_re: Regex,
}

pub type State = HashMap<String, String>;

fn new_session() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:x}-{:x}", nanos, std::process::id())
}

/// JSON value as a Lua literal.
fn lua(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("nil"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => lua_str(s, out),
        Value::Array(a) => {
            out.push('{');
            for x in a {
                lua(x, out);
                out.push(',');
            }
            out.push('}');
        }
        Value::Object(o) => {
            out.push('{');
            for (k, x) in o {
                out.push('[');
                lua_str(k, out);
                out.push_str("]=");
                lua(x, out);
                out.push(',');
            }
            out.push('}');
        }
    }
}

fn lua_str(s: &str, out: &mut String) {
    out.push('"');
    for &b in s.as_bytes() {
        if b.is_ascii_alphanumeric() || b" _.,:;-+*/()[]{}=<>!?@#$%^&|~'".contains(&b) {
            out.push(b as char);
        } else {
            let _ = write!(out, "\\{:03}", b);
        }
    }
    out.push('"');
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = |c: u8| (c as char).to_digit(16);
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn xml_unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn int(st: &State, key: &str) -> u64 {
    st.get(key).and_then(|v| v.parse().ok()).unwrap_or(0)
}

impl Bridge {
    pub fn new(mod_dir: PathBuf, savegame: PathBuf) -> Bridge {
        Bridge {
            mod_dir,
            savegame,
            session: new_session(),
            next_id: 1,
            done: 0,
            pending: Vec::new(),
            key_re: Regex::new(r#"<mcp_([A-Za-z0-9_]+)(?:\s+value="([^"]*)")?\s*/>"#).unwrap(),
        }
    }

    /// Write the mod's path config and an empty inbox. Fails if the mod folder is missing.
    pub fn install(&self) -> Result<(), String> {
        if !self.mod_dir.join("main.lua").exists() {
            return Err(format!(
                "bridge mod not found at {} (set TEARDOWN_MCP_MOD_DIR)",
                self.mod_dir.display()
            ));
        }
        let mut dir = self.mod_dir.to_string_lossy().replace('\\', "/");
        if !dir.ends_with('/') {
            dir.push('/');
        }
        let mut cfg = String::from(
            "-- Written by the MCP server at startup: absolute path of this mod folder.\nMCP_DIR = ",
        );
        lua_str(&dir, &mut cfg);
        cfg.push('\n');
        let path = self.mod_dir.join("mcp_config.lua");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(cfg.as_str()) {
            std::fs::write(&path, cfg).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        self.write_inbox()
    }

    fn write_inbox(&self) -> Result<(), String> {
        let mut s = String::from("return {session=");
        lua_str(&self.session, &mut s);
        let _ = write!(s, ",done={},cmds={{\n", self.done);
        for (id, c) in &self.pending {
            let _ = write!(s, "{{id={},action=", id);
            lua_str(&c.action, &mut s);
            s.push_str(",side=");
            lua_str(&c.side, &mut s);
            let _ = write!(s, ",wait_scene={},args=", c.wait_scene);
            if c.args.is_object() {
                lua(&c.args, &mut s);
            } else {
                s.push_str("{}");
            }
            s.push_str("},\n");
        }
        s.push_str("}}\n");

        // Write beside, then rename over: the mod never sees a half-written file.
        let tmp = self.mod_dir.join("mcp_inbox.tmp");
        let dst = self.mod_dir.join("mcp_inbox.lua");
        let mut last = String::new();
        for _ in 0..40 {
            match std::fs::write(&tmp, &s).and_then(|_| std::fs::rename(&tmp, &dst)) {
                Ok(()) => return Ok(()),
                Err(e) => last = e.to_string(),
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err(format!("cannot write {}: {last}", dst.display()))
    }

    /// All `mcp_*` keys the mod has saved. None if the file could not be read right now.
    pub fn read_state(&self) -> Option<State> {
        let bytes = std::fs::read(&self.savegame).ok()?;
        let text = String::from_utf8_lossy(&bytes);
        let mut st = State::new();
        for c in self.key_re.captures_iter(&text) {
            let v = c.get(2).map(|m| xml_unescape(m.as_str())).unwrap_or_default();
            st.insert(c[1].to_string(), v);
        }
        Some(st)
    }

    fn result_of(st: &State, id: u64) -> Option<Value> {
        let n = int(st, &format!("r{id}_n"));
        if n == 0 {
            return None;
        }
        let mut s = String::new();
        for i in 1..=n {
            s.push_str(st.get(&format!("r{id}_{i}"))?);
        }
        serde_json::from_str(&pct_decode(&s)).ok()
    }

    /// Run commands in order and wait for their results.
    pub fn send(
        &mut self,
        cmds: Vec<Cmd>,
        t: &Timeouts,
        progress: &mut dyn FnMut(&str),
    ) -> Result<Outcome, String> {
        if cmds.is_empty() {
            return Err("no commands".into());
        }
        self.install()?;
        let scene0 = self.read_state().map(|s| int(&s, "scene")).unwrap_or(0);

        let mut ids = Vec::new();
        let mut waits = Vec::new(); // ids of commands that wait for a scene
        for c in cmds {
            let id = self.next_id;
            self.next_id += 1;
            ids.push(id);
            if c.wait_scene {
                waits.push(id);
            }
            self.pending.push((id, c));
        }
        let last_id = *ids.last().unwrap();
        self.write_inbox()?;

        let start = Instant::now();
        let mut last_progress = start;
        let mut last_note = start;
        let mut last_ack = 0;
        let mut loading_since: Option<Instant> = None;
        let mut scene_wait: Option<Duration> = None;
        let mut halted = false;

        let failure = loop {
            std::thread::sleep(Duration::from_millis(60));
            let now = Instant::now();
            let st = self.read_state().unwrap_or_default();
            let ours = st.get("session").map(|s| s == &self.session).unwrap_or(false);
            let mut loading = false;
            if ours {
                let ack = int(&st, "ack");
                if ack != last_ack {
                    last_ack = ack;
                    last_progress = now;
                }
                halted = int(&st, "halt") == 1;
                let k = waits.iter().filter(|&&id| id <= ack).count() as u64;
                loading = k > 0 && (int(&st, "scene") < scene0 + k || int(&st, "waiting") == 1);
                match (loading, loading_since) {
                    (true, None) => loading_since = Some(now),
                    (false, Some(since)) => {
                        scene_wait = Some(scene_wait.unwrap_or_default() + (now - since));
                        loading_since = None;
                        last_progress = now;
                    }
                    _ => {}
                }
                if ack >= last_id && !loading {
                    let results: Option<Vec<Value>> =
                        ids.iter().map(|&id| Self::result_of(&st, id)).collect();
                    if let Some(results) = results {
                        self.done = last_id;
                        self.pending.clear();
                        let _ = self.write_inbox();
                        let level = st.get("level").cloned().unwrap_or_default();
                        return Ok(Outcome { results, scene_wait, level });
                    }
                }
            }
            let limit = if loading { t.scene } else { t.response };
            if now - last_progress > limit {
                let got: Vec<Value> = ids
                    .iter()
                    .filter_map(|&id| Self::result_of(&st, id).map(|r| json!({"id": id, "result": r})))
                    .collect();
                break if loading {
                    format!(
                        "the scene did not report back within {} s after the reload was requested",
                        limit.as_secs()
                    )
                } else if !ours {
                    format!(
                        "no answer from the game within {} s. Teardown must be running inside a level (not the main menu) with the \"MCP Bridge\" mod enabled. The bridge cannot run while the game is paused or in a menu, and cannot unpause it: ask the user to resume. screenshot shows what the game is doing.",
                        limit.as_secs()
                    )
                } else {
                    format!(
                        "the game stopped answering for {} s (acknowledged up to command {} of {}..={}; the game is probably paused or back in the main menu, which only the user can fix; screenshot shows it). Partial results: {}",
                        limit.as_secs(),
                        last_ack,
                        ids[0],
                        last_id,
                        Value::Array(got)
                    )
                };
            }
            if now - last_note > Duration::from_secs(5) {
                last_note = now;
                progress(&if loading {
                    format!("waiting for the scene to load ({} s)", (now - start).as_secs())
                } else {
                    format!("waiting for the game ({} s)", (now - start).as_secs())
                });
            }
        };

        // Give up: drop the commands so they cannot run later, and if the mod is still
        // holding for a scene that never came, a new session releases it.
        self.pending.clear();
        self.done = last_id;
        if halted {
            self.session = new_session();
        }
        let _ = self.write_inbox();
        Err(failure)
    }
}
