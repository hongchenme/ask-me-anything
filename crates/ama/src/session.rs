//! Per-terminal session identity and the fallback transcript (ADR-002).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::{BufRead, BufWriter, Write};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionKey(String);

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Keep only characters that are unambiguous in a file name. Everything else
/// collapses to `-`, which also defeats `..` traversal from a hostile value.
fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed
    }
}

/// Pure key derivation, so precedence is testable without a terminal.
pub fn key_from(
    tmux_pane: Option<&str>,
    ama_session: Option<&str>,
    tty: Option<&str>,
) -> SessionKey {
    let pick = |prefix: &str, v: &str| SessionKey(format!("{prefix}-{}", sanitize(v)));
    if let Some(p) = tmux_pane.filter(|v| !v.is_empty()) {
        return pick("tmux", p);
    }
    if let Some(s) = ama_session.filter(|v| !v.is_empty()) {
        return pick("sh", s);
    }
    if let Some(t) = tty.filter(|v| !v.is_empty()) {
        return pick("tty", t);
    }
    SessionKey("default".to_string())
}

pub fn current_key() -> SessionKey {
    let tmux = std::env::var("TMUX_PANE").ok();
    let sh = std::env::var("AMA_SESSION").ok();
    let tty = std::fs::read_link("/proc/self/fd/2")
        .ok()
        .map(|p| p.to_string_lossy().into_owned());
    key_from(tmux.as_deref(), sh.as_deref(), tty.as_deref())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Turn {
    pub question: String,
    pub answer: String,
}

fn sessions_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".qmx2").join("sessions")
}

pub fn transcript_path(key: &SessionKey) -> PathBuf {
    sessions_dir().join(format!("{key}.jsonl"))
}

/// Missing or corrupt transcripts read as empty rather than failing a turn:
/// losing context is a nuisance, refusing to answer is a bug.
pub fn load_turns(key: &SessionKey) -> Vec<Turn> {
    let Ok(file) = std::fs::File::open(transcript_path(key)) else {
        return Vec::new();
    };
    std::io::BufReader::new(file)
        .lines()
        .map_while(Result::ok)
        .filter_map(|l| serde_json::from_str::<Turn>(&l).ok())
        .collect()
}

pub fn append_turn(key: &SessionKey, turn: &Turn) -> std::io::Result<()> {
    let dir = sessions_dir();
    std::fs::create_dir_all(&dir)?;
    let path = transcript_path(key);
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    set_owner_only(&mut opts);
    let file = opts.open(&path)?;
    let mut w = BufWriter::new(file);
    let line = serde_json::to_string(turn)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    writeln!(w, "{line}")?;
    w.flush()
}

#[cfg(unix)]
fn set_owner_only(opts: &mut std::fs::OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    opts.mode(0o600);
}

#[cfg(not(unix))]
fn set_owner_only(_opts: &mut std::fs::OpenOptions) {}

pub fn reset(key: &SessionKey) -> std::io::Result<()> {
    match std::fs::remove_file(transcript_path(key)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}
