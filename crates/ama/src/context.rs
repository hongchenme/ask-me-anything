//! Where the conversation's context comes from (ADR-002).

use crate::session::{self, SessionKey};

/// Rendered answers are marked so that slicing can tell the agent's words
/// apart from the user's commands.
pub const ANSWER_PREFIX: &str = "🤖: ";
pub const CONT_INDENT: &str = "   ";

const TRIGGER_IN_LINE: &str = "@@ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Tmux,
    Screen,
    Transcript,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Source::Tmux => "tmux pane",
            Source::Screen => "screen window",
            Source::Transcript => "transcript (no multiplexer)",
        })
    }
}

pub fn detect_source() -> Source {
    let set = |k: &str| std::env::var(k).is_ok_and(|v| !v.is_empty());
    if set("TMUX") {
        Source::Tmux
    } else if set("STY") {
        Source::Screen
    } else {
        Source::Transcript
    }
}

/// True when the line is part of a rendered answer rather than a command.
fn is_answer_line(line: &str) -> bool {
    line.trim_start().starts_with(ANSWER_PREFIX.trim_end()) || line.starts_with(CONT_INDENT)
}

/// Everything from the first *command* line bearing the trigger to the end.
/// Answer lines are skipped during the search (Review Focus 2).
pub fn slice_from_first_trigger(lines: &[String]) -> &[String] {
    let start = lines
        .iter()
        .position(|l| !is_answer_line(l) && l.contains(TRIGGER_IN_LINE));
    match start {
        Some(i) => &lines[i..],
        None => &[],
    }
}

/// Keep the most recent `max` lines (NFR-02).
pub fn cap(lines: &[String], max: usize) -> &[String] {
    if lines.len() <= max {
        lines
    } else {
        &lines[lines.len() - max..]
    }
}

fn capture_tmux() -> Option<Vec<String>> {
    let out = std::process::Command::new("tmux")
        .args(["capture-pane", "-p"])
        .output()
        .ok()?;
    out.status.success().then(|| decode_lines(&out.stdout))
}

fn capture_screen() -> Option<Vec<String>> {
    let dir = std::env::temp_dir().join(format!("ama-hardcopy-{}", std::process::id()));
    let ok = std::process::Command::new("screen")
        .args(["-X", "hardcopy", &dir.to_string_lossy()])
        .status()
        .ok()?
        .success();
    let text = ok.then(|| std::fs::read(&dir).ok()).flatten();
    let _ = std::fs::remove_file(&dir);
    text.map(|b| decode_lines(&b))
}

/// Pane output is arbitrary bytes: progress bars, odd file names, partial
/// escape sequences. Decode lossily rather than failing the turn
/// (Review Focus 3).
fn decode_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.trim_end().to_string())
        .collect()
}

fn from_transcript(key: &SessionKey) -> Vec<String> {
    let mut out = Vec::new();
    for t in session::load_turns(key) {
        out.push(format!("$ @@ '{}'", t.question));
        for (i, l) in t.answer.lines().enumerate() {
            out.push(if i == 0 {
                format!("{ANSWER_PREFIX}{l}")
            } else {
                format!("{CONT_INDENT}{l}")
            });
        }
    }
    out
}

/// Gather context for this turn, falling back silently when a capture fails
/// (RISK-05 surfaces the active source through `ama doctor` instead).
pub fn gather(source: Source, key: &SessionKey, max: usize) -> Vec<String> {
    let captured = match source {
        Source::Tmux => capture_tmux(),
        Source::Screen => capture_screen(),
        Source::Transcript => None,
    };
    let lines = match captured {
        Some(pane) => slice_from_first_trigger(&pane).to_vec(),
        None => from_transcript(key),
    };
    cap(&lines, max).to_vec()
}
