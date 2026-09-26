use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use std::io::Write;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A config pointing at a fake agent, plus an isolated HOME so the real
/// ~/.qmx2 is never touched by tests.
fn env() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config.yml");
    let mut f = std::fs::File::create(&cfg).expect("create");
    writeln!(f, "agent:\n  command: [{}]", fixture("fake-agent-echo")).expect("write");
    (dir, cfg)
}

fn ama(cfg: &std::path::Path, home: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("ama").expect("binary");
    // session::current_key() falls back to the calling process's tty
    // (`/proc/self/fd/2`) when neither $TMUX_PANE nor $AMA_SESSION is set.
    // assert_cmd always pipes a spawned child's stdio (see its `cmd.rs`),
    // so that tty fallback resolves to a *fresh pipe* per invocation --
    // stable only when the test happens to run inside a real tmux pane
    // (where the ambient $TMUX_PANE would carry through instead). Pinning
    // $AMA_SESSION here is what makes "two ama invocations, one
    // conversation" tests deterministic on every runner, tmux or not.
    c.env("AMA_CONFIG", cfg)
        .env("HOME", home)
        .env("AMA_SESSION", "integration-cli-test")
        .env_remove("TMUX")
        .env_remove("STY");
    c
}

#[test]
fn ask_answers_with_the_robot_prefix() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "what is this"])
        .assert()
        .success()
        .stdout(predicates::str::starts_with("🤖: "));
}

#[test]
fn the_question_reaches_the_agent() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "what's this project all about?"])
        .assert()
        .success()
        .stdout(predicates::str::contains("what's this project all about?"));
}

#[test]
fn no_context_sends_the_question_alone() {
    let (d, cfg) = env();
    // A fresh session has nothing to gather anyway, which would make this
    // vacuously true regardless of whether `--no-context` does anything --
    // record a real prior turn first so there is context to suppress.
    ama(&cfg, d.path())
        .args(["ask", "--", "first question"])
        .assert()
        .success();
    ama(&cfg, d.path())
        .args(["ask", "--no-context", "--", "bare"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains("## Terminal")
                .not()
                .and(predicates::str::contains("first question").not()),
        );
}

#[test]
fn a_second_turn_carries_the_first_in_its_context() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "first question"])
        .assert()
        .success();
    ama(&cfg, d.path())
        .args(["ask", "--", "second question"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first question"));
}

#[test]
fn session_reset_drops_the_conversation() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "first question"])
        .assert()
        .success();
    ama(&cfg, d.path())
        .args(["session", "reset"])
        .assert()
        .success();
    ama(&cfg, d.path())
        .args(["ask", "--", "second question"])
        .assert()
        .success()
        .stdout(predicates::str::contains("first question").not());
}

#[test]
fn the_transcript_is_readable_only_by_its_owner() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let (d, cfg) = env();
        ama(&cfg, d.path())
            .args(["ask", "--", "q"])
            .assert()
            .success();
        let sessions = d.path().join(".qmx2/sessions");
        let entry = std::fs::read_dir(&sessions)
            .expect("sessions dir")
            .next()
            .expect("one transcript")
            .expect("entry");
        let mode = entry.metadata().expect("meta").permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "transcript must be 0600, got {:o}",
            mode & 0o777
        );
    }
}

#[test]
fn a_missing_config_exits_two_and_says_where_to_put_it() {
    let d = tempfile::tempdir().expect("tempdir");
    let missing = d.path().join("nope.yml");
    Command::cargo_bin("ama")
        .expect("binary")
        .env("AMA_CONFIG", &missing)
        .env("HOME", d.path())
        .args(["ask", "--", "x"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("nope.yml"));
}

#[test]
fn a_failing_agent_exits_one() {
    let d = tempfile::tempdir().expect("tempdir");
    let cfg = d.path().join("config.yml");
    std::fs::write(
        &cfg,
        format!("agent:\n  command: [{}]\n", fixture("fake-agent-fail")),
    )
    .expect("write");
    // R1: the exit-code contract is applied in cli::ask, which maps any
    // non-zero agent exit to 1 -- it does not forward the agent's raw code
    // (fake-agent-fail exits 7; see agent::run's own test for that raw value).
    ama(&cfg, d.path())
        .args(["ask", "--", "x"])
        .assert()
        .code(1);
}

#[test]
fn an_empty_question_is_a_silent_no_op() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "--", "   "])
        .assert()
        .success()
        .stdout("");
}

#[test]
fn the_at_at_name_is_an_alias_for_ask() {
    let (d, cfg) = env();
    let bin = assert_cmd::cargo::cargo_bin("ama");
    let alias = d.path().join("@@");
    std::fs::copy(&bin, &alias).expect("copy");

    // Immediately exec'ing a just-copied binary can transiently race the
    // kernel's teardown of the write mapping used to create it: a rare,
    // environment-dependent ETXTBSY (os error 26) observed under a full
    // `cargo test` run, not an `ama` bug. Retry briefly rather than let
    // that one-in-a-while OS race flake this test.
    let mut output = None;
    for _ in 0..20 {
        let mut cmd = Command::new(&alias);
        cmd.env("AMA_CONFIG", &cfg)
            .env("HOME", d.path())
            .env_remove("TMUX")
            .arg("hello from the alias");
        match cmd.output() {
            Ok(o) => {
                output = Some(o);
                break;
            }
            Err(e) if e.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("failed to spawn the `@@` alias: {e}"),
        }
    }
    let output = output.expect("`@@` alias never became executable (persistent ETXTBSY)");
    assert!(
        output.status.success(),
        "status: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello from the alias"), "stdout: {stdout}");
}

/// R4: `trailing_var_arg` alone still leaves clap treating a leading hyphen
/// as an unknown flag; a question is arbitrary text and must be usable even
/// when it happens to start with `-`. Deliberately omits `--`: with a literal
/// `--` clap already treats everything after it as positional regardless of
/// `allow_hyphen_values`, which would make this test pass either way. The
/// real failure clap reports without `allow_hyphen_values` is on the
/// no-`--` form: `error: unexpected argument '-v' found`.
#[test]
fn a_question_starting_with_a_hyphen_is_taken_literally() {
    let (d, cfg) = env();
    ama(&cfg, d.path())
        .args(["ask", "-v", "is", "not", "a", "flag"])
        .assert()
        .success()
        .stdout(predicates::str::contains("-v is not a flag"));
}

/// R2: `AgentError::Spawn`/`NoCommand` are configuration errors (exit 2, the
/// user's `agent:` line names something unrunnable); only `AgentError::Io`
/// is an agent failure (exit 1).
#[test]
fn an_unrunnable_agent_command_exits_two() {
    let d = tempfile::tempdir().expect("tempdir");
    let cfg = d.path().join("config.yml");
    std::fs::write(
        &cfg,
        "agent:\n  command: [definitely-not-an-installed-program]\n",
    )
    .expect("write");
    ama(&cfg, d.path())
        .args(["ask", "--", "x"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains(
            "definitely-not-an-installed-program",
        ));
}

#[test]
fn init_bash_emits_a_syntactically_valid_script() {
    let out = Command::cargo_bin("ama")
        .expect("bin")
        .args(["init", "bash"])
        .output()
        .expect("run");
    assert!(out.status.success());
    let script = String::from_utf8_lossy(&out.stdout);
    assert!(script.contains("__ama_hook"), "hook missing");
    let mut check = Command::new("bash");
    check
        .args(["-n", "/dev/stdin"])
        .write_stdin(script.as_bytes().to_vec());
    check.assert().success();
}

#[test]
fn init_rejects_an_unknown_shell() {
    Command::cargo_bin("ama")
        .expect("bin")
        .args(["init", "fish"])
        .assert()
        .code(2)
        .stderr(predicates::str::contains("bash"));
}

// The brief's original version of this test appended a bare `exit 0` and
// asserted only `.success()` -- non-discriminating, and its replacement
// (below, from the prior fix round) has since been overtaken by R18
// finding 1: REQ-24's guard used to `return 0 2>/dev/null || exit 0`,
// which -- under a plain `bash -c` with no enclosing function or
// sourced-file context -- always fell through to `exit`, killing the
// whole process before an appended diagnostic line could ever run. That
// made "the diagnostic line is unreachable" the thing to assert. R18
// finding 1 found that same `exit` fires under the *documented* install
// path too (`eval "$(ama init bash)"` inside an interactive shell is fine,
// but a non-interactive one sourcing a `.bashrc` containing that line is
// not), and replaced the guard with a plain `if` that only skips its own
// body. So the property flips: a line appended *after* the script must
// now run (nothing aborts the caller), while `AMA_SESSION` -- exported
// only inside the `if`'s body -- must still stay unset (the
// interactive-only body really was skipped, not just that the process
// happened to keep going regardless).
#[test]
fn sourcing_the_script_in_a_non_interactive_shell_is_a_silent_success() {
    let out = Command::cargo_bin("ama")
        .expect("bin")
        .args(["init", "bash"])
        .output()
        .expect("run");
    let script = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut c = Command::new("bash");
    c.env_remove("AMA_SESSION");
    c.arg("-c").arg(format!(
        "{script}\nprintf 'AMA_SESSION=[%s]' \"$AMA_SESSION\""
    ));
    c.assert().success().stdout("AMA_SESSION=[]").stderr("");
}

// R18 finding 1's exact failure mode, reproduced directly: line 1 of
// `ama.bash` documents `eval "$(ama init bash)"` as the install line, and
// the realistic place for that line is a `.bashrc`. `eval` does not open a
// new sourcing frame -- it runs in the *current* one -- so if a
// non-interactive shell ever sources a `.bashrc` containing that line (an
// ssh remote command, `$BASH_ENV`, some distros' non-interactive startup),
// a `return` reached inside the eval'd guard returns from the `.bashrc`
// sourcing operation itself, silently skipping every line after it for the
// rest of that file. Verified directly against the pre-fix script: this
// exact construct printed only "AFTER_SOURCE_CALL", never
// "AFTER_EVAL_IN_RCFILE".
#[test]
fn eval_ing_the_script_in_a_sourced_rcfile_does_not_skip_the_rest_of_that_file() {
    let out = Command::cargo_bin("ama")
        .expect("bin")
        .args(["init", "bash"])
        .output()
        .expect("run");
    let dir = tempfile::tempdir().expect("tempdir");
    let script_path = dir.path().join("ama_init_bash.sh");
    std::fs::write(&script_path, &out.stdout).expect("write script");
    let rcfile = dir.path().join("fake_bashrc");
    std::fs::write(
        &rcfile,
        format!(
            "eval \"$(cat {})\"\necho AFTER_EVAL_IN_RCFILE\n",
            script_path.display()
        ),
    )
    .expect("write rcfile");
    Command::new("bash")
        .arg("-c")
        .arg(format!(
            "source {}; echo AFTER_SOURCE_CALL",
            rcfile.display()
        ))
        .assert()
        .success()
        .stdout(
            predicates::str::contains("AFTER_EVAL_IN_RCFILE")
                .and(predicates::str::contains("AFTER_SOURCE_CALL")),
        );
}
