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
