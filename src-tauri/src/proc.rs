//! Subprocess helper with a wall-clock timeout — protects the blocking thread
//! pool from hung external commands (`git`, `gh`).

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;
use wait_timeout::ChildExt;

pub const GIT_TIMEOUT: Duration = Duration::from_secs(15);
pub const GH_TIMEOUT: Duration = Duration::from_secs(20);

/// Run an external command in `cwd`, capturing stdout, with a wall-clock timeout.
/// Returns `Err(msg)` if the command fails, times out, or could not be spawned.
pub fn run_capturing(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn {program}: {e}"))?;

    let status = match child
        .wait_timeout(timeout)
        .map_err(|e| format!("wait_timeout failed: {e}"))?
    {
        Some(s) => s,
        None => {
            // Killing on timeout is best-effort: if it fails the child becomes a
            // zombie, but we will not block this thread further.
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} timed out after {}s", timeout.as_secs()));
        }
    };

    let mut stdout = String::new();
    if let Some(mut s) = child.stdout.take() {
        let _ = s.read_to_string(&mut stdout);
    }
    if !status.success() {
        let mut stderr = String::new();
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut stderr);
        }
        return Err(stderr.trim().to_string());
    }
    Ok(stdout)
}
