//! End-to-end tests that drive a real interactive shell inside tmux and read
//! the pane back. This is the only layer that can catch the class of defect
//! that eliminated design approach (A) -- see 02-discovery-and-risk.md F-03.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

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
    home: tempfile::TempDir,
}

impl Pane {
    /// Start `shell` interactively in a detached tmux session with the
    /// integration loaded and the `fake-agent-echo` fixture configured.
    fn start(name: &str, shell: &str) -> Pane {
        Self::start_with(name, shell, "fake-agent-echo", "", "")
    }

    /// Like `start`, but names which fixture under `tests/fixtures/` plays
    /// the agent (R16: the interrupt test needs one that is still running
    /// when Ctrl-C arrives, not the near-instant echo fixture the other
    /// tests use).
    fn start_with_agent(name: &str, shell: &str, agent: &str) -> Pane {
        Self::start_with(name, shell, agent, "", "")
    }

    /// Like `start_with_agent`, with `extra_config` appended to the config
    /// file as top-level keys -- e.g. `spinner: false`.
    fn start_with_config(name: &str, shell: &str, agent: &str, extra_config: &str) -> Pane {
        Self::start_with(name, shell, agent, "", extra_config)
    }

    /// Like `start`, but injects `extra_rc` into the shell's startup file
    /// immediately before the `ama init` script is sourced -- used to plant
    /// a competing `\C-m` binding ahead of time so `__ama_install`'s
    /// chaining branch (RISK-03) has something real to chain to, rather
    /// than only ever exercising its no-existing-binding default path.
    fn start_with_extra_rc(name: &str, shell: &str, extra_rc: &str) -> Pane {
        Self::start_with(name, shell, "fake-agent-echo", extra_rc, "")
    }

    fn start_with(
        name: &str,
        shell: &str,
        agent: &str,
        extra_rc: &str,
        extra_config: &str,
    ) -> Pane {
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
            format!("agent:\n  command: [{fixtures}/{agent}]\n{extra_config}"),
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
            // `-d` (--no-globalrcs) skips /etc/zsh/*, not $ZDOTDIR/.zshrc.
            //
            // ZDOTDIR does not isolate a test from the *global* startup
            // files, and Debian/Ubuntu's /etc/zsh/zshrc runs `compinit`.
            // On a GitHub runner the completion directories are
            // group-writable, so compinit emits
            // `zsh compinit: insecure directories` and waits -- swallowing
            // the keys the harness then sends, so every zsh test times out.
            // This machine has no /etc/zsh at all, which is why it only
            // showed up in CI.
            //
            // Isolating is also the right default for these tests: they
            // exist to prove *our* hook works in a real interactive zsh,
            // not to re-test whatever a distro puts in its global rc.
            // Coexistence with someone else's widget is covered separately
            // by the chaining test.
            "zsh" => format!("ZDOTDIR={} zsh -d -i", home.display()),
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
            home: dir,
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

    /// Poll the pane until `needle` is gone from it, or the timeout expires.
    fn wait_until_gone(&self, needle: &str, timeout: Duration) {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let text = self.capture();
            if !text.contains(needle) {
                return;
            }
            if std::time::Instant::now() > deadline {
                panic!("timed out waiting for {needle:?} to leave the pane\n--- pane ---\n{text}");
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Poll until Ctrl-L has visibly finished: `gone` has left the scrollback
    /// and `kept` has been redrawn on screen. Returns the last (screen,
    /// scrollback) pair either way, for the caller to assert on -- a fixed
    /// pause here once failed 1 run in 20 under load (0.1.2 D-07).
    fn wait_for_clear(&self, gone: &str, kept: &str, timeout: Duration) -> (String, String) {
        let deadline = Instant::now() + timeout;
        loop {
            let (screen, history) = (self.capture(), self.capture_history(500));
            if (!history.contains(gone) && screen.contains(kept)) || Instant::now() > deadline {
                return (screen, history);
            }
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Every transcript `ama` has written under this pane's `$HOME`.
    fn transcripts(&self) -> String {
        let dir = self.home.path().join(".ama").join("sessions");
        let mut all = String::new();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return all;
        };
        for entry in entries.flatten() {
            all.push_str(&std::fs::read_to_string(entry.path()).expect("transcript"));
        }
        all
    }

    fn capture(&self) -> String {
        let out = Command::new("tmux")
            .args(["capture-pane", "-t", &self.name, "-p"])
            .output()
            .expect("capture-pane");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Like `capture`, but including up to `lines` of scrollback history
    /// (`-S -N`), not just what is currently on screen. Used to tell
    /// "the visible screen was cleared" apart from "the terminal's
    /// scrollback was actually dropped, not just scrolled past".
    fn capture_history(&self, lines: i32) -> String {
        let out = Command::new("tmux")
            .args([
                "capture-pane",
                "-t",
                &self.name,
                "-p",
                "-S",
                &format!("-{lines}"),
            ])
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

/// REQ-02 / RISK-02, and the one test that pins the *composition* of the
/// hook with `__ama_sq` rather than `__ama_sq` alone.
///
/// R27: the obvious needle -- the question text anywhere on the pane -- is
/// satisfied by the terminal's own echo of the typed line, before the agent
/// is involved at all, so this test could not fail for the property it
/// names. Verified by mutation: replacing `$(__ama_sq "$prompt")` in a copy
/// of `ama.bash` with a plain `"$prompt"` left it green while the agent
/// actually received `$HOME` *expanded*; for `@@ what does $(id -un) mean?`
/// the same mutant had the user's own shell run a command out of a typed
/// question and hand the agent `what does hong mean?`. That is RISK-02's
/// exact failure mode. TEST-P01 does not cover it either -- it exercises
/// `__ama_sq` in isolation, so a hook that stops *calling* it passes every
/// property test.
///
/// Counting occurrences does not discriminate: `>= 2` passes on the mutant
/// too (the echo, plus the context block's own copy of that echo). What
/// does is the `## Question` body as `fake-agent-echo` mirrors it back --
/// a `CONT_INDENT` continuation line carrying the question and nothing
/// else. On the mutant that line shows the expanded path; only a hook that
/// really quotes reproduces the typed bytes there.
#[test]
fn shell_metacharacters_reach_the_agent_untouched() {
    require_tmux!();
    const QUESTION: &str = "list *.rs and $HOME | grep foo && done?";
    let p = Pane::start("ama-e2e-meta", "bash");
    p.send(&format!("@@ {QUESTION}"));
    // Not `wait_for("🤖:")`: the robot marker lands on the *first* line the
    // agent streams, and `## Question` is the last section of the composed
    // prompt, so the needle below is what has to be waited for.
    let needle = format!("\n{}{QUESTION}", ama::context::CONT_INDENT);
    let pane = p.wait_for(&needle, Duration::from_secs(10));
    assert!(
        pane.contains(&needle),
        "the question must reach the agent byte-for-byte\n{pane}"
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
//
// That used to be a fixed 300 ms sleep and then a check that the pane was
// clear -- the same race `clear_and_trigger_on_one_line_starts_a_fresh_
// conversation` had. With all 16 cores busy the check failed 3 runs in 20 on
// 0.1.1 and 4 in 20 on 0.1.2 (0.1.2 build record, D-06). So it now waits for
// the copies to actually be gone.
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
    p.wait_until_gone("remember me", Duration::from_secs(10));
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
// having proven nothing about turn 2.
//
// This used to be closed with a fixed 300 ms settle, on the reasoning that
// `clear` is near-instant. It is -- but the settle has to cover the whole
// path from `send-keys` to `clear` running, and under load that path is
// the slow part. With all 16 cores of the 0.1.2 build machine busy, the
// fixed settle failed 4 runs in 20 on 0.1.1 and 5 in 20 on 0.1.2, every
// time with the pane still showing turn 1 and not even the echo of the
// typed line (0.1.2 build record, D-04). So it now waits for the event
// itself: turn 1's text leaving the pane, which only `clear` can cause.
// After that, `wait_for("🤖:", ..)` can only be satisfied by turn 2.
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
    p.wait_until_gone("first question", Duration::from_secs(10));
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
/// R23: **REQ-11's acceptance criterion, verbatim** -- `ls /nonexistent`,
/// then `@@ …`, with no prior `@@` anywhere on the pane. The test below
/// looks nearly identical but is not the criterion: it sends a *prior*
/// trigger first, which is what makes "slice from the first trigger down"
/// reach above the question at all.
///
/// The current question is already echoed on the pane by the time `ama`
/// captures it, so when it is the only trigger the old slice was that one
/// line and every command above it -- including the failure being asked
/// about -- was discarded. ADR-002 justifies the entire tmux path as "the
/// difference between answering 'why did that build fail?' and not", so
/// this is the product's headline case, and it did not work.
///
/// The needle is the same one the test below uses, for the same reason:
/// "No such file" appears exactly once in GNU `ls`'s error and never in a
/// typed line, so a *second* occurrence can only be `fake-agent-echo`
/// mirroring back a composed prompt that really carried the failure.
/// (`nonexistent-marker-xyz` would not do: `ls` prints the name back, so
/// it is already at two occurrences with no agent involved.)
#[test]
fn the_first_question_in_a_pane_still_sees_the_output_above_it() {
    require_tmux!();
    let p = Pane::start("ama-e2e-first-context", "bash");
    p.send("ls /nonexistent-marker-xyz");
    p.wait_for("nonexistent-marker-xyz", Duration::from_secs(5));
    p.send("@@ why did that fail");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let text = p.capture();
        if text.matches("No such file").count() >= 2 {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "the first question in a pane must still carry the output above it \
                 (REQ-11, as amended by A-04)\n--- pane ---\n{text}"
            );
        }
        std::thread::sleep(Duration::from_millis(120));
    }
}

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

/// R24 / REQ-15, on the path the defect was reported from: a real pane with
/// a secret visible on it. `--no-context` is one of only two mitigations
/// `04-design.md` names for the owner-accepted RISK-01, and through the
/// trigger it did neither half of its job -- the whole pane went to the
/// agent and the question itself read `--no-context what is my key`.
///
/// This is the e2e half rather than a duplicate of the integration test:
/// the hook rewrites the typed line to `@@ '--no-context what is my key'`,
/// a *single* quoted word, so a fix that peeled an argv token would pass
/// the integration test and still leak here. Only peeling the joined
/// question covers both.
///
/// Waiting on the `## Question` body is what makes this discriminating:
/// before the fix that line read `   --no-context what is my key`, so the
/// needle below never appears and the timeout dumps the leaking pane.
#[test]
fn the_trigger_honours_no_context_in_a_real_shell() {
    require_tmux!();
    let p = Pane::start("ama-e2e-no-context", "bash");
    p.send("echo AWS_SECRET_ACCESS_KEY=zzz999");
    p.wait_for("zzz999", Duration::from_secs(5));
    p.send("@@ --no-context what is my key");
    let needle = format!("\n{}what is my key", ama::context::CONT_INDENT);
    let pane = p.wait_for(&needle, Duration::from_secs(10));
    assert!(
        !pane.contains("## Terminal"),
        "`@@ --no-context` must not send the pane (RISK-01's mitigation)\n{pane}"
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
    // Not `wait_for("rc=")`: that needle is satisfied by the terminal's own
    // echo of `echo rc=$?`, so the assertion below could be evaluated
    // before the output it is about even exists -- in the only test that
    // covers NFR-05's exit code 130.
    let pane = p.wait_for("rc=130", Duration::from_secs(5));
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

/// R26 / REQ-03, driven through a real shell rather than through
/// `__ama_split` alone. Both lines below are ordinary `echo`s that an
/// uninstrumented bash prints verbatim, and REQ-03 -- "lines not beginning
/// with the trigger behave exactly as in an uninstrumented shell" -- makes
/// that non-negotiable for anything bound to Enter. Rule 2 had no notion of
/// quoting, so the operator inside each quoted word looked like a real
/// command separator: the first line was silently rewritten to
/// `echo 'clear && @@ 'joke'\'''` and printed `clear && @@ joke\`, and the
/// second was rewritten to an unbalanced `echo "a; @@ 'not a prompt"'`,
/// dropping the shell to `PS2` where it sat forever.
///
/// The assertions are deliberately line-*exact*: `tmux send-keys` echoes
/// the typed line, so the pane contains `clear && @@ joke` as a substring
/// whether or not the pass-through worked. Only a correct pass-through puts
/// that text on a line of its own, as `echo`'s output.
///
/// The trailing marker is what catches the `PS2` hang, and it is split as
/// `QUOTED-OP-'DONE'` for the same reason: the typed line is echoed even at
/// a continuation prompt, so a marker spelled literally would appear on the
/// pane while the shell sat wedged. Only the shell actually *running* the
/// `echo` joins the two halves.
#[test]
fn a_quoted_operator_before_the_trigger_leaves_an_ordinary_command_alone() {
    require_tmux!();
    let p = Pane::start("ama-e2e-quoted-op", "bash");
    p.send("echo 'clear && @@ joke'");
    p.send("echo \"a; @@ not a prompt\"");
    p.send("echo QUOTED-OP-'DONE'");
    let pane = p.wait_for("QUOTED-OP-DONE", Duration::from_secs(8));
    let has_line = |needle: &str| pane.lines().any(|l| l.trim_end() == needle);
    assert!(
        has_line("clear && @@ joke"),
        "a quoted `&&` before the trigger must not corrupt the command\n{pane}"
    );
    assert!(
        has_line("a; @@ not a prompt"),
        "a quoted `;` before the trigger must not corrupt the command\n{pane}"
    );
    assert!(
        !pane.contains("🤖:"),
        "neither line is a trigger; the agent must not run\n{pane}"
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

// R18 finding 4: "indented question" has no shell metacharacter, so
// "   @@ indented question" is also a perfectly valid command line on its
// own -- bash itself discards leading whitespace before a command name,
// and `@@` is a real executable on `$PATH` in this harness (`Pane::start`
// copies it there), so it runs and answers whether or not `__ama_split`'s
// leading-whitespace handling does anything at all. Verified by mutation:
// forcing `__ama_split` to never strip a leading-whitespace prefix left
// this test green. Using a question bash cannot parse unquoted (mirroring
// `the_readme_first_example_works_verbatim`) closes that: if the trigger
// is not recognised, bash chokes on the bare apostrophe and no answer
// -- indented or otherwise -- ever appears.
#[test]
fn leading_whitespace_still_triggers() {
    require_tmux!();
    let p = Pane::start("ama-e2e-indent", "bash");
    p.send("   @@ what's up?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("what's up?"), "{pane}");
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
///
/// R18 finding 3 extends this to install *twice*: the guard
/// `$existing != *'\C-x\C-aq'*` correctly stops a second install from
/// re-chaining (which would nest `\C-x\C-aq` inside itself), but the
/// original code then fell into the same `else` branch a fresh install
/// uses, unconditionally overwriting `\C-m` with a bare `\C-x\C-aq\C-j` --
/// dropping the third-party payload it had *already* correctly chained.
/// Verified directly across three real installs in a tmux pane: `\C-m`
/// went `[]` -> `\C-x\C-aqecho PRE-EXISTING\C-j` (correct) ->
/// `\C-x\C-aq\C-j` (payload lost) on the second install. `__ama_install`
/// is called a second time here via `\C-j`, not `Enter` -- `\C-m` is
/// already chained by that point, so pressing it would replay that chain
/// on top of the command instead of running it cleanly.
#[test]
fn an_existing_c_m_binding_is_chained_not_clobbered() {
    require_tmux!();
    let probe = "bind '\"\\C-m\": \"echo CHAIN-PROBE-RAN\\C-j\"'\n";
    let p = Pane::start_with_extra_rc("ama-e2e-chain", "bash", probe);
    p.send_raw("__ama_install");
    std::thread::sleep(Duration::from_millis(200));
    p.send_raw("C-j");
    std::thread::sleep(Duration::from_millis(200));
    p.send_raw("Enter");
    let pane = p.wait_for("CHAIN-PROBE-RAN", Duration::from_secs(5));
    assert!(
        pane.contains("CHAIN-PROBE-RAN"),
        "a pre-existing \\C-m binding must be chained, not clobbered, \
         even after a second install\n{pane}"
    );
}

// ---- REQ-33: Return is not always `\C-m` (0.1.1) ---------------------------
//
// 0.1.0 bound only `\C-m`, because `\C-j` was the terminator its Enter macro
// ended with. A terminal that sends LF for Return -- iTerm2 can be set up
// that way, macOS Terminal is not -- therefore bypassed the hook entirely:
// the raw line reached bash, the apostrophe in `what's` opened a quote that
// never closed, and the shell sat at PS2 with no output and no error. That
// is defect 2 of this cycle, and the reporter's `~/.bash_history` shows it
// directly: every `@@` line the hook processed is stored rewritten, and the
// failing one is stored raw.
//
// Every other test in this file submits with `Enter`, which tmux sends as
// CR. That is why the whole 0.1.0 suite was green on a defect that made the
// product unusable on one of two common terminals, and it is why these
// tests send the key explicitly.

#[test]
fn a_question_submitted_with_line_feed_reaches_the_agent() {
    require_tmux!();
    let p = Pane::start("ama-e2e-lf", "bash");
    // The reporter's line, verbatim -- apostrophe and all, which is what
    // turned a skipped hook into a stuck shell rather than a wrong answer.
    p.send_raw("@@ what's the files listed under this dir");
    std::thread::sleep(Duration::from_millis(300));
    p.send_raw("C-j");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("@@ 'what'\\''s the files listed under this dir'"),
        "LF must reach the hook and be rewritten, not land at PS2\n{pane}"
    );
}

/// REQ-03 on the new key: an untriggered line must behave exactly as it
/// would in an uninstrumented shell. Binding a second key to Enter is the
/// most invasive thing this cycle does, so it is checked on both.
#[test]
fn an_ordinary_command_submitted_with_line_feed_is_unaffected() {
    require_tmux!();
    let p = Pane::start("ama-e2e-lf-plain", "bash");
    p.send_raw("echo LF-NORMAL-OK");
    std::thread::sleep(Duration::from_millis(300));
    p.send_raw("C-j");
    let pane = p.wait_for("LF-NORMAL-OK", Duration::from_secs(5));
    assert!(!pane.contains("🤖:"), "no agent should have run\n{pane}");
}

/// RISK-03 on the new key: `\C-j` gets the same chaining treatment `\C-m`
/// has, so a tool that already owns it keeps working.
#[test]
fn an_existing_c_j_binding_is_chained_not_clobbered() {
    require_tmux!();
    let probe = "bind '\"\\C-j\": \"echo CJ-PROBE-RAN\\C-m\"'\n";
    let p = Pane::start_with_extra_rc("ama-e2e-chain-cj", "bash", probe);
    p.send_raw("C-j");
    let pane = p.wait_for("CJ-PROBE-RAN", Duration::from_secs(5));
    assert!(
        pane.contains("CJ-PROBE-RAN"),
        "a pre-existing \\C-j binding must be chained, not clobbered\n{pane}"
    );
}

/// RISK-09 / F-11 / NFR-09 -- the hazard the REQ-33 fix introduces, pinned
/// before it can escape.
///
/// Once `\C-j` expands to a macro of ours, a third-party `\C-m` macro that
/// *ends* in `\C-j` -- the ordinary way to write one, and what this file's
/// own `an_existing_c_m_binding_is_chained_not_clobbered` plants -- makes
/// one keypress run `__ama_hook` twice. The second pass would re-quote the
/// buffer the first pass already quoted:
///
/// ```text
/// @@ what's it  ->  @@ 'what'\''s it'  ->  @@ ''\''what'\''\'\'''\''s it'\'''
/// ```
///
/// which reaches the agent as a question full of quote marks. The guard in
/// `__ama_hook` compares the buffer against the last one it wrote and
/// returns if they match. Verified by mutation: with the guard deleted, this
/// test fails and the corrupt form below is what appears on the pane.
#[test]
fn a_third_party_enter_macro_does_not_double_quote_a_triggered_line() {
    require_tmux!();
    // A no-op wrapper around Enter: the whole macro body is the key that
    // accepts the line. Harmless on its own, and the minimal shape that
    // routes `\C-m` through `\C-j`.
    let probe = "bind '\"\\C-m\": \"\\C-j\"'\n";
    let p = Pane::start_with_extra_rc("ama-e2e-double", "bash", probe);
    p.send_raw("@@ what's it");
    std::thread::sleep(Duration::from_millis(300));
    p.send_raw("Enter");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("@@ 'what'\\''s it'"),
        "the line must be quoted exactly once\n{pane}"
    );
    assert!(
        !pane.contains("''\\''what"),
        "the hook ran twice and re-quoted its own output\n{pane}"
    );
}

/// Ctrl-L must clear the screen *and* the scrollback, and must not throw
/// away whatever the user was part-way through typing.
///
/// R28: the buffer assertion was the whole of this test, and it cannot
/// fail for the property the test is named after -- a widget that did
/// nothing at all would leave the in-progress line exactly where it was
/// and pass. The scrollback marker below is what pins the clearing, and it
/// is the same technique the zsh counterpart already used: fill well past
/// the pane height, then require `capture-pane -S` (which sees history
/// `-p` alone cannot) to come back with nothing. Verified by mutation:
/// with `__ama_clear_screen_widget`'s `printf` deleted, the buffer check
/// still passes and this one fails.
#[test]
fn ctrl_l_clears_the_screen_and_scrollback_without_losing_the_typed_line() {
    require_tmux!();
    let p = Pane::start("ama-e2e-ctrl-l", "bash");
    // Fill well past the pane's 60-row height, so some of this is only
    // reachable through scrollback once Ctrl-L has run.
    p.send("for i in $(seq 1 80); do echo SCROLLBACK-MARKER-$i; done");
    p.wait_for("SCROLLBACK-MARKER-80", Duration::from_secs(5));

    p.send_raw("echo HALF-TYPED-COMMAND");
    std::thread::sleep(Duration::from_millis(400));
    p.send_raw("C-l");
    let (pane, history) = p.wait_for_clear(
        "SCROLLBACK-MARKER",
        "echo HALF-TYPED-COMMAND",
        Duration::from_secs(5),
    );

    assert!(
        pane.contains("echo HALF-TYPED-COMMAND"),
        "Ctrl-L must preserve the in-progress line, not just clear the screen\n{pane}"
    );

    assert!(
        !history.contains("SCROLLBACK-MARKER"),
        "Ctrl-L must actually clear the screen and drop scrollback, \
         not merely leave the typed line alone\n{history}"
    );
}

// ---- Task 8: the same integration, ported to zsh ---------------------------

#[test]
fn the_readme_first_example_works_in_zsh_too() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let p = Pane::start("ama-e2e-zsh", "zsh");
    p.send("@@ what's this project all about?");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(pane.contains("what's this project all about?"), "{pane}");
}

/// F-10: zsh never had defect 2, for a structural reason worth a test.
///
/// `ama.zsh` replaces the `accept-line` *widget*, and zsh binds both `^M`
/// and `^J` to that widget by name -- so one replacement covers both keys.
/// bash binds key *sequences* to functions and macros, so each sequence has
/// to be handled on its own, which is why REQ-33's fix is bash-only.
///
/// This test passes before and after that fix. It exists so that a later
/// reader who sees `\C-m`/`\C-j` handled explicitly in `ama.bash` and
/// "ports the fix" to `ama.zsh` -- rebinding keys instead of the widget --
/// gets a failure instead of a silent regression.
#[test]
fn a_question_submitted_with_line_feed_works_in_zsh_too() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let p = Pane::start("ama-e2e-zsh-lf", "zsh");
    p.send_raw("@@ what's the files listed under this dir");
    std::thread::sleep(Duration::from_millis(300));
    p.send_raw("C-j");
    let pane = p.wait_for("🤖:", Duration::from_secs(10));
    assert!(
        pane.contains("what's the files listed under this dir"),
        "zsh must handle LF through the accept-line widget\n{pane}"
    );
}

/// zsh's own `clear-screen` widget (bound to Ctrl-L by default) already
/// preserves and redraws an in-progress line by itself -- confirmed
/// directly against real zsh with no override installed at all, unlike
/// bash's `bind -x` callbacks (ama.bash's R18 finding 2). So the buffer
/// check below is really a sanity check that ama.zsh's override did not
/// *break* that pre-existing behaviour. The scrollback check is the part
/// that needed a real fix: zsh's `.clear-screen` only clears the visible
/// screen, so without ama.zsh's added `\033[3J`, tmux's saved history
/// still contains the earlier lines after Ctrl-L -- confirmed directly by
/// running this same scenario against a widget that calls only
/// `zle .clear-screen` with no `3J`, where the assertion below fails.
#[test]
fn ctrl_l_clears_the_screen_and_scrollback_without_losing_the_typed_line_in_zsh() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let p = Pane::start("ama-e2e-zsh-ctrl-l", "zsh");
    // Fill well past the pane's 60-row height so some of this is only
    // reachable via scrollback, not the visible screen, once Ctrl-L runs.
    p.send("for i in $(seq 1 80); do echo SCROLLBACK-MARKER-$i; done");
    p.wait_for("SCROLLBACK-MARKER-80", Duration::from_secs(5));

    p.send_raw("echo HALF-TYPED-COMMAND");
    std::thread::sleep(Duration::from_millis(400));
    p.send_raw("C-l");
    let (pane, history) = p.wait_for_clear(
        "SCROLLBACK-MARKER",
        "echo HALF-TYPED-COMMAND",
        Duration::from_secs(5),
    );

    assert!(
        pane.contains("echo HALF-TYPED-COMMAND"),
        "Ctrl-L must preserve the in-progress line, not just clear the screen\n{pane}"
    );

    assert!(
        !history.contains("SCROLLBACK-MARKER"),
        "Ctrl-L must also drop scrollback (ama.bash does this too), \
         not just clear the visible screen\n{history}"
    );
}

/// The zsh equivalent of ama.bash's `an_existing_c_m_binding_is_chained_
/// not_clobbered` (RISK-03): a pre-existing `accept-line` customization --
/// as e.g. a completion or autosuggestion framework would install -- must
/// survive `ama init zsh`, and survive repeated `eval`s of it in the same
/// shell (the zsh equivalent of R18 finding 3). zsh has no `bind -s`-style
/// text to scrape; `ama.zsh` instead saves the previous widget under
/// `__ama_orig_accept_line` via `zle -A` and checks `$widgets` before
/// re-saving.
///
/// The failure mode behind an unconditional (non-idempotent) `zle -A` is
/// *not* "silently overwritten" the way bash's bare `\C-m` rebind is, and
/// it is not an immediate hang either -- both guesses tried and rejected
/// while developing this integration, in favour of driving real zsh: one
/// extra `eval` merely adds one harmless level of indirection (confirmed
/// by instrumenting `__ama_accept_line` directly: exactly one nested call,
/// not a runaway chain). It takes a handful of repeated installs -- e.g. a
/// user re-sourcing their rc file a few times over a session -- before the
/// self-referential alias actually recurses deeply enough to hit zsh's own
/// recursion guard, surfacing as `__ama_split:...: maximum nested function
/// level reached` on stderr and the probe never running again (reproduced
/// directly: 1 extra install is silent, 2 already prints the error). Four
/// installs is comfortably past that threshold without depending on the
/// exact number, so that is what this test does, asserting the error
/// string never appears and the probe still fires after all of them --
/// which only holds if every install after the first was a genuine no-op.
#[test]
fn an_existing_accept_line_widget_is_chained_not_clobbered_in_zsh() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let probe = "__ama_e2e_probe_accept_line() { print -r -- CHAIN-PROBE-RAN; zle .accept-line; }\n\
                 zle -N accept-line __ama_e2e_probe_accept_line\n";
    let p = Pane::start_with_extra_rc("ama-e2e-zsh-chain", "zsh", probe);

    p.send("true");
    let pane = p.wait_for("CHAIN-PROBE-RAN", Duration::from_secs(5));
    assert!(
        pane.contains("CHAIN-PROBE-RAN"),
        "a pre-existing accept-line widget must be chained, not clobbered\n{pane}"
    );

    // Re-`eval "$(ama init zsh)"` several times in the same shell, exactly
    // as repeatedly re-sourcing an rc file would.
    for _ in 0..4 {
        p.send(r#"eval "$(ama init zsh)""#);
        std::thread::sleep(Duration::from_millis(300));
    }
    let pane = p.capture();
    assert!(
        !pane.contains("maximum nested function level"),
        "each re-install after the first must be a no-op, not one more \
         layer of self-referential chaining\n{pane}"
    );

    // Reset the screen so the next appearance can only be explained by the
    // `true` below, not by earlier probe output still sitting on screen.
    p.send("clear");
    // Wait for the pane to be clear rather than pausing a fixed 300 ms: the
    // same shape that made two other tests flaky under load (0.1.2 D-04,
    // D-06). This one held 20 runs in 20 under load -- the change is
    // preventive.
    p.wait_until_gone("CHAIN-PROBE-RAN", Duration::from_secs(10));

    p.send("true");
    let pane = p.wait_for("CHAIN-PROBE-RAN", Duration::from_secs(5));
    assert!(
        pane.contains("CHAIN-PROBE-RAN"),
        "the pre-existing widget must still be chained after repeated installs\n{pane}"
    );
}

// ---- 0.1.2: the moon while the agent thinks --------------------------------
//
// Everything here runs against `fake-agent-thinking` or `fake-agent-chatty`,
// which stay quiet (or talk only on stderr) long enough for the moon to be
// sampled. Samples are taken every 30 ms rather than `wait_for`'s 120 ms:
// the moon moves every 100 ms, and these tests are about seeing it move.

/// The eight phases, written out here rather than taken from `spinner.rs`,
/// so that a wrong glyph there cannot also be the expectation here.
const MOONS: [char; 8] = ['🌑', '🌒', '🌓', '🌔', '🌕', '🌖', '🌗', '🌘'];

/// The phase on a line that is exactly the moon: `🤖`, a space, one phase.
fn moon_line(line: &str) -> Option<char> {
    let mut rest = line.trim_end().strip_prefix("🤖 ")?.chars();
    match (rest.next(), rest.next()) {
        (Some(c), None) if MOONS.contains(&c) => Some(c),
        _ => None,
    }
}

fn moons_on(text: &str) -> Vec<char> {
    text.chars().filter(|c| MOONS.contains(c)).collect()
}

impl Pane {
    /// Like `capture`, with lines the terminal soft-wrapped joined back up
    /// (`-J`). A diagnostic naming a fixture by its absolute path is longer
    /// than the pane is wide, and would otherwise be split mid-sentence.
    fn capture_joined(&self) -> String {
        let out = Command::new("tmux")
            .args(["capture-pane", "-t", &self.name, "-p", "-J"])
            .output()
            .expect("capture-pane");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// Poll until a transcript under this pane's `$HOME` mentions `needle`.
    /// `ama` writes it after the answer's last line reaches the screen, so
    /// seeing that line is not yet proof the transcript exists.
    fn wait_for_transcript(&self, needle: &str, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let all = self.transcripts();
            if all.contains(needle) {
                return all;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for a transcript mentioning {needle:?}: {all:?}"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
    }

    /// Poll quickly until a moon line is on screen.
    fn wait_for_moon(&self, timeout: Duration) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let pane = self.capture();
            if pane.lines().any(|l| moon_line(l).is_some()) {
                return pane;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for the moon\n--- pane ---\n{pane}"
            );
            std::thread::sleep(Duration::from_millis(30));
        }
    }
}

/// REQ-35, REQ-36, RISK-17 -- with the owner's own words for a prompt.
///
/// `fake-agent-thinking` is silent for 2 s and then echoes its whole prompt
/// back, so this one test sees everything the feature promises: the moon
/// arrives at once, it moves, and it leaves nothing behind. Not on the
/// screen, not in the transcript, and not in the prompt -- where a captured
/// frame would come straight back in the echoed answer.
#[test]
fn the_moon_waxes_while_the_agent_thinks() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon", "bash", "fake-agent-thinking");
    p.send("@@ whats weather?");
    let sent = Instant::now();

    let mut first_moon = None;
    let mut phases: Vec<char> = Vec::new();
    loop {
        let pane = p.capture();
        if pane.contains("🤖:") {
            break;
        }
        if let Some(phase) = pane.lines().find_map(moon_line) {
            first_moon.get_or_insert(sent.elapsed());
            if phases.last() != Some(&phase) {
                phases.push(phase);
            }
        }
        assert!(
            sent.elapsed() < Duration::from_secs(10),
            "no answer\n--- pane ---\n{pane}"
        );
        std::thread::sleep(Duration::from_millis(30));
    }

    let first = first_moon.expect("the moon never showed before the answer");
    assert!(first < Duration::from_secs(1), "the moon took {first:?}");
    let distinct: std::collections::HashSet<&char> = phases.iter().collect();
    assert!(
        distinct.len() >= 3,
        "a static glyph, not an animation: {phases:?}"
    );

    // The echoed question is the answer's last line. The moon was erased
    // before the answer's first byte, so there is nothing left to wait for
    // on screen; the transcript is written a moment later, so poll for it.
    let pane = p.wait_for("   whats weather?", Duration::from_secs(5));
    assert!(
        moons_on(&pane).is_empty(),
        "the moon left a trace on screen or in the prompt\n{pane}"
    );
    let transcripts = p.wait_for_transcript("whats weather?", Duration::from_secs(5));
    assert!(
        moons_on(&transcripts).is_empty(),
        "the moon reached the transcript: {transcripts}"
    );
    // serde_json writes a raw ESC as the six characters `\u001b`.
    assert!(
        !transcripts.contains("\\u001b"),
        "an escape sequence reached the transcript: {transcripts}"
    );
}

/// REQ-38 -- the codex case (F-14), where each of thirty progress lines on
/// stderr would otherwise get a moon glued to the front of it.
#[test]
fn agent_stderr_keeps_its_own_lines_while_the_moon_is_up() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon-stderr", "bash", "fake-agent-chatty");
    p.send("@@ what are you doing");

    // During the first pause, the moon belongs on the line below the
    // finished stderr line -- still showing, not given up on.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let pane = p.capture();
        let lines: Vec<&str> = pane.lines().map(str::trim_end).collect();
        if lines
            .windows(2)
            .any(|w| w[0] == "progress one" && moon_line(w[1]).is_some())
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "no moon below the stderr line\n--- pane ---\n{pane}"
        );
        std::thread::sleep(Duration::from_millis(30));
    }

    let pane = p.wait_for("🤖: the answer", Duration::from_secs(10));
    let lines: Vec<&str> = pane.lines().map(str::trim_end).collect();
    assert!(lines.contains(&"progress one"), "{pane}");
    // Written in two pieces with a pause between: a moon drawn over the
    // unfinished half would have erased `partial`.
    assert!(lines.contains(&"partial line"), "{pane}");
    assert!(moons_on(&pane).is_empty(), "{pane}");
}

/// REQ-36: with no answer to take the moon's place, `ama`'s own diagnostic
/// must still begin on a clean line, not after a frame left behind.
#[test]
fn a_silent_agent_is_reported_on_a_clean_line() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon-silent", "bash", "fake-agent-silent");
    p.send("@@ say nothing");
    let needle = "exited without producing any output";
    let deadline = Instant::now() + Duration::from_secs(10);
    let pane = loop {
        let pane = p.capture_joined();
        if pane.contains(needle) {
            break pane;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {needle:?}\n--- pane ---\n{pane}"
        );
        std::thread::sleep(Duration::from_millis(30));
    };
    let line = pane
        .lines()
        .find(|l| l.contains(needle))
        .expect("the diagnostic");
    assert!(line.starts_with("ama: "), "{line:?}\n--- pane ---\n{pane}");
    assert!(moons_on(&pane).is_empty(), "{pane}");
}

/// REQ-39: `spinner: false` shows no moon at any point while the agent
/// thinks, and the answer still arrives.
#[test]
fn the_moon_can_be_turned_off() {
    require_tmux!();
    let p = Pane::start_with_config(
        "ama-e2e-moon-off",
        "bash",
        "fake-agent-thinking",
        "spinner: false\n",
    );
    p.send("@@ whats weather?");
    let sent = Instant::now();
    loop {
        let pane = p.capture();
        if pane.contains("🤖:") {
            break;
        }
        assert!(
            moons_on(&pane).is_empty(),
            "the moon showed with `spinner: false`\n{pane}"
        );
        assert!(
            sent.elapsed() < Duration::from_secs(10),
            "no answer\n--- pane ---\n{pane}"
        );
        std::thread::sleep(Duration::from_millis(30));
    }
}

/// RISK-15: Ctrl-C while the moon is up must behave as it always has --
/// `$?` is 130, and the shell keeps taking commands. The moon adds a thread
/// and no signal handling, and neither may change that.
#[test]
fn an_interrupt_during_the_moon_returns_a_usable_shell() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon-interrupt", "bash", "fake-agent-thinking");
    p.send("@@ whats weather?");
    p.wait_for_moon(Duration::from_secs(5));
    p.send_raw("C-c");
    std::thread::sleep(Duration::from_millis(400));
    p.send("echo rc=$?");
    p.wait_for("rc=130", Duration::from_secs(5));
    p.send("echo SHELL-ALIVE");
    p.wait_for("SHELL-ALIVE", Duration::from_secs(5));
}

/// ADR-010: the moon is drawn by the binary, not the hook, so the other
/// shell gets it without a line of `ama.zsh` changing.
#[test]
fn the_moon_waxes_in_zsh_too() {
    require_tmux!();
    if !have("zsh") {
        eprintln!("skipping: zsh not installed");
        return;
    }
    let p = Pane::start_with_agent("ama-e2e-moon-zsh", "zsh", "fake-agent-thinking");
    p.send("@@ whats weather?");
    p.wait_for_moon(Duration::from_secs(5));
    let pane = p.wait_for("   whats weather?", Duration::from_secs(10));
    assert!(moons_on(&pane).is_empty(), "{pane}");
}

/// RISK-18 (review finding I-1). Enter pressed while the moon is up is
/// echoed by the terminal, which moves the cursor down and strands the last
/// frame on the line above. On screen that is cosmetic. Captured as the next
/// turn's context it is not: it would reach the agent as if typed. The
/// fixture echoes each prompt back whole, and the transcript records it, so
/// a stranded frame in turn 2's context would show up here.
#[test]
fn an_enter_pressed_mid_think_never_puts_the_moon_in_the_next_prompt() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon-enter", "bash", "fake-agent-thinking");
    p.send("@@ whats weather?");
    p.wait_for_moon(Duration::from_secs(5));
    p.send_raw("Enter");
    std::thread::sleep(Duration::from_millis(250));
    p.send_raw("Enter");
    p.wait_for("   whats weather?", Duration::from_secs(10));

    p.send("@@ second turn");
    p.wait_for("   second turn", Duration::from_secs(10));
    let transcripts = p.wait_for_transcript("second turn", Duration::from_secs(5));
    assert!(
        moons_on(&transcripts).is_empty(),
        "a stranded frame was sent as context: {transcripts}"
    );
}

/// ADR-012, and review finding M3. The agent's stderr is a pipe only while
/// the moon is showing on a terminal. Whenever `ama`'s own stderr is not
/// that terminal, the agent gets `ama`'s stderr itself, as in 0.1.1.
#[test]
fn the_agents_stderr_is_routed_only_while_the_moon_shares_its_terminal() {
    require_tmux!();
    let p = Pane::start_with_agent("ama-e2e-moon-stderr-kind", "bash", "fake-agent-stderr-kind");
    p.send("@@ what is your stderr");
    p.wait_for("🤖: stderr is a pipe", Duration::from_secs(10));
    // stdout is still the terminal here, so the moon still shows -- but
    // stderr goes elsewhere, so there is nothing to keep off its line.
    p.send("ama ask -- what is your stderr 2>/dev/null");
    p.wait_for("🤖: stderr is something else", Duration::from_secs(10));
}

/// REQ-39 / RISK-13. `spinner: false` is the escape hatch for an agent that
/// misbehaves on a piped stderr, so it must hand the agent the terminal back.
#[test]
fn spinner_false_hands_the_agent_its_terminal_back() {
    require_tmux!();
    let p = Pane::start_with_config(
        "ama-e2e-moon-stderr-off",
        "bash",
        "fake-agent-stderr-kind",
        "spinner: false\n",
    );
    p.send("@@ what is your stderr");
    p.wait_for("🤖: stderr is a terminal", Duration::from_secs(10));
}
