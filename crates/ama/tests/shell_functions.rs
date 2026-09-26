//! TEST-P01 and the split fixtures. The oracle is the real shell, not a
//! Rust reimplementation: we ask bash to quote a string, then ask bash to
//! parse the result, and require the round trip to be lossless.

use std::process::Command;

const BASH_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shell/ama.bash");

/// Quote `s` with `__ama_sq`, let the shell parse the result, recover it.
fn roundtrip(s: &str) -> String {
    let script = format!(
        r#"source '{BASH_SRC}'
           q=$(__ama_sq "$1")
           eval "printf '%s' $q""#
    );
    let out = Command::new("bash")
        .args(["-c", &script, "ama-test", s])
        .output()
        .expect("spawn bash");
    assert!(out.status.success(), "bash failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn roundtrips_the_readme_example() {
    let s = "what's this project all about?";
    assert_eq!(roundtrip(s), s);
}

#[test]
fn roundtrips_shell_metacharacters_literally() {
    for s in [
        "list *.rs files | grep foo && echo $HOME",
        "rm -rf / # not really",
        "back\\slash and \"double\" and 'single'",
        "$(whoami) `id` ${PATH}",
        "newline\nin\nthe\nmiddle",
        "trailing backslash \\",
        "emoji 🤖 and CJK 日本語 and accents éàü",
        "!histexpand !!",
    ] {
        assert_eq!(roundtrip(s), s, "failed to round-trip {s:?}");
    }
}

proptest::proptest! {
    #[test]
    // (?s) makes `.` match newlines too (regex-syntax defaults to excluding
    // them). Newlines are reachable input here — bracketed paste delivers
    // them, see survives_an_embedded_newline_from_bracketed_paste — so the
    // swept domain must include them.
    fn roundtrips_arbitrary_strings(s in "(?s).{0,200}") {
        // NUL cannot occur here: it cannot be a process argument (execve
        // requires NUL-terminated C strings — std::process::Command
        // rejects it before bash even runs) and it cannot live in a bash
        // variable either (bash's own strings are NUL-terminated C
        // strings), so a typed line can never carry one. Excluded from
        // the domain because it is not a reachable input, not because
        // `__ama_sq` mishandles it.
        proptest::prop_assume!(!s.contains('\0'));
        proptest::prop_assert_eq!(roundtrip(&s), s);
    }
}

/// Returns Some((prefix, prompt)) when `__ama_split` recognises a trigger.
fn split(line: &str) -> Option<(String, String)> {
    let script = format!(r#"source '{BASH_SRC}'; __ama_split "$1""#);
    let out = Command::new("bash")
        .args(["-c", &script, "ama-test", line])
        .output()
        .expect("spawn bash");
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    let (prefix, prompt) = s.split_once('\u{1f}')?;
    Some((prefix.to_string(), prompt.to_string()))
}

#[test]
fn recognises_triggers_at_command_positions() {
    assert_eq!(split("@@ hello"), Some((String::new(), "hello".into())));
    assert_eq!(split("   @@ hello"), Some(("   ".into(), "hello".into())));
    assert_eq!(
        split("clear && @@ show me the joke of the day"),
        Some(("clear && ".into(), "show me the joke of the day".into()))
    );
    assert_eq!(
        split("cd /tmp; @@ where am i"),
        Some(("cd /tmp; ".into(), "where am i".into()))
    );
}

#[test]
fn line_start_wins_over_a_later_operator() {
    // The prompt merely contains "&& @@"; it is not a prefix.
    assert_eq!(
        split("@@ compare a && @@ b"),
        Some((String::new(), "compare a && @@ b".into()))
    );
}

#[test]
fn the_first_trigger_wins_even_behind_a_prefix() {
    assert_eq!(
        split("clear && @@ compare a && @@ b"),
        Some(("clear && ".into(), "compare a && @@ b".into()))
    );
}

#[test]
fn a_prompt_containing_operators_is_not_split() {
    assert_eq!(
        split("@@ list *.rs | grep foo"),
        Some((String::new(), "list *.rs | grep foo".into()))
    );
}

#[test]
fn rejects_non_triggers() {
    assert_eq!(split("echo hello"), None);
    assert_eq!(split("@@hello"), None, "trigger requires a following space");
    assert_eq!(split("@@"), None, "bare trigger is not a prompt");
    assert_eq!(split("@@@ hello"), None);
    assert_eq!(split("echo @@ hello"), None, "not at a command position");
}

#[test]
fn survives_an_embedded_newline_from_bracketed_paste() {
    assert_eq!(
        split("@@ first line\nsecond line"),
        Some((String::new(), "first line\nsecond line".into()))
    );
}

/// Whether `__ama_hook` calls `ama session reset` for a given `READLINE_LINE`.
///
/// A real terminal cannot observe this directly: `clear` itself already
/// wipes any pane-based context regardless of whether this call happens, so
/// Task 7's tmux end-to-end suite has no way to tell "the call fired" apart
/// from "the terminal's own `clear` did all the work" -- confirmed by
/// mutation there (deleting the call left `e2e_shell.rs`'s
/// `clear_and_trigger_on_one_line_starts_a_fresh_conversation` passing
/// 6 times out of 6, unchanged). This pins the wiring directly instead: a
/// stub `ama` shell function shadows the real binary and records whether it
/// was ever invoked with `session reset`. `__ama_hook` and `__ama_install`
/// (called unconditionally when the file is sourced) both need readline
/// machinery, hence `bash -i` -- verified separately to work with piped,
/// non-tty input; the only cost is two harmless "no job control" warnings
/// on stderr, discarded here like the round-trip helpers above discard
/// unrelated stderr.
///
/// R18 finding 5: an earlier version of this helper built the log path from
/// `pid + line.len()`, and two call sites (`"clear && @@ fresh start"`,
/// `"reset && @@ fresh start"`) are both 23 characters -- so, in the same
/// process, they collided on one shared file in the system temp directory.
/// Only the whole-suite run is mandated single-threaded; `cargo test --test
/// shell_functions` on its own runs in parallel by default, where one
/// test's `remove_file` at entry could delete the other's still-unread
/// record between its `bash` run and its `read_to_string`. `tempfile::
/// tempdir()` gives every call its own uniquely-named directory (already a
/// dev-dependency, already used the same way in `e2e_shell.rs`), and its
/// `Drop` cleans it up automatically -- no manual `remove_file` needed.
fn calls_session_reset_for(line: &str) -> bool {
    let dir = tempfile::tempdir().expect("tempdir");
    let log = dir.path().join("session-reset.log");
    let script = format!(
        r#"ama() {{ printf '%s\n' "$*" >> '{log}'; }}
           source '{BASH_SRC}'
           READLINE_LINE=$1
           READLINE_POINT=${{#READLINE_LINE}}
           __ama_hook"#,
        log = log.display(),
    );
    let out = Command::new("bash")
        .args(["-i", "-c", &script, "ama-test", line])
        .output()
        .expect("spawn bash");
    assert!(
        out.status.success(),
        "bash failed: {out:?}\nstderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(&log).is_ok_and(|s| s.contains("session reset"))
}

#[test]
fn clearing_the_screen_before_a_trigger_resets_the_session() {
    assert!(calls_session_reset_for("clear && @@ fresh start"));
}

#[test]
fn resetting_the_screen_before_a_trigger_also_resets_the_session() {
    assert!(calls_session_reset_for("reset && @@ fresh start"));
}

#[test]
fn an_ordinary_trigger_does_not_reset_the_session() {
    assert!(!calls_session_reset_for("@@ carry on"));
}

#[test]
fn a_bare_clear_with_no_trigger_also_resets_the_session() {
    // The non-trigger fallback branch of `__ama_hook` (a plain typed
    // `clear`, or Ctrl-L, not part of any `@@` line) must reset too,
    // independent of the trigger-rewrite branch above.
    assert!(calls_session_reset_for("clear"));
}

#[test]
fn an_ordinary_command_does_not_reset_the_session() {
    assert!(!calls_session_reset_for("echo hi"));
}
