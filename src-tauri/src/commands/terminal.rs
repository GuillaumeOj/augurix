//! "Open terminal" — hand the user off to the terminal window/tab where a
//! Claude Code session is actually running (or open a fresh one as a fallback).
//!
//! macOS only for now. We know the session's OS `pid` (and `cwd`) and map it to
//! a window:
//!   * Apple Terminal / iTerm2 — `pid → controlling tty` (`ps -o tty=`), then
//!     AppleScript selects the tab/session whose `tty` matches and activates.
//!   * kitty — remote control: `kitty @ ls` reports each window's
//!     `foreground_processes`; we match our `pid` then `kitty @ focus-window`.

use crate::caches::SharedCaches;
use crate::proc::run_capturing;
use crate::settings::{Settings, SettingsStore, TerminalChoice};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

type SharedSettings = std::sync::Arc<SettingsStore>;

const TERM_TIMEOUT: Duration = Duration::from_secs(8);

/// One terminal option for the settings page.
#[derive(Debug, Clone, Serialize)]
pub struct TerminalInfo {
    pub choice: TerminalChoice,
    pub name: String,
    /// Whether the terminal is installed on this machine.
    pub installed: bool,
    /// Whether it's usable right now (for kitty: remote control reachable).
    pub ready: bool,
    /// Optional human-facing hint (e.g. "Not installed", setup needed).
    pub note: Option<String>,
}

/// Result of clicking "Open terminal" — the frontend switches on `status`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum OpenTerminalOutcome {
    /// Focused the session (or opened a fallback terminal) successfully.
    Ok,
    /// No terminal configured yet — frontend should route to /settings.
    NeedsSetup,
    /// kitty is selected but remote control isn't reachable — route to /settings.
    KittyNeedsSetup,
    /// Something went wrong; surface the message.
    Error { message: String },
}

/// Result of the kitty automatic-setup action.
#[derive(Debug, Clone, Serialize)]
pub struct KittySetupResult {
    /// Whether we modified kitty.conf.
    pub changed: bool,
    pub config_path: String,
    pub backup_path: Option<String>,
    /// True if kitty must be restarted for the change to take effect.
    pub needs_restart: bool,
    /// True if remote control is already reachable.
    pub ready: bool,
    pub message: String,
}

#[tauri::command]
pub fn get_settings(settings: State<SharedSettings>) -> Settings {
    settings.get()
}

#[tauri::command]
pub fn set_terminal(
    settings: State<SharedSettings>,
    choice: TerminalChoice,
) -> Result<Settings, String> {
    settings.set_terminal(choice).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_terminals() -> Vec<TerminalInfo> {
    let mut out = Vec::new();

    // Apple Terminal ships with macOS.
    out.push(TerminalInfo {
        choice: TerminalChoice::AppleTerminal,
        name: "Apple Terminal".into(),
        installed: true,
        ready: true,
        note: None,
    });

    // iTerm2.
    let iterm = Path::new("/Applications/iTerm.app").exists();
    out.push(TerminalInfo {
        choice: TerminalChoice::Iterm2,
        name: "iTerm2".into(),
        installed: iterm,
        ready: iterm,
        note: (!iterm).then(|| "Not installed".to_string()),
    });

    // kitty.
    let bin = kitty_bin();
    let installed = bin.is_some();
    let ready = bin.as_deref().map(kitty_remote_ready).unwrap_or(false);
    let note = if !installed {
        Some("Not installed".to_string())
    } else if !ready {
        Some("Remote control not enabled — run automatic setup".to_string())
    } else {
        None
    };
    out.push(TerminalInfo {
        choice: TerminalChoice::Kitty,
        name: "kitty".into(),
        installed,
        ready,
        note,
    });

    out
}

#[tauri::command]
pub fn setup_kitty() -> Result<KittySetupResult, String> {
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
    let cfg_dir = home.join(".config").join("kitty");
    let cfg = cfg_dir.join("kitty.conf");
    let config_path = cfg.display().to_string();

    let existing = std::fs::read_to_string(&cfg).unwrap_or_default();
    let ready_now = kitty_bin()
        .as_deref()
        .map(kitty_remote_ready)
        .unwrap_or(false);

    if kitty_config_enabled(&existing) {
        return Ok(KittySetupResult {
            changed: false,
            config_path,
            backup_path: None,
            needs_restart: !ready_now,
            ready: ready_now,
            message: if ready_now {
                "kitty remote control is already enabled and reachable.".into()
            } else {
                "kitty.conf already enables remote control — restart kitty (quit & relaunch) \
                 to apply it."
                    .into()
            },
        });
    }

    std::fs::create_dir_all(&cfg_dir)
        .map_err(|e| format!("creating {}: {e}", cfg_dir.display()))?;

    // Back up any existing config before appending.
    let backup_path = if cfg.exists() {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let backup = cfg.with_extension(format!("conf.bak-{ts}"));
        std::fs::copy(&cfg, &backup).map_err(|e| format!("backing up kitty.conf: {e}"))?;
        Some(backup.display().to_string())
    } else {
        None
    };

    let mut next = existing;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(KITTY_CONFIG_BLOCK);
    std::fs::write(&cfg, next).map_err(|e| format!("writing {}: {e}", config_path))?;

    Ok(KittySetupResult {
        changed: true,
        config_path,
        backup_path,
        needs_restart: true,
        ready: false,
        message: "Enabled remote control in kitty.conf. Quit and relaunch kitty (a config \
                  reload is not enough for listen_on) and then click \"Open terminal\" again."
            .into(),
    })
}

#[tauri::command]
pub fn open_terminal(
    settings: State<SharedSettings>,
    caches: State<SharedCaches>,
    pid: Option<u32>,
    cwd: String,
    project_root: Option<String>,
) -> OpenTerminalOutcome {
    let choice = match settings.get().terminal {
        Some(c) => c,
        None => return OpenTerminalOutcome::NeedsSetup,
    };

    // When the displayed instance has no detected pid (e.g. a session launched
    // at the repo root but shown under a nested worktree), search the project
    // subtree so we still focus the real session window rather than guessing by
    // cwd — which can land on an unrelated shell sitting in the worktree.
    let pid = match (pid, project_root.as_deref()) {
        (Some(p), _) => Some(p),
        (None, Some(root)) => {
            let snap = caches.snapshot_processes();
            crate::commands::agents::find_session_process(Path::new(&cwd), Path::new(root), &snap)
        }
        (None, None) => None,
    };

    match choice {
        TerminalChoice::AppleTerminal => focus_apple_terminal(pid, &cwd),
        TerminalChoice::Iterm2 => focus_iterm2(pid, &cwd),
        TerminalChoice::Kitty => focus_kitty(pid, &cwd),
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

const KITTY_CONFIG_BLOCK: &str = "\n# --- Added by Augurix (open-terminal integration) ---\n\
allow_remote_control yes\n\
listen_on unix:/tmp/kitty\n";

fn run_osascript(script: &str) -> Result<String, String> {
    run_capturing("osascript", &["-e", script], Path::new("/"), TERM_TIMEOUT)
}

/// Escape a string for embedding inside an AppleScript double-quoted literal.
fn applescript_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Controlling tty of `pid`, as a `/dev/...` path, or `None` if it has no tty.
fn tty_for_pid(pid: u32) -> Option<String> {
    let out = run_capturing(
        "ps",
        &["-o", "tty=", "-p", &pid.to_string()],
        Path::new("/"),
        TERM_TIMEOUT,
    )
    .ok()?;
    let t = out.trim();
    if t.is_empty() || t == "??" || t == "-" {
        return None;
    }
    Some(if t.starts_with("/dev/") {
        t.to_string()
    } else {
        format!("/dev/{t}")
    })
}

// ---------------------------------------------------------------------------
// Apple Terminal
// ---------------------------------------------------------------------------

fn focus_apple_terminal(pid: Option<u32>, cwd: &str) -> OpenTerminalOutcome {
    if let Some(tty) = pid.and_then(tty_for_pid) {
        let script = format!(
            r#"tell application "Terminal"
    activate
    set targetTTY to "{tty}"
    repeat with w in windows
        repeat with t in tabs of w
            try
                if tty of t is targetTTY then
                    set selected of t to true
                    set frontmost of w to true
                    return "ok"
                end if
            end try
        end repeat
    end repeat
end tell
return "nomatch""#
        );
        match run_osascript(&script) {
            Ok(out) if out.trim() == "ok" => return OpenTerminalOutcome::Ok,
            Ok(_) => {} // no matching tab — fall back to opening a new one
            Err(e) => return OpenTerminalOutcome::Error { message: e },
        }
    }

    let esc = applescript_escape(cwd);
    let script = format!(
        r#"tell application "Terminal"
    activate
    do script "cd " & quoted form of "{esc}"
end tell"#
    );
    match run_osascript(&script) {
        Ok(_) => OpenTerminalOutcome::Ok,
        Err(e) => OpenTerminalOutcome::Error { message: e },
    }
}

// ---------------------------------------------------------------------------
// iTerm2
// ---------------------------------------------------------------------------

fn focus_iterm2(pid: Option<u32>, cwd: &str) -> OpenTerminalOutcome {
    if let Some(tty) = pid.and_then(tty_for_pid) {
        let script = format!(
            r#"tell application "iTerm"
    activate
    repeat with w in windows
        repeat with t in tabs of w
            repeat with s in sessions of t
                try
                    if tty of s is "{tty}" then
                        tell s to select
                        return "ok"
                    end if
                end try
            end repeat
        end repeat
    end repeat
end tell
return "nomatch""#
        );
        match run_osascript(&script) {
            Ok(out) if out.trim() == "ok" => return OpenTerminalOutcome::Ok,
            Ok(_) => {}
            Err(e) => return OpenTerminalOutcome::Error { message: e },
        }
    }

    let esc = applescript_escape(cwd);
    let script = format!(
        r#"tell application "iTerm"
    activate
    set newWindow to (create window with default profile)
    tell current session of newWindow
        write text "cd " & quoted form of "{esc}"
    end tell
end tell"#
    );
    match run_osascript(&script) {
        Ok(_) => OpenTerminalOutcome::Ok,
        Err(e) => OpenTerminalOutcome::Error { message: e },
    }
}

// ---------------------------------------------------------------------------
// kitty (remote control)
// ---------------------------------------------------------------------------

fn kitty_bin() -> Option<String> {
    if let Ok(out) = run_capturing("which", &["kitty"], Path::new("/"), TERM_TIMEOUT) {
        let p = out.trim();
        if !p.is_empty() {
            return Some(p.to_string());
        }
    }
    let app = "/Applications/kitty.app/Contents/MacOS/kitty";
    Path::new(app).exists().then(|| app.to_string())
}

/// Parse the `listen_on unix:<path>` base from a kitty.conf body. The last
/// directive wins (kitty's own precedence). `tcp:` and abstract `unix:@`
/// addresses are ignored — they aren't filesystem sockets we can glob.
fn parse_listen_on(content: &str) -> Option<PathBuf> {
    let mut base = None;
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        if let Some(rest) = l.strip_prefix("listen_on") {
            if let Some(path) = rest.trim().strip_prefix("unix:") {
                let path = path.trim();
                if !path.is_empty() && !path.starts_with('@') {
                    base = Some(PathBuf::from(path));
                }
            }
        }
    }
    base
}

/// The configured unix socket base from `~/.config/kitty/kitty.conf`, if any.
/// e.g. `listen_on unix:/tmp/mykitty` -> `/tmp/mykitty`.
fn configured_kitty_socket_base() -> Option<PathBuf> {
    let cfg = dirs::home_dir()?
        .join(".config")
        .join("kitty")
        .join("kitty.conf");
    parse_listen_on(&std::fs::read_to_string(cfg).ok()?)
}

/// All candidate kitty control sockets. kitty creates one socket per running
/// instance, named `<listen_on>-<pid>`. We discover them three ways so we work
/// regardless of how the user named their socket:
///   1. the `listen_on` path from kitty.conf (authoritative — e.g. `mykitty`),
///   2. our auto-setup default `/tmp/kitty`,
///   3. a safety net: any `/tmp` entry whose name contains "kitty".
///
/// Reachability is proven later by actually running `kitty @ ... ls`, so any
/// non-socket false positives collected here are harmless.
fn kitty_sockets() -> Vec<PathBuf> {
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(b) = configured_kitty_socket_base() {
        bases.push(b);
    }
    bases.push(PathBuf::from("/tmp/kitty"));

    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut out: Vec<PathBuf> = Vec::new();
    let mut push = |p: PathBuf, out: &mut Vec<PathBuf>| {
        if seen.insert(p.clone()) {
            out.push(p);
        }
    };

    for base in &bases {
        let dir = base.parent().unwrap_or_else(|| Path::new("/tmp"));
        let prefix = match base.file_name().and_then(|s| s.to_str()) {
            Some(p) => p.to_string(),
            None => continue,
        };
        let dashed = format!("{prefix}-");
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let name = e.file_name();
                let name = name.to_string_lossy();
                if name.as_ref() == prefix || name.starts_with(&dashed) {
                    push(e.path(), &mut out);
                }
            }
        }
    }

    // Safety net: catch sockets like `/tmp/mykitty-<pid>` regardless of config.
    if let Ok(rd) = std::fs::read_dir("/tmp") {
        for e in rd.flatten() {
            if e.file_name()
                .to_string_lossy()
                .to_lowercase()
                .contains("kitty")
            {
                push(e.path(), &mut out);
            }
        }
    }

    out
}

fn kitty_remote_ready(bin: &str) -> bool {
    kitty_sockets().iter().any(|s| {
        let to = format!("unix:{}", s.display());
        run_capturing(bin, &["@", "--to", &to, "ls"], Path::new("/"), TERM_TIMEOUT).is_ok()
    })
}

/// Returns true if kitty.conf already enables socket remote control + listen_on.
/// Later directives win in kitty, so we take the final value seen for each.
fn kitty_config_enabled(content: &str) -> bool {
    let mut remote = false;
    let mut listen = false;
    for line in content.lines() {
        let l = line.trim();
        if l.starts_with('#') {
            continue;
        }
        if let Some(rest) = l.strip_prefix("allow_remote_control") {
            let v = rest.trim();
            remote = matches!(v, "yes" | "socket-only" | "socket");
        }
        if let Some(rest) = l.strip_prefix("listen_on") {
            listen = rest.trim().starts_with("unix:");
        }
    }
    remote && listen
}

fn focus_kitty(pid: Option<u32>, cwd: &str) -> OpenTerminalOutcome {
    let bin = match kitty_bin() {
        Some(b) => b,
        None => {
            return OpenTerminalOutcome::Error {
                message: "kitty executable not found".into(),
            }
        }
    };

    // Sweep the sockets once: focus the window hosting the session if we find
    // it, and remember the first reachable socket for the open-a-fresh-tab
    // fallback (avoids a second `kitty @ ls` round-trip per socket).
    let mut first_reachable: Option<String> = None;
    for sock in kitty_sockets() {
        let to = format!("unix:{}", sock.display());
        let ls = match run_capturing(
            &bin,
            &["@", "--to", &to, "ls"],
            Path::new("/"),
            TERM_TIMEOUT,
        ) {
            Ok(o) => o,
            Err(_) => continue,
        };
        if first_reachable.is_none() {
            first_reachable = Some(to.clone());
        }

        if let Some(window_id) = find_kitty_window(&ls, pid, cwd) {
            let match_arg = format!("id:{window_id}");
            let _ = run_capturing(
                &bin,
                &["@", "--to", &to, "focus-window", "--match", &match_arg],
                Path::new("/"),
                TERM_TIMEOUT,
            );
            let _ = run_osascript(r#"tell application "kitty" to activate"#);
            return OpenTerminalOutcome::Ok;
        }
    }

    let Some(to) = first_reachable else {
        // No socket answered — remote control isn't set up (or kitty isn't running).
        return OpenTerminalOutcome::KittyNeedsSetup;
    };

    // Reachable, but the session isn't in any kitty window — open a fresh tab.
    let _ = run_capturing(
        &bin,
        &["@", "--to", &to, "launch", "--type=tab", "--cwd", cwd],
        Path::new("/"),
        TERM_TIMEOUT,
    );
    let _ = run_osascript(r#"tell application "kitty" to activate"#);
    OpenTerminalOutcome::Ok
}

#[derive(Deserialize)]
struct KOsWindow {
    #[serde(default)]
    tabs: Vec<KTab>,
}
#[derive(Deserialize)]
struct KTab {
    #[serde(default)]
    windows: Vec<KWindow>,
}
#[derive(Deserialize)]
struct KWindow {
    id: i64,
    #[serde(default)]
    pid: Option<i64>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    foreground_processes: Vec<KProc>,
}
#[derive(Deserialize)]
struct KProc {
    #[serde(default)]
    pid: Option<i64>,
    #[serde(default)]
    cwd: Option<String>,
}

/// Find the kitty window hosting `pid` (preferred) or whose cwd matches `cwd`.
fn find_kitty_window(ls_json: &str, pid: Option<u32>, cwd: &str) -> Option<i64> {
    let osw: Vec<KOsWindow> = serde_json::from_str(ls_json).ok()?;

    if let Some(target) = pid.map(|p| p as i64) {
        for w in &osw {
            for t in &w.tabs {
                for win in &t.windows {
                    if win.pid == Some(target)
                        || win
                            .foreground_processes
                            .iter()
                            .any(|p| p.pid == Some(target))
                    {
                        return Some(win.id);
                    }
                }
            }
        }
    }

    let cwd_canon = std::fs::canonicalize(cwd).ok();
    let matches_cwd = |candidate: &str| -> bool {
        if candidate == cwd {
            return true;
        }
        match (&cwd_canon, std::fs::canonicalize(candidate)) {
            (Some(cc), Ok(wc)) => &wc == cc,
            _ => false,
        }
    };
    for w in &osw {
        for t in &w.tabs {
            for win in &t.windows {
                // The shell's cwd (window) can differ from the running command's
                // (foreground process), so check both.
                let hit = win.cwd.as_deref().is_some_and(&matches_cwd)
                    || win
                        .foreground_processes
                        .iter()
                        .any(|p| p.cwd.as_deref().is_some_and(&matches_cwd));
                if hit {
                    return Some(win.id);
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_enabled_detection() {
        assert!(!kitty_config_enabled(""));
        assert!(!kitty_config_enabled("allow_remote_control yes"));
        assert!(kitty_config_enabled(
            "allow_remote_control yes\nlisten_on unix:/tmp/kitty"
        ));
        assert!(kitty_config_enabled(
            "listen_on unix:/tmp/kitty\nallow_remote_control socket-only"
        ));
        // last value wins — a later `no` disables it.
        assert!(!kitty_config_enabled(
            "allow_remote_control yes\nlisten_on unix:/tmp/kitty\nallow_remote_control no"
        ));
        // commented lines are ignored.
        assert!(!kitty_config_enabled(
            "# allow_remote_control yes\n# listen_on unix:/tmp/kitty"
        ));
    }

    #[test]
    fn parses_listen_on_base() {
        assert_eq!(
            parse_listen_on("allow_remote_control yes\nlisten_on unix:/tmp/mykitty"),
            Some(PathBuf::from("/tmp/mykitty"))
        );
        // Last directive wins.
        assert_eq!(
            parse_listen_on("listen_on unix:/tmp/a\nlisten_on unix:/tmp/b"),
            Some(PathBuf::from("/tmp/b"))
        );
        // Commented, tcp, and abstract addresses are ignored.
        assert_eq!(parse_listen_on("# listen_on unix:/tmp/x"), None);
        assert_eq!(parse_listen_on("listen_on tcp:localhost:123"), None);
        assert_eq!(parse_listen_on("listen_on unix:@abstract"), None);
        assert_eq!(parse_listen_on(""), None);
    }

    #[test]
    fn find_window_by_foreground_cwd() {
        // pid is None (session not running as tracked) — fall back to cwd, and
        // the match lives on the foreground process, not the window.
        let json = r#"[{"tabs":[{"windows":[
          {"id":9,"pid":1,"cwd":"/Users/me","foreground_processes":[
            {"pid":2,"cwd":"/Users/me/dev/app"}
          ]}
        ]}]}]"#;
        assert_eq!(find_kitty_window(json, None, "/Users/me/dev/app"), Some(9));
        assert_eq!(find_kitty_window(json, None, "/Users/me/dev/other"), None);
    }

    #[test]
    fn applescript_escaping() {
        assert_eq!(applescript_escape("/a/b"), "/a/b");
        assert_eq!(applescript_escape(r#"/a "x"/b"#), r#"/a \"x\"/b"#);
        assert_eq!(applescript_escape(r"/a\b"), r"/a\\b");
    }

    #[test]
    fn find_window_by_foreground_pid() {
        let json = r#"[
          {"tabs":[
            {"windows":[
              {"id":1,"pid":100,"cwd":"/x","foreground_processes":[{"pid":101}]},
              {"id":2,"pid":200,"cwd":"/y","foreground_processes":[{"pid":222}]}
            ]}
          ]}
        ]"#;
        assert_eq!(find_kitty_window(json, Some(222), "/nope"), Some(2));
        assert_eq!(find_kitty_window(json, Some(100), "/nope"), Some(1));
    }

    #[test]
    fn find_window_by_cwd_fallback() {
        let json = r#"[{"tabs":[{"windows":[
          {"id":7,"pid":1,"cwd":"/repo/main","foreground_processes":[]}
        ]}]}]"#;
        assert_eq!(find_kitty_window(json, None, "/repo/main"), Some(7));
        assert_eq!(find_kitty_window(json, Some(999), "/repo/other"), None);
    }
}
