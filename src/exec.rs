//! Approval-gated execution: run the approved command via the user's shell.

use nobodywho::miette;

/// Run `command` through `shell -c`, inheriting stdio so output streams live.
/// Returns the process exit code (signals map to 128+signum, as shells do).
pub fn run(shell: &str, command: &str) -> miette::Result<i32> {
    use std::os::unix::process::ExitStatusExt;

    let status = std::process::Command::new(shell)
        .arg("-c")
        .arg(command)
        .status()
        .map_err(|e| miette::miette!("failed to spawn {shell}: {e}"))?;

    Ok(match status.code() {
        Some(code) => code,
        None => 128 + status.signal().unwrap_or(0),
    })
}
