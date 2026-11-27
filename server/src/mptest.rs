//! Start and stop Teardown's local multiplayer test: two `teardown_modtest.exe` windows
//! (host + client), the same way the game's mod manager launches them:
//!
//!   teardown_modtest.exe -half_screen 1 -mod_test_host "<mod id>" "<level xml>" "<layers>"
//!                        "<game mode id>" "<global mod ids, space-separated, one argument>"
//!                        -logfile <appdata>/log_0.txt
//!   teardown_modtest.exe -half_screen 2 -mod_test_client -logfile <appdata>/log_1.txt

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const EXE: &str = "teardown_modtest.exe";
const STEAM_APP_ID: &str = "1167630";

pub struct Launch {
    pub mod_id: String,
    pub level: String,
    /// Level layers to enable ("" = none), as the mod manager passes for a content game mode.
    pub layers: String,
    /// Game mode id "<mod id>.<Name from gamemodes.txt>" ("" = none).
    pub game_mode: String,
    pub global_mods: Vec<String>,
    /// Also start the client window (false: the host plays alone, still a multiplayer session).
    pub client: bool,
}

/// Mod id the way the game derives it from a folder name: lower case, anything that is
/// not a letter or digit becomes '-'.
pub fn slug(folder: &str) -> String {
    folder
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

fn fwd(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Ids the game knows, from its mods.xml.
fn known_ids(appdata: &Path) -> Vec<String> {
    let text = std::fs::read_to_string(appdata.join("mods.xml")).unwrap_or_default();
    text.split("<mod id=\"").skip(1).filter_map(|s| s.split('"').next()).map(str::to_string).collect()
}

/// Folder of a mod id: local mods, built-in mods, Workshop mods.
fn mod_folder(id: &str, game_dir: &Path, local_mods: &Path) -> Option<PathBuf> {
    let find = |dir: &Path, want: &str| -> Option<PathBuf> {
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
            let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            // The game sometimes appends a digit to the id (seen: "...--release-0").
            p.is_dir() && want.starts_with(&slug(&name)) && want.len() <= slug(&name).len() + 1
        })
    };
    if let Some(rest) = id.strip_prefix("local-") {
        find(local_mods, rest)
    } else if let Some(rest) = id.strip_prefix("builtin-") {
        find(&game_dir.join("mods"), rest)
    } else if let Some(rest) = id.strip_prefix("steam-") {
        // <library>/steamapps/common/Teardown -> <library>/steamapps/workshop/content/<app>/<id>
        let p = game_dir.parent()?.parent()?.join("workshop").join("content").join(STEAM_APP_ID).join(rest);
        p.is_dir().then_some(p)
    } else {
        None
    }
}

/// A game mode's `layers` from its mod's gamemodes.txt, as the mod manager looks them up
/// before it starts a test (`gamemodes.<id>.layers`). `mode` is "<mod id>.<key>"; the key is
/// matched the way the game makes registry keys: section name in lower case, other characters
/// as '-' ("MCP probe" and "mcp-probe" both match `[MCP probe]`). Ok(None): a global mode
/// (no `layers`). Err: the mod or the mode was not found.
pub fn mode_layers(mode: &str, game_dir: &Path, local_mods: &Path) -> Result<Option<String>, String> {
    let (mod_id, key) = mode
        .split_once('.')
        .ok_or_else(|| format!("game_mode \"{mode}\" must look like \"<mod id>.<mode>\""))?;
    let folder = mod_folder(mod_id, game_dir, local_mods)
        .ok_or_else(|| format!("cannot find the folder of mod {mod_id} (from game_mode \"{mode}\")"))?;
    let text = std::fs::read_to_string(folder.join("gamemodes.txt"))
        .map_err(|e| format!("{}: {e}", folder.join("gamemodes.txt").display()))?;
    let want = slug(key);
    let mut names = Vec::new();
    let mut current: Option<String> = None;
    let mut layers: Option<String> = None;
    let mut found = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            if found {
                break;
            }
            names.push(name.to_string());
            found = slug(name) == want;
            current = Some(name.to_string());
            continue;
        }
        if found && current.is_some() {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim() == "layers" {
                    let v = v.trim();
                    layers = (!v.is_empty()).then(|| v.to_string());
                }
            }
        }
    }
    if !found {
        return Err(format!(
            "mod {mod_id} has no game mode \"{key}\" in its gamemodes.txt; it has: {}",
            names.iter().map(|n| format!("{mod_id}.{}", slug(n))).collect::<Vec<_>>().join(", ")
        ));
    }
    Ok(layers)
}

/// Turn what the caller gave (a mod id, or a folder name under the local mods folder) into
/// the id and level file to start.
pub fn resolve(
    mod_arg: &str,
    level: Option<&str>,
    game_dir: &Path,
    local_mods: &Path,
    appdata: &Path,
) -> Result<(String, String), String> {
    let ids = known_ids(appdata);
    let id = if ids.iter().any(|i| i == mod_arg) {
        mod_arg.to_string()
    } else {
        let want = format!("local-{}", slug(mod_arg));
        match ids.iter().find(|i| **i == want).or_else(|| {
            ids.iter().find(|i| i.starts_with(&want) && i.len() == want.len() + 1)
        }) {
            Some(i) => i.clone(),
            // Not in mods.xml yet (the main game has not seen it), but the folder is there.
            None if mod_folder(&want, game_dir, local_mods).is_some() => want.clone(),
            None if mod_folder(mod_arg, game_dir, local_mods).is_some() => mod_arg.to_string(),
            None => {
                let mut local: Vec<&String> = ids.iter().filter(|i| i.starts_with("local-")).collect();
                local.sort();
                return Err(format!(
                    "no mod \"{mod_arg}\" known to the game. Give a mod id (local-..., steam-<workshop id>, builtin-...) or a folder name in {}. Local mods: {:?}",
                    local_mods.display(),
                    local
                ));
            }
        }
    };
    let level = match level {
        Some(l) if Path::new(l).is_absolute() => fwd(Path::new(l)),
        other => {
            let folder = mod_folder(&id, game_dir, local_mods)
                .ok_or_else(|| format!("cannot find the folder of mod {id}; pass `level` as a full path"))?;
            fwd(&folder.join(other.unwrap_or("main.xml")))
        }
    };
    if !Path::new(&level).is_file() {
        return Err(format!("level file not found: {level}"));
    }
    Ok((id, level))
}

pub fn running() -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {EXE}"), "/NH", "/FO", "CSV"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_lowercase().contains(EXE))
        .unwrap_or(false)
}

/// Close the test windows. They never write the savegame, so nothing is lost.
pub fn stop() -> bool {
    let was = running();
    if was {
        let _ = Command::new("taskkill")
            .args(["/IM", EXE, "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        for _ in 0..50 {
            if !running() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
    was
}

#[cfg(windows)]
fn keep_stdio_private() {
    // A started game inherits every inheritable handle of ours, including the pipes an MCP
    // client talks to us through; the client would then wait on the game for end-of-output.
    #[link(name = "kernel32")]
    extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
        fn SetHandleInformation(handle: *mut std::ffi::c_void, mask: u32, flags: u32) -> i32;
    }
    for which in [-10i32 as u32, -11i32 as u32, -12i32 as u32] {
        unsafe {
            let h = GetStdHandle(which);
            if !h.is_null() {
                SetHandleInformation(h, 1, 0); // HANDLE_FLAG_INHERIT off
            }
        }
    }
}

#[cfg(not(windows))]
fn keep_stdio_private() {}

/// Start the test exe; returns its process id.
fn spawn_exe(game_dir: &Path, args: Vec<String>) -> Result<u32, String> {
    keep_stdio_private();
    let exe = game_dir.join(EXE);
    if !exe.is_file() {
        return Err(format!("{} not found (set TEARDOWN_DIR to the game folder)", exe.display()));
    }
    Command::new(&exe)
        .args(&args)
        .current_dir(game_dir)
        // Started outside Steam's own launch, the game needs to be told which app it is.
        .env("SteamAppId", STEAM_APP_ID)
        .env("SteamGameId", STEAM_APP_ID)
        // Never let the game write into our stdout: that is the MCP channel.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|c| c.id())
        .map_err(|e| format!("cannot start {}: {e}", exe.display()))
}

/// The client window alone, to join a host that is already running (a late join). Returns
/// the client's process id.
pub fn start_client(game_dir: &Path, appdata: &Path) -> Result<u32, String> {
    spawn_exe(
        game_dir,
        ["-half_screen", "2", "-mod_test_client", "-logfile", &fwd(&appdata.join("log_1.txt"))]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
}

/// Close one test window's process (the client, to make a player leave).
pub fn kill_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// Returns the client's process id when a client was started.
pub fn start(l: &Launch, game_dir: &Path, appdata: &Path) -> Result<Option<u32>, String> {
    let spawn = |args: Vec<String>| spawn_exe(game_dir, args);
    // Alone, the host gets a normal window instead of the left half of the screen.
    let mut host: Vec<String> = Vec::new();
    if l.client {
        host.extend(["-half_screen".to_string(), "1".to_string()]);
    }
    // The two slots after the level take what the mod manager's Command("mods.testplay", n,
    // mod, layers, gameModeId) passes: layers, then the game mode.
    host.extend(["-mod_test_host", &l.mod_id, &l.level, &l.layers, &l.game_mode].iter().map(|s| s.to_string()));
    // All global mods go in ONE argument, separated by spaces, as the game's own launcher
    // passes them ("local-mcp-bridge builtin-mpclassics"). As separate arguments the host
    // takes only the first and drops the rest (a global game mode then never starts). The
    // host's "Args:" log line joins arguments with spaces and hides the difference.
    host.push(l.global_mods.join(" "));
    host.push("-logfile".into());
    host.push(fwd(&appdata.join("log_0.txt")));
    spawn(host)?;
    if !l.client {
        return Ok(None);
    }
    start_client(game_dir, appdata).map(Some)
}
