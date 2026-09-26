//! TEST-P01 and the split fixtures. The oracle is the real shell, not a
//! Rust reimplementation: we ask bash to quote a string, then ask bash to
//! parse the result, and require the round trip to be lossless.

use std::process::Command;

const BASH_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shell/ama.bash");
const ZSH_SRC: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/shell/ama.zsh");

/// Whether `zsh` is on `PATH`. Mirrors `e2e_shell.rs`'s `have("tmux")`: a
/// skip here is for machines that genuinely lack zsh (this crate's CI
/// matrix need not include it), not a way for the zsh-specific tests below
/// to pass without ever actually running zsh.
fn zsh_available() -> bool {
    Command::new("sh")
        .args(["-c", "command -v zsh >/dev/null"])
        .status()
        .is_ok_and(|s| s.success())
}

/// Quote `s` with `__ama_sq` under `shell`, let that same shell parse the
/// result, recover it. The oracle is the real shell, not a Rust
/// reimplementation, for either shell: bash's own `${1//\'/\'\\\'\'}` does
/// not port verbatim to zsh (zsh's `${...//pat/rep}` handles backslashes in
/// the replacement text differently), so `ama.zsh` uses zsh's own `(qq)`
/// quote flag instead -- a different implementation with the same
/// contract, checked the same way.
fn roundtrip_in(shell: &str, src: &str, s: &str) -> String {
    let script = format!(
        r#"source '{src}'
           q=$(__ama_sq "$1")
           eval "printf '%s' $q""#
    );
    let out = Command::new(shell)
        .args(["-c", &script, "ama-test", s])
        .output()
        .expect("spawn shell");
    assert!(out.status.success(), "{shell} failed: {out:?}");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Quote `s` with bash's `__ama_sq`, let bash parse the result, recover it.
fn roundtrip(s: &str) -> String {
    roundtrip_in("bash", BASH_SRC, s)
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

#[test]
fn zsh_quoting_round_trips_the_same_cases() {
    if !zsh_available() {
        eprintln!("skipping: zsh not installed");
        return;
    }
    for s in [
        "what's this project all about?",
        "list *.rs files | grep foo && echo $HOME",
        "rm -rf / # not really",
        "back\\slash and \"double\" and 'single'",
        "$(whoami) `id` ${PATH}",
        "newline\nin\nthe\nmiddle",
        "trailing backslash \\",
        "emoji 🤖 and CJK 日本語 and accents éàü",
        "!histexpand !!",
    ] {
        assert_eq!(roundtrip_in("zsh", ZSH_SRC, s), s, "zsh failed on {s:?}");
    }
}

#[test]
// Same domain and NUL exclusion as `roundtrips_arbitrary_strings` above
// (see there for why); this is the zsh half of TEST-P01. `ama.zsh`'s
// `__ama_sq` is a *different* implementation from `ama.bash`'s (zsh's
// `(qq)` quote flag, not a hand-rolled `${...//pat/rep}`), so it needs its
// own proof against real zsh, not an assumption that porting the bash test
// file covers it. The environment gate lives outside the `proptest!` body
// (rather than as a `prop_assume!` inside it): asserting it on every
// generated case would make proptest itself fail with "too many global
// rejects" on a zsh-less machine instead of skipping cleanly.
fn roundtrips_arbitrary_strings_zsh() {
    if !zsh_available() {
        eprintln!("skipping: zsh not installed");
        return;
    }
    proptest::proptest!(|(s in "(?s).{0,200}")| {
        proptest::prop_assume!(!s.contains('\0'));
        proptest::prop_assert_eq!(roundtrip_in("zsh", ZSH_SRC, &s), s);
    });
}

// ama.bash documents (R18, finding 1) that `eval "$(ama init bash)"` -- the
// install path this file's own header comment advertises -- breaks under a
// guard that `return`s or `exit`s from a non-interactive shell, because
// `eval`'d code is not "in a function or sourced script". The same is true
// of zsh's `return`, so `ama.zsh` uses an `if` for the same reason. This
// pins that directly: a regression back to a `return`-based guard would
// make this either print nothing (if `return`'s failure aborts the
// enclosing `eval`) or fail outright (a bare top-level `return` is a zsh
// error), so this test can actually fail, unlike a test that merely
// re-asserts what the source already says.
#[test]
fn zsh_eval_in_a_non_interactive_shell_does_not_abort_the_caller() {
    if !zsh_available() {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let script = format!(r#"eval "$(cat '{ZSH_SRC}')"; echo REACHED"#);
    let out = Command::new("zsh")
        .args(["-c", &script])
        .output()
        .expect("spawn zsh");
    assert!(out.status.success(), "zsh failed: {out:?}");
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "REACHED");
}

/// Returns Some((prefix, prompt)) when `__ama_split` recognises a trigger,
/// under `shell`.
fn split_in(shell: &str, src: &str, line: &str) -> Option<(String, String)> {
    let script = format!(r#"source '{src}'; __ama_split "$1""#);
    let out = Command::new(shell)
        .args(["-c", &script, "ama-test", line])
        .output()
        .expect("spawn shell");
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).into_owned();
    let (prefix, prompt) = s.split_once('\u{1f}')?;
    Some((prefix.to_string(), prompt.to_string()))
}

/// Returns Some((prefix, prompt)) when bash's `__ama_split` recognises a
/// trigger.
fn split(line: &str) -> Option<(String, String)> {
    split_in("bash", BASH_SRC, line)
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

// `ama.zsh`'s `__ama_split` is a straight port of the bash algorithm's
// parameter-expansion approach (see the comment there), but it is worth
// pinning against real zsh independently rather than trusting the port:
// the brief this task started from used a greedy ERE here instead
// (`^(.*[;&|][[:space:]]*)@@ (.*)$`), which makes the *last* trigger win --
// exactly backwards for `the_first_trigger_wins_even_behind_a_prefix`
// below. Every case here is one already covered for bash above; this test
// exists to prove the zsh port did not quietly regress to that greedy
// behaviour (or any other divergence), not to explore new split behaviour.
#[test]
fn zsh_split_recognises_the_same_triggers() {
    if !zsh_available() {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let split = |line: &str| split_in("zsh", ZSH_SRC, line);

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
    // Line start wins over a later operator: the prompt merely contains
    // "&& @@"; it is not a prefix.
    assert_eq!(
        split("@@ compare a && @@ b"),
        Some((String::new(), "compare a && @@ b".into()))
    );
    // The first trigger wins even behind a prefix -- the greedy-regex
    // defect this test would have caught.
    assert_eq!(
        split("clear && @@ compare a && @@ b"),
        Some(("clear && ".into(), "compare a && @@ b".into()))
    );
    assert_eq!(
        split("@@ list *.rs | grep foo"),
        Some((String::new(), "list *.rs | grep foo".into()))
    );
    assert_eq!(split("echo hello"), None);
    assert_eq!(split("@@hello"), None, "trigger requires a following space");
    assert_eq!(split("@@"), None, "bare trigger is not a prompt");
    assert_eq!(split("@@@ hello"), None);
    assert_eq!(split("echo @@ hello"), None, "not at a command position");
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
