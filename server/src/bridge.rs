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
    /// Decides per call whether results come back through the game's log file instead of
    /// the savegame (the multiplayer test windows never write savegame.xml).
    pub detect_log: fn() -> bool,
    pub log_dir: PathBuf,
    log_mode: bool,
    log_re: Regex,
    log_offsets: HashMap<PathBuf, u64>,
    log_state: State,
    /// Last "<fn> is not available in <side> script" line seen in the game's logs: the engine
    /// stops that half of the calling script, which is the usual reason a half goes quiet.
    pub last_side_error: Option<String>,
    side_re: Regex,
    /// Checks from outside that a game is there to answer (window open, not hung). Run before
    /// sending when the game has not answered for ALIVE_RECHECK, so a session that comes back
    /// hours later learns at once that the game was closed instead of waiting for a timeout.
    pub alive_check: fn() -> Result<(), String>,
    /// When the game last answered or the outside check last passed; None: check first.
    last_alive: Option<Instant>,
    /// When the game last really answered (for the message).
    last_answer: Option<Instant>,
}

/// After this long without an answer from the game, check it is still there before sending.
const ALIVE_RECHECK: Duration = Duration::from_secs(300);

fn ago(d: Duration) -> String {
    let s = d.as_secs();
    if s < 120 {
        format!("{s} s")
    } else if s < 7200 {
        format!("{} min", s / 60)
    } else if s < 172800 {
        format!("{} h {} min", s / 3600, s % 3600 / 60)
    } else {
        format!("{} days", s / 86400)
    }
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
        let log_dir = savegame.parent().map(PathBuf::from).unwrap_or_default();
        Bridge {
            detect_log: || false,
            log_dir,
            log_mode: false,
            // mcpr_<session>_<id>_<chunk>_<chunks>_<hex json>   result chunk
            // mcps_<session>_<ack>_<hex level path>_<n>         a scene started running
            log_re: Regex::new(r"mcp([rs])_([0-9a-f]+-[0-9a-f]+)_(\d+)_(?:(\d+)_(\d+)_)?([0-9a-f]*)(?:_\d+)?\.ogg").unwrap(),
            log_offsets: HashMap::new(),
            log_state: State::new(),
            last_side_error: None,
            alive_check: || Ok(()),
            last_alive: None,
            last_answer: None,
            side_re: Regex::new(r"(\S+: )?(\w+) is not available in (client|server) script").unwrap(),
            mod_dir,
            savegame,
            session: new_session(),
            next_id: 1,
            done: 0,
            pending: Vec::new(),
            key_re: Regex::new(r#"<mcp_([A-Za-z0-9_]+)(?:\s+value="([^"]*)")?\s*/>"#).unwrap(),
        }
    }

    /// Write the mod's path config. Fails if the mod folder is missing. The inbox is only
    /// written when a command is sent, so an idle server never disturbs another one.
    pub fn install(&self) -> Result<(), String> {
        if !self.mod_dir.join("main.lua").exists() {
            return Err(format!(
                "bridge mod not found at {} (set TEARDOWN_MCP_MOD_DIR)",
                self.mod_dir.display()
            ));
        }
        // The inbox lives outside the mod folder: a multiplayer session compares the mods of
        // host and client, and a file changing in the folder gets the client kicked.
        let mut cfg = String::from(
            "-- Written by the MCP server: the file the mod reads its commands from.\nMCP_INBOX = ",
        );
        lua_str(&self.inbox_path().to_string_lossy().replace('\\', "/"), &mut cfg);
        cfg.push('\n');
        for stale in ["mcp_inbox.lua", "mcp_inbox.tmp"] {
            let _ = std::fs::remove_file(self.mod_dir.join(stale));
        }
        let path = self.mod_dir.join("mcp_config.lua");
        if std::fs::read_to_string(&path).ok().as_deref() != Some(cfg.as_str()) {
            std::fs::write(&path, cfg).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        Ok(())
    }

    /// The command file, next to the savegame.
    pub fn inbox_path(&self) -> PathBuf {
        self.log_dir.join("mcp_inbox.lua")
    }

    fn write_inbox(&self) -> Result<(), String> {
        let mut s = String::from("return {session=");
        lua_str(&self.session, &mut s);
        let _ = write!(s, ",log={},done={},cmds={{\n", self.log_mode, self.done);
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
        let dst = self.inbox_path();
        let tmp = dst.with_extension("tmp");
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

    /// Read what the game's log files gained since the last call and fold this session's
    /// bridge lines into `log_state`, shaped like the savegame keys.
    fn tail_logs(&mut self) {
        let Ok(dir) = std::fs::read_dir(&self.log_dir) else { return };
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !name.starts_with("log") || !name.ends_with(".txt") {
                continue;
            }
            let path = entry.path();
            use std::io::{Read, Seek, SeekFrom};
            // Ask the open file for its size: the directory listing lags behind for a file
            // the game is still writing.
            let Ok(mut f) = std::fs::File::open(&path) else { continue };
            let len = f.metadata().map(|m| m.len()).unwrap_or(0);
            let mut off = self.log_offsets.get(&path).copied().unwrap_or(0);
            if len < off {
                off = 0; // the game started a new log
            }
            let mut buf = Vec::new();
            if f.seek(SeekFrom::Start(off)).and_then(|_| f.read_to_end(&mut buf)).is_err() || buf.is_empty() {
                continue;
            }
            // Only whole lines; the rest is picked up next time.
            let Some(end) = buf.iter().rposition(|&b| b == b'\n') else { continue };
            let text = String::from_utf8_lossy(&buf[..=end]).into_owned();
            self.log_offsets.insert(path, off + end as u64 + 1);
            if let Some(c) = self.side_re.captures_iter(&text).last() {
                self.last_side_error = Some(c[0].to_string());
            }
            for c in self.log_re.captures_iter(&text) {
                if c[2] != self.session {
                    continue;
                }
                self.log_state.insert("session".into(), self.session.clone());
                let num: u64 = c[3].parse().unwrap_or(0);
                // hex -> %XX, the form result_of() decodes
                let hex = &c[6];
                let mut enc = String::with_capacity(hex.len() / 2 * 3);
                for pair in hex.as_bytes().chunks_exact(2) {
                    enc.push('%');
                    enc.push(pair[0] as char);
                    enc.push(pair[1] as char);
                }
                if &c[1] == "r" {
                    let (Some(i), Some(n)) = (c.get(4), c.get(5)) else { continue };
                    self.log_state.insert(format!("r{num}_{}", i.as_str()), enc);
                    self.log_state.insert(format!("r{num}_n"), n.as_str().to_string());
                } else if num >= int(&self.log_state, "scene_ack") {
                    self.log_state.insert("scene_ack".into(), num.to_string());
                    self.log_state.insert("scene_seen".into(), "1".into());
                    self.log_state.insert("level".into(), pct_decode(&enc));
                }
            }
        }
        // Commands run in order: the newest complete result is the acknowledged one.
        let mut ack = int(&self.log_state, "ack");
        for (id, _) in &self.pending {
            if *id > ack && Self::result_of(&self.log_state, *id).is_some() {
                ack = *id;
            }
        }
        self.log_state.insert("ack".into(), ack.to_string());
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

    /// Forget that the game answered: the next command checks the game windows first (after
    /// the multiplayer test was stopped), or count it as checked (right after starting one).
    pub fn set_alive(&mut self, alive: bool) {
        self.last_alive = alive.then(Instant::now);
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
        if self.last_alive.is_none_or(|t| t.elapsed() > ALIVE_RECHECK) {
            if let Err(e) = (self.alive_check)() {
                let when = match self.last_answer {
                    Some(t) => format!("the game last answered {} ago", ago(t.elapsed())),
                    None => "this server has not reached the game yet".to_string(),
                };
                return Err(format!(
                    "{e}. Nothing was sent: the server looks at the game windows before the first command after 5 minutes without an answer, a failed call or mp_test_stop ({when})."
                ));
            }
            self.last_alive = Some(Instant::now());
        }
        self.log_mode = (self.detect_log)();
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
            self.tail_logs();
            let st = if self.log_mode {
                self.log_state.clone()
            } else {
                self.read_state().unwrap_or_default()
            };
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
                loading = if self.log_mode {
                    // The new scene's first line names the last command it knows of.
                    match waits.iter().filter(|&&id| id <= ack).max() {
                        Some(&w) => !(st.contains_key("scene_seen") && int(&st, "scene_ack") >= w),
                        None => false,
                    }
                } else {
                    k > 0 && (int(&st, "scene") < scene0 + k || int(&st, "waiting") == 1)
                };
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
                        self.last_alive = Some(Instant::now());
                        self.last_answer = self.last_alive;
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
                        "no answer from the game within {} s. Teardown must be running inside a level (not the main menu) with the \"MCP Bridge\" mod enabled ({}). The bridge does not run in a menu; in the single-player pause menu it answers side=client commands only (an older copy of the bridge mod answers nothing there). Take a screenshot: from the pause menu, run_lua side=client with SetPaused(false) or a send_input click on Continue resumes; otherwise ask the user.",
                        limit.as_secs(),
                        if self.log_mode {
                            "multiplayer test window found: expecting replies in its log file"
                        } else {
                            "expecting replies in savegame.xml"
                        }
                    )
                } else {
                    format!(
                        "the game stopped answering for {} s (acknowledged up to command {} of {}..={}; the game is probably back in the main menu, or paused with an older bridge mod; screenshot shows it, and in the pause menu run_lua side=client with SetPaused(false) or a send_input click on Continue resumes). Partial results: {}",
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

        // Say at once if the game is gone or hung, and check again before the next command.
        let failure = match (self.alive_check)() {
            Err(e) => format!("{e}. ({failure})"),
            Ok(()) => failure,
        };
        self.last_alive = None;
        let failure = match &self.last_side_error {
            Some(line) => format!(
                "{failure}\nThe game's log has: \"{line}\". A call on the wrong side makes the engine stop that half of the calling script until the level reloads; if that was the bridge, reload the level from the other half (reload_scene with side=client or side=server) or restart the multiplayer test."
            ),
            None => failure,
        };
        // Give up: drop the commands so they cannot run later, and if the mod is still
        // holding for a scene that never came, a new session releases it.
        self.pending.clear();
        self.done = last_id;
        if halted || loading_since.is_some() {
            self.session = new_session();
            self.log_state.clear();
        }
        let _ = self.write_inbox();
        Err(failure)
    }
}
