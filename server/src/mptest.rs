//! Start and stop Teardown's local multiplayer test: two `teardown_modtest.exe` windows
//! (host + client), the same way the game's mod manager launches them:
//!
//!   teardown_modtest.exe -half_screen 1 -mod_test_host "<mod id>" "<level xml>" "" ""
//!                        "<global mod id>" -logfile <appdata>/log_0.txt
//!   teardown_modtest.exe -half_screen 2 -mod_test_client -logfile <appdata>/log_1.txt

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const EXE: &str = "teardown_modtest.exe";
const STEAM_APP_ID: &str = "1167630";

pub struct Launch {
    pub mod_id: String,
    pub level: String,
    pub global_mods: Vec<String>,
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

pub fn start(l: &Launch, game_dir: &Path, appdata: &Path) -> Result<(), String> {
    let exe = game_dir.join(EXE);
    if !exe.is_file() {
        return Err(format!("{} not found (set TEARDOWN_DIR to the game folder)", exe.display()));
    }
    let spawn = |args: Vec<String>| -> Result<(), String> {
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
            .map(|_| ())
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))
    };
    let mut host: Vec<String> = ["-half_screen", "1", "-mod_test_host", &l.mod_id, &l.level, "", ""]
        .iter()
        .map(|s| s.to_string())
        .collect();
    host.extend(l.global_mods.iter().cloned());
    host.push("-logfile".into());
    host.push(fwd(&appdata.join("log_0.txt")));
    spawn(host)?;
    spawn(
        ["-half_screen", "2", "-mod_test_client", "-logfile", &fwd(&appdata.join("log_1.txt"))]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    )
}
