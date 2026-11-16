//! Terminal context the model conditions on: OS, shell, cwd listing.

use std::path::{Path, PathBuf};

/// What we tell the model about the environment it is writing a command for.
pub struct EnvContext {
    /// e.g. "Linux 6.18.63"
    pub os: String,
    /// shell *name* (basename of $SHELL), e.g. "fish"
    pub shell: String,
    pub cwd: PathBuf,
    /// Truncated top-level listing of `cwd`, dirs suffixed with `/`.
    pub entries: Vec<String>,
}

const MAX_ENTRIES: usize = 60;

pub fn gather() -> EnvContext {
    let os = std::process::Command::new("uname")
        .args(["-sr"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| format!("{} {}", std::env::consts::OS, std::env::consts::ARCH));

    let shell = std::env::var("SHELL")
        .ok()
        .map(|s| {
            Path::new(&s)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or(s)
        })
        .unwrap_or_else(|| "sh".to_string());

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let mut entries = list_dir(&cwd);
    entries.sort();
    entries.truncate(MAX_ENTRIES);

    EnvContext {
        os,
        shell,
        cwd,
        entries,
    }
}

fn list_dir(path: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(path) else {
        return vec![];
    };
    rd.filter_map(|entry| {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let suffix = if entry.file_type().is_ok_and(|t| t.is_dir()) {
            "/"
        } else {
            ""
        };
        Some(format!("{name}{suffix}"))
    })
    .collect()
}

impl EnvContext {
    /// cwd as displayable string.
    pub fn cwd_display(&self) -> String {
        self.cwd.to_string_lossy().into_owned()
    }
}
