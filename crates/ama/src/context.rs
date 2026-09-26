//! Where the conversation's context comes from (ADR-002).

use crate::session::{self, SessionKey};
use crate::spinner;

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

/// Indices of the *command* lines bearing the trigger, answer lines skipped.
fn trigger_positions(lines: &[String]) -> impl Iterator<Item = usize> + '_ {
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !is_answer_line(l) && l.contains(TRIGGER_IN_LINE))
        .map(|(i, _)| i)
}

/// The pane content this turn should send (REQ-11, as amended by **A-04**).
///
/// R23. "From the first trigger line to the bottom" is right only once a
/// conversation is already on screen. By the time `ama` captures the pane
/// the current question has *already been echoed onto it*, so when that
/// question is the only trigger, slicing from "the first trigger" keeps
/// exactly one line -- the question -- and throws away the whole screen
/// above it, including the failing command the user is asking about. That
/// is REQ-11's own acceptance criterion (`ls /nonexistent`, then `@@ …`)
/// and ADR-002's stated reason for scraping the pane at all: "the
/// difference between answering 'why did that build fail?' and not".
///
/// So the rule is by *prior* conversation, not by first trigger:
///
/// * **Two or more triggers** -- a prior turn is on screen. Start at the
///   first of them, exactly as before, so the conversation has a boundary
///   and a cleared screen still starts fresh (REQ-13).
/// * **Exactly one** -- it is this turn's own echo, with nothing before it.
///   Send the whole visible pane.
/// * **None** -- e.g. `clear && @@ …`, where `clear` has already wiped the
///   echo. Send nothing; REQ-13 wants a fresh conversation.
///
/// Note the widening only ever *adds* already-visible screen content, which
/// RISK-01 (accepted by the owner) covers and `--no-context` opts out of.
pub fn select_context(lines: &[String]) -> &[String] {
    let mut triggers = trigger_positions(lines);
    match (triggers.next(), triggers.next()) {
        (Some(_), Some(_)) => slice_from_first_trigger(lines),
        (Some(_), None) => lines,
        _ => &[],
    }
}

/// Drop the run of blank rows at the very end of a pane capture.
///
/// `tmux capture-pane -p` and `screen -X hardcopy` both pad their dump out to
/// the pane's full height: every row below the cursor that has never been
/// written to comes back as an empty line. Left alone, that padding becomes
/// part of the "context" `slice_from_first_trigger` returns whenever the
/// trigger line is not the pane's last line -- which is the common case
/// right after `clear`, or on the very first `@@` in a fresh pane -- bloating
/// every turn with dozens of contentless lines. Only a real terminal
/// exhibits this padding; `slice_from_first_trigger`'s own unit tests all
/// use hand-built fixtures that stop exactly at the last real line, so nothing
/// caught it until Task 7's tmux end-to-end suite exercised a real capture.
/// Trimming here, before slicing, is simplest: `slice_from_first_trigger`
/// keeps its existing "start-to-end-of-slice" contract unchanged.
pub fn trim_trailing_blank(lines: &[String]) -> &[String] {
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    &lines[..end]
}

/// Take the moon's leftovers out of a screen capture (RISK-18).
///
/// A frame stranded by the user's own keys -- Enter pressed while the moon
/// was up, or Ctrl-C -- is still on screen when the next turn captures it.
/// Left in, it would reach the agent as if the user had typed it. A line
/// that is only a frame is dropped. A frame followed by something else keeps
/// the something else: `^C`, or keys the terminal echoed onto its line.
pub fn without_moon_frames(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .filter_map(|line| match spinner::strip_frame(&line) {
            None => Some(line),
            Some(rest) if rest.trim().is_empty() => None,
            Some(rest) => Some(rest.to_string()),
        })
        .collect()
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
        Some(pane) => select_context(trim_trailing_blank(&without_moon_frames(pane))).to_vec(),
        None => from_transcript(key),
    };
    cap(&lines, max).to_vec()
}

#[cfg(test)]
mod tests {
    // `decode_lines` stays private -- captured pane bytes are never valid
    // UTF-8 by construction (progress bars, odd file names, partial escape
    // sequences can all clip a multi-byte character), so the no-panic
    // property (Review Focus 3) is pinned here directly rather than only
    // resting on `from_utf8_lossy`'s documented guarantee.
    use super::decode_lines;

    #[test]
    fn a_lone_continuation_byte_is_replaced_without_panicking() {
        // 0x80 is a continuation byte with no leading byte -- invalid on its
        // own, a single-byte maximal invalid subpart.
        let got = decode_lines(b"before\x80after\n");
        assert_eq!(got.len(), 1, "got {got:?}");
        assert_eq!(got[0], "before\u{FFFD}after");
    }

    #[test]
    fn a_truncated_multibyte_sequence_is_replaced_without_panicking() {
        // 0xE2 0x82 is the first two bytes of '€' (U+20AC, 3 bytes); cut
        // short and followed by an ASCII byte that cannot extend it.
        let got = decode_lines(b"mid\xE2\x82end\n");
        assert_eq!(got.len(), 1, "got {got:?}");
        assert_eq!(got[0], "mid\u{FFFD}end");
    }

    #[test]
    fn an_interrupted_escape_sequence_around_an_invalid_byte_does_not_panic() {
        // An ANSI CSI sequence cut off before its final byte (no `m`),
        // immediately followed by 0xFF, which is never valid UTF-8 in any
        // position. The escape bytes are plain ASCII, so they survive
        // decoding as literal (non-printable) characters, same as a real
        // captured pane would carry them.
        let got = decode_lines(b"before\x1b[31\xFFafter\n");
        assert_eq!(got.len(), 1, "got {got:?}");
        assert_eq!(got[0], "before\u{1b}[31\u{FFFD}after");
    }

    #[test]
    fn the_final_line_survives_with_no_trailing_newline() {
        let got = decode_lines(b"first\nsecond line no newline\xFF");
        assert_eq!(got.len(), 2, "got {got:?}");
        assert_eq!(got[0], "first");
        assert_eq!(got[1], "second line no newline\u{FFFD}");
    }
}
