//! End-to-end tests that drive a real interactive shell inside tmux and read
//! the pane back. This is the only layer that can catch the class of defect
//! that eliminated design approach (A) -- see 02-discovery-and-risk.md F-03.

use std::process::{Command, Stdio};
use std::time::Duration;

fn have(bin: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {bin} >/dev/null")])
        .status()
        .is_ok_and(|s| s.success())
}

/// Best-effort session teardown, used both before a fresh `new-session` (in
/// case a previous run left a same-named session behind) and on `Drop`. The
/// result is deliberately discarded -- there is nothing to react to either
/// way -- but tmux's own diagnostics (e.g. "no server running..." once the
/// last session has already gone away) are not test output and must not
/// spray across an otherwise-passing run.
fn kill_session(name: &str) {
    let _ = Command::new("tmux")
        .args(["kill-session", "-t", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

pub struct Pane {
    name: String,
    _dir: tempfile::TempDir,
}

impl Pane {
    /// Start `shell` interactively in a detached tmux session with the
    /// integration loaded and the `fake-agent-echo` fixture configured.
    fn start(name: &str, shell: &str) -> Pane {
        Self::start_with(name, shell, "fake-agent-echo", "")
    }

    /// Like `start`, but names which fixture under `tests/fixtures/` plays
    /// the agent (R16: the interrupt test needs one that is still running
    /// when Ctrl-C arrives, not the near-instant echo fixture the other
    /// tests use).
    fn start_with_agent(name: &str, shell: &str, agent: &str) -> Pane {
        Self::start_with(name, shell, agent, "")
    }

    /// Like `start`, but injects `extra_rc` into the shell's startup file
    /// immediately before the `ama init` script is sourced -- used to plant
    /// a competing `\C-m` binding ahead of time so `__ama_install`'s
    /// chaining branch (RISK-03) has something real to chain to, rather
    /// than only ever exercising its no-existing-binding default path.
    fn start_with_extra_rc(name: &str, shell: &str, extra_rc: &str) -> Pane {
        Self::start_with(name, shell, "fake-agent-echo", extra_rc)
    }

    fn start_with(name: &str, shell: &str, agent: &str, extra_rc: &str) -> Pane {
        let dir = tempfile::tempdir().expect("tempdir");
        let home = dir.path();
        let bin_dir = home.join("bin");
        std::fs::create_dir_all(&bin_dir).expect("bin dir");

        let ama = assert_cmd::cargo::cargo_bin("ama");
        std::fs::copy(&ama, bin_dir.join("ama")).expect("copy ama");
        std::fs::copy(&ama, bin_dir.join("@@")).expect("copy @@");

        let fixtures = format!("{}/tests/fixtures", env!("CARGO_MANIFEST_DIR"));
        std::fs::write(
            home.join("config.yml"),
            format!("agent:\n  command: [{fixtures}/{agent}]\n"),
        )
        .expect("config");

        let rc = home.join("rc");
        let init = Command::new(&ama)
            .args(["init", shell])
            .output()
            .expect("ama init");
        let body = format!(
            "PS1='T$ '\nexport PATH={bin}:$PATH\nexport AMA_CONFIG={home}/config.yml\n\
             export HOME={home}\n{extra_rc}{script}",
            bin = bin_dir.display(),
            home = home.display(),
            script = String::from_utf8_lossy(&init.stdout),
        );
        std::fs::write(&rc, body).expect("rc");

        let cmd = match shell {
            "zsh" => format!("ZDOTDIR={} zsh -i", home.display()),
            _ => format!("bash --rcfile {} -i", rc.display()),
        };
        if shell == "zsh" {
            std::fs::copy(&rc, home.join(".zshrc")).expect("zshrc");
        }

        kill_session(name);
        // 60 rows, not the brief's original 24: `fake-agent-echo` mirrors
        // the *entire* composed prompt back into the same pane it was
        // captured from, and each further `@@` in a multi-turn test folds
        // the previous turn's whole rendered answer into the next turn's
        // context, so real content genuinely exceeds 24 rows by the second
        // turn (verified directly: `terminal_output_from_other_commands_
        // becomes_context` failed here, its own needle scrolled off-screen
        // before ever being captured, not because context-gathering was
        // wrong). Safe to raise now that `context::trim_trailing_blank`
        // (crates/ama/src/context.rs) drops the padding a taller pane would
        // otherwise add to every capture.
        Command::new("tmux")
            .args([
                "new-session",
                "-d",
                "-s",
                name,
                "-x",
                "100",
                "-y",
                "60",
                &cmd,
            ])
            .status()
            .expect("tmux new-session");
        std::thread::sleep(Duration::from_millis(900));
        Pane {
            name: name.to_string(),
            _dir: dir,
        }
    }

    fn send(&self, keys: &str) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.name, keys, "Enter"])
            .status()
            .expect("send-keys");
    }

    fn send_raw(&self, keys: &str) {
        Command::new("tmux")
            .args(["send-keys", "-t", &self.name, keys])
            .status()
            .expect("send-keys");
    }

    /// Poll the pane until `needle` appears or the timeout expires.
    fn wait_for(&self, needle: &str, timeout: Duration) -> String {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let text = self.capture();
            if text.contains(needle) {
                return text;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {needle:?}\n--- pane ---\n{text}");
            }
            std::thread::sleep(Duration::from_millis(120));
        }
    }

    fn capture(&self) -> String {
        let out = Command::new("tmux")
            .args(["capture-pane", "-t", &self.name, "-p"])
            .output()
            .expect("capture-pane");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        kill_session(&self.name);
    }
}

macro_rules! require_tmux {
    () => {
        if !have("tmux") {
            eprintln!("skipping: tmux not installed");
            return;
        }
    };
}

#[test]
fn the_readme_first_example_works_verbatim() {
    require_tmux!();
    let p = Pane::start("ama-e2e-readme", "bash");
    p.send("@@ what's this project all about?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("what's this project all about?"),
        "the apostrophe must survive the round trip\n{pane}"
    );
}

#[test]
fn shell_metacharacters_reach_the_agent_untouched() {
    require_tmux!();
    let p = Pane::start("ama-e2e-meta", "bash");
    p.send("@@ list *.rs and $HOME | grep foo && done?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("list *.rs and $HOME | grep foo && done?"),
        "{pane}"
    );
}

// `count() >= 2` looked like it required both the on-screen text and a
// `history` entry, but "remember me" is already visible 3 times right
// after turn 1 answers (the executed line plus its echoed-back rendering)
// -- verified directly: capturing the pane before ever sending "history 3"
// already shows a count of 3. So the original assertion could not fail
// regardless of whether `history` recorded anything at all. `clear` first,
// to remove every on-screen copy, so a later reappearance can only be
// explained by `history 3` actually listing the command.
#[test]
fn the_question_stays_on_screen_and_in_history() {
    require_tmux!();
    let p = Pane::start("ama-e2e-history", "bash");
    p.send("@@ remember me");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("remember me"),
        "the question must remain visible on screen\n{pane}"
    );
    p.send("clear");
    std::thread::sleep(Duration::from_millis(300));
    let cleared = p.capture();
    assert!(
        !cleared.contains("remember me"),
        "sanity check: clear should have wiped the pane\n{cleared}"
    );
    p.send("history 3");
    let pane = p.wait_for("remember me", Duration::from_secs(5));
    assert!(
        pane.contains("remember me"),
        "the question must be recorded in shell history\n{pane}"
    );
}

#[test]
fn ordinary_commands_are_unaffected() {
    require_tmux!();
    let p = Pane::start("ama-e2e-passthrough", "bash");
    p.send("echo NORMAL-OK");
    p.wait_for("NORMAL-OK", Duration::from_secs(5));
    p.send("for i in 1 2; do echo L$i; done");
    let pane = p.wait_for("L2", Duration::from_secs(5));
    assert!(pane.contains("L1") && pane.contains("L2"), "{pane}");
}

// `wait_for("joke of the day", ..)` alone would race: that exact text also
// appears, unindented, in the raw command echo the moment the rewritten
// line is submitted -- before `clear` (sequenced first via `&&`) has even
// run, let alone before turn 2's own context capture and agent round trip
// complete (verified by mutation, see below).
//
// Waiting for "🤖:" fixes the "too early" half (that marker is written only
// by `Robot`, never present in a raw typed line) but a naive second
// `wait_for("🤖:", ..)` can still race the *other* way: `clear` genuinely
// wipes turn 1's own "🤖:" from the pane, so the two can never be visible
// at once, but if `wait_for`'s very first poll lands before `clear` has
// run, it finds turn 1's still-present "🤖:" and returns immediately,
// having proven nothing about turn 2. A short settle first -- comfortably
// longer than `clear` (a trivial, near-instant subprocess) needs, much
// shorter than the full second-turn round trip -- removes that window, so
// the subsequent `wait_for("🤖:", ..)` can only be satisfied by turn 2's
// real answer.
//
// What this test can and cannot prove, run against a pane-height fixed at
// 24 rows: with the original racy needle, deleting the clear-triggered
// `ama session reset` call from `__ama_hook` failed 4 of 6 runs (a real
// signal, but not a reliable one). With the race fixed above, the same
// mutation now passes 6 of 6 -- because inside tmux, `clear` (the real
// terminal command) already wipes the pane `context::gather` reads
// regardless of that call, so this test structurally cannot tell "the call
// fired" apart from "the terminal did the work on its own". The call's own
// wiring is instead pinned directly, without a pty, in
// `shell_functions.rs`'s `calls_session_reset_for` tests.
#[test]
fn clear_and_trigger_on_one_line_starts_a_fresh_conversation() {
    require_tmux!();
    let p = Pane::start("ama-e2e-clear", "bash");
    p.send("@@ first question");
    p.wait_for("🤖:", Duration::from_secs(10));
    p.send("clear && @@ show me the joke of the day");
    std::thread::sleep(Duration::from_millis(300));
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("joke of the day"), "{pane}");
    assert!(
        !pane.contains("first question"),
        "clear must drop the earlier turn (REQ-13, REQ-28)\n{pane}"
    );
}

// Neither "No such file"/"cannot access" nor "why did that fail" prove
// anything by themselves: the `ls` failure is already sitting on screen
// from the command that produced it, and "why did that fail" is already
// inline in the rewritten command's own echo -- both visible before the
// agent could possibly have responded. Verified by mutation: forcing
// `context::gather` to always return an empty `Vec` (context-gathering
// completely disabled) still left the original single-occurrence checks
// green.
//
// A first fix counting occurrences of "nonexistent-marker-xyz" instead was
// *also* non-discriminating, just for a sillier reason caught by re-running
// the same mutation: GNU `ls`'s own error line is
// "ls: cannot access '/nonexistent-marker-xyz': No such file or directory",
// so the marker filename already appears twice from the `ls` invocation
// alone (once typed, once in its error) with no agent involved at all.
// "No such file" has no such problem: it appears in the error text exactly
// once, never in the typed command, so a genuine *second* occurrence can
// only be explained by `fake-agent-echo` mirroring back a composed prompt
// that actually included the failure as context.
#[test]
fn terminal_output_from_other_commands_becomes_context() {
    require_tmux!();
    let p = Pane::start("ama-e2e-context", "bash");
    p.send("@@ start the conversation");
    p.wait_for("🤖:", Duration::from_secs(10));
    p.send("ls /nonexistent-marker-xyz");
    p.wait_for("nonexistent-marker-xyz", Duration::from_secs(5));
    p.send("@@ why did that fail");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let pane = loop {
        let text = p.capture();
        if text.matches("No such file").count() >= 2 {
            break text;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "timed out waiting for the ls failure to be echoed back as context\n--- pane ---\n{text}"
            );
        }
        std::thread::sleep(Duration::from_millis(120));
    };
    assert!(
        pane.contains("No such file") || pane.contains("cannot access"),
        "the failure output must be visible as context\n{pane}"
    );
}

/// R16 (supersedes R7): the fast echo agent used by every other test above
/// almost certainly exits before Ctrl-C is ever sent, so it can only prove
/// "the shell survived", not "Ctrl-C interrupted a running agent" -- and
/// `130` is otherwise unobtainable from any test in this suite, since
/// `std::process::ExitStatus::code()` returns `None` for a signal death and
/// a real interactive shell is the only thing that computes 128+signal into
/// `$?`. `fake-agent-slow` prints, then sleeps 3s, then prints again, so
/// waiting for its first line of output before sending Ctrl-C guarantees the
/// interrupt lands mid-sleep, with both `ama` and the agent still alive and
/// in the terminal's foreground process group.
#[test]
fn an_interrupt_returns_a_usable_shell() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-interrupt", "bash", "fake-agent-slow");
    p.send("@@ slow question");
    // Proves the agent is actually mid-flight (past `cat`, past its first
    // `echo`, inside the 3s `sleep`) rather than merely "sent".
    p.wait_for("🤖: first", Duration::from_secs(5));
    std::thread::sleep(Duration::from_millis(600));
    p.send_raw("C-c");
    std::thread::sleep(Duration::from_millis(400));
    p.send("echo rc=$?");
    let pane = p.wait_for("rc=", Duration::from_secs(5));
    assert!(
        pane.contains("rc=130"),
        "NFR-05's fourth exit code (130, interrupted) must reach $?\n{pane}"
    );
    // The interrupt must not wedge the shell: it keeps taking commands.
    p.send("echo SHELL-ALIVE");
    p.wait_for("SHELL-ALIVE", Duration::from_secs(5));
}

// `ask`'s own empty-question check (cli.rs) means the agent never runs
// either way, even if `__ama_hook`'s blank-prompt branch were deleted --
// verified by mutation: removing that branch left this test's original
// single assertion (no "🤖:") passing, because a deleted buffer would
// rewrite READLINE_LINE to `@@ ''`, which still reaches the agent as an
// empty (whitespace-only) question and is turned away one layer down. The
// two checks below pin the *bash-side* half of REQ-06 specifically: a
// cleared `READLINE_LINE` never becomes a shell command at all, so bash
// records nothing to history and the prompt comes back with nothing typed.
#[test]
fn a_bare_trigger_does_not_call_the_agent() {
    require_tmux!();
    let p = Pane::start("ama-e2e-bare", "bash");
    p.send("echo BEFORE-MARKER");
    p.wait_for("BEFORE-MARKER", Duration::from_secs(5));
    p.send("@@ ");
    std::thread::sleep(Duration::from_millis(700));
    assert!(
        !p.capture().contains("🤖:"),
        "empty prompt must be a no-op (REQ-06)"
    );
    p.send("history 2");
    let pane = p.wait_for("BEFORE-MARKER", Duration::from_secs(5));
    assert!(
        !pane.contains("@@"),
        "a blank prompt must clear the buffer, not submit `@@ ''`\n{pane}"
    );
}

// `wait_for("literal", ..)` looks safe but is not: "literal" appears the
// instant the typed line is echoed back by the terminal, whether or not
// `@@` gets dispatched, because a hijacked rewrite (`@@ 'literal'`) still
// contains the word "literal" inside its own quoting. That made the
// original version of this test a race rather than a proof -- verified by
// mutation: forcing `__ama_split` to treat "echo @@ literal" as a genuine
// trigger (dropping the "echo " prefix, so the rewritten line becomes a
// real `@@ 'literal'` dispatch) still left the old assertion green, because
// `wait_for` returned on the command echo well before the agent could have
// produced any "🤖:" output. Settling for a fixed delay before capturing
// (long enough for `fake-agent-echo` to have finished if it had wrongly
// been dispatched) closes that gap; re-running the same mutation against
// this version fails it correctly.
#[test]
fn a_trigger_that_is_not_at_a_command_position_is_left_alone() {
    require_tmux!();
    let p = Pane::start("ama-e2e-notrigger", "bash");
    p.send("echo @@ literal");
    std::thread::sleep(Duration::from_millis(1500));
    let pane = p.capture();
    assert!(
        pane.contains("literal"),
        "the echo itself must still run\n{pane}"
    );
    assert!(
        !pane.contains("🤖:"),
        "must not hijack a real command\n{pane}"
    );
}

#[test]
fn a_trigger_without_a_following_space_is_not_a_trigger() {
    require_tmux!();
    let p = Pane::start("ama-e2e-nospace", "bash");
    p.send("@@nosuchcommand");
    let pane = p.wait_for("command not found", Duration::from_secs(5));
    assert!(!pane.contains("🤖:"), "{pane}");
}

#[test]
fn leading_whitespace_still_triggers() {
    require_tmux!();
    let p = Pane::start("ama-e2e-indent", "bash");
    p.send("   @@ indented question");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("indented question"), "{pane}");
}

/// RISK-03 / "things that will bite you": `bind -s` output parsing for the
/// existing-binding chain is fiddly enough that it needs its own proof, not
/// just an assumption that the `if` branch in `__ama_install` is correct.
/// None of the tests above exercise it -- with no prior `\C-m` binding,
/// `__ama_install` always takes the `else` branch, which is only the
/// default case. This plants a real pre-existing `\C-m` macro (as some
/// other tool might) *before* `ama init bash`'s trailing `__ama_install`
/// call runs, then proves it survived by pressing Enter on an empty line:
/// if the old binding had been clobbered instead of chained, nothing would
/// ever run it and the marker would never appear.
#[test]
fn an_existing_c_m_binding_is_chained_not_clobbered() {
    require_tmux!();
    let probe = "bind '\"\\C-m\": \"echo CHAIN-PROBE-RAN\\C-j\"'\n";
    let p = Pane::start_with_extra_rc("ama-e2e-chain", "bash", probe);
    p.send_raw("Enter");
    let pane = p.wait_for("CHAIN-PROBE-RAN", Duration::from_secs(5));
    assert!(
        pane.contains("CHAIN-PROBE-RAN"),
        "a pre-existing \\C-m binding must be chained, not clobbered\n{pane}"
    );
}
