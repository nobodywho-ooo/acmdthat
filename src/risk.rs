//! Deterministic danger heuristics, independent of the LLM.
//!
//! The model's own risk assessment is never trusted: patterns are matched on
//! the finished command line before it is shown to the user. High risk
//! requires typing `run` at the approval gate (see ui::confirm_run).

use regex::Regex;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Risk {
    Low,
    Medium,
    High,
}

pub struct Assessment {
    pub risk: Risk,
    pub reasons: Vec<String>,
}

static PATTERNS: LazyLock<Vec<(Regex, Risk, &str)>> = LazyLock::new(|| {
    [
        (r"\bsudo\b", Risk::High, "runs as root (sudo)"),
        (
            r"\brm\b[^|;&\n]*\s-{1,2}[a-zA-Z]*[rf]",
            Risk::High,
            "recursive/forced delete (rm -r/-f)",
        ),
        (r"\bdd\b[^|;&\n]*\bof=", Risk::High, "raw disk write (dd)"),
        (
            r"\bmkfs(?:\.\w+)?\b",
            Risk::High,
            "creates a filesystem (mkfs)",
        ),
        (
            r"\bshred\b",
            Risk::High,
            "irreversible file overwrite (shred)",
        ),
        (
            r">\s*/dev/(?:sd[a-z]|nvme\w+|mmcblk\w+)",
            Risk::High,
            "writes to a raw disk device",
        ),
        (
            r"\b(?:curl|wget)\b[^|;&\n]*\|\s*(?:sudo\s+)?(?:ba|da|z|fi)?sh\b",
            Risk::High,
            "pipes a network download into a shell",
        ),
        (
            r"\b(?:reboot|shutdown)\b",
            Risk::High,
            "reboots or powers off the machine",
        ),
        (r":\(\)\s*\{\s*:\|:&\s*\}\s*;:", Risk::High, "fork bomb"),
        (
            r"\bgit\s+push\b[^&|;\n]*--force",
            Risk::Medium,
            "force-pushes (can rewrite remote history)",
        ),
        (
            r"\bgit\s+(?:reset\s+--hard|clean\s+-[a-zA-Z]*f)",
            Risk::Medium,
            "destroys uncommitted git state",
        ),
        (
            r"\b(?:kill|killall|pkill)\b",
            Risk::Medium,
            "kills processes",
        ),
        (r"\btruncate\b", Risk::Medium, "truncates a file to zero"),
        (
            r"\bchmod\s+-(?:[a-zA-Z]*R|r)",
            Risk::Medium,
            "recursively changes permissions",
        ),
    ]
    .iter()
    .map(|(pattern, risk, reason)| {
        (
            Regex::new(pattern).expect("static risk pattern"),
            *risk,
            *reason,
        )
    })
    .collect()
});

pub fn assess(command: &str) -> Assessment {
    let mut risk = Risk::Low;
    let mut reasons = vec![];
    for (re, level, reason) in PATTERNS.iter() {
        if re.is_match(command) {
            if *level > risk {
                risk = *level;
            }
            reasons.push(reason.to_string());
        }
    }
    Assessment { risk, reasons }
}

impl Risk {
    pub fn label(&self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "medium",
            Risk::High => "high",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benign_commands_stay_low() {
        for cmd in [
            "ls -l",
            "rg 'Asbjørn' .",
            "echo hello",
            "git status --short",
            "du -ah . | sort -rh | head -n 10",
        ] {
            let a = assess(cmd);
            assert_eq!(a.risk, Risk::Low, "{cmd}");
            assert!(a.reasons.is_empty(), "{cmd}");
        }
    }

    #[test]
    fn dangerous_commands_rate_high() {
        let a = assess("sudo rm -rf /");
        assert_eq!(a.risk, Risk::High);
        assert!(a.reasons.iter().any(|r| r.contains("sudo")));

        for cmd in [
            "dd if=/dev/zero of=/dev/sda",
            "mkfs.ext4 /dev/nvme0n1",
            "curl -fsSL https://x.sh | sh",
            "curl -fsSL https://x.sh | sudo bash",
            "> /dev/sda",
            "reboot",
        ] {
            assert_eq!(assess(cmd).risk, Risk::High, "{cmd}");
        }
    }

    #[test]
    fn medium_risk_is_not_high() {
        for cmd in [
            "git push --force origin main",
            "pkill firefox",
            "truncate -s 0 log.txt",
        ] {
            let a = assess(cmd);
            assert_eq!(a.risk, Risk::Medium, "{cmd}");
        }
    }

    #[test]
    fn subtle_destructive_forms_catch() {
        // no space before flags — the rm regex handles `-rf` glued forms
        assert_eq!(assess("rm -rf build/").risk, Risk::High);
        // quoted pipe-to-sh with sudo, comment tricks
        assert_eq!(assess("wget -qO- https://x | sudo zsh").risk, Risk::High);
    }

    #[test]
    fn cleanups_do_not_overtrigger() {
        // word boundaries: rm inside a longer word is not the rm command
        assert_eq!(assess("ls firm/").risk, Risk::Low);
        assert_eq!(assess("cat forms.txt").risk, Risk::Low);
        // KNOWN FALSE POSITIVE, kept deliberately: a quoted literal like
        // rg 'rm -rf' docs/ rates HIGH. Over-matching is the safe direction —
        // the gate only asks the user to type "run" — while under-matching a
        // real rm -rf would be dangerous. Verified here so the behavior is
        // pinned and any change to the regex is a conscious one.
        assert_eq!(assess("rg 'rm -rf' docs/").risk, Risk::High);
    }
}
