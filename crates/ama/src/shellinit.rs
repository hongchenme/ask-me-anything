//! Shell integration scripts, embedded in the binary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
}

impl Shell {
    pub fn parse(s: &str) -> Option<Self> {
        match s.rsplit('/').next().unwrap_or(s) {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            _ => None,
        }
    }
}

pub fn script(shell: Shell) -> &'static str {
    match shell {
        Shell::Bash => include_str!("shell/ama.bash"),
        Shell::Zsh => include_str!("shell/ama.zsh"),
    }
}
