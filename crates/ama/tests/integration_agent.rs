use ama::agent;
use ama::config::{AgentSpec, PromptVia};
use ama::spinner::Spinner;

fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn spec(name: &str) -> AgentSpec {
    AgentSpec {
        argv: vec![fixture(name)],
        prompt_via: PromptVia::Stdin,
    }
}

#[test]
fn the_prompt_reaches_the_agent_on_stdin() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-echo"), "hello prompt", &mut out, None).expect("run");
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8_lossy(&out), "🤖: hello prompt\n");
}

#[test]
fn a_prompt_placeholder_is_substituted_into_argv() {
    let s = AgentSpec {
        argv: vec!["/bin/echo".into(), "{prompt}".into()],
        prompt_via: PromptVia::Arg(1),
    };
    let mut out: Vec<u8> = Vec::new();
    agent::run(&s, "substituted", &mut out, None).expect("run");
    assert_eq!(String::from_utf8_lossy(&out), "🤖: substituted\n");
}

#[test]
fn a_failing_agent_reports_its_exit_code() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-fail"), "x", &mut out, None).expect("run");
    assert_eq!(code, 7);
}

#[test]
fn a_missing_agent_is_a_typed_error_naming_the_program() {
    let s = AgentSpec {
        argv: vec!["definitely-not-installed".into()],
        prompt_via: PromptVia::Stdin,
    };
    let err = agent::run(&s, "x", &mut Vec::new(), None).unwrap_err();
    assert!(
        err.to_string().contains("definitely-not-installed"),
        "{err}"
    );
}

#[test]
fn an_empty_argv_is_a_typed_error_not_a_panic() {
    let s = AgentSpec {
        argv: vec![],
        prompt_via: PromptVia::Stdin,
    };
    assert!(agent::run(&s, "x", &mut Vec::new(), None).is_err());
}

#[test]
fn the_agent_inherits_the_working_directory() {
    let mut out: Vec<u8> = Vec::new();
    agent::run(&spec("fake-agent-cwd"), "x", &mut out, None).expect("run");
    let cwd = std::env::current_dir().expect("cwd");
    assert!(String::from_utf8_lossy(&out).contains(&cwd.to_string_lossy().to_string()));
}

#[test]
fn a_silent_agent_is_reported_rather_than_looking_like_success() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-silent"), "x", &mut out, None).expect("run");
    assert_eq!(code, 1, "silence is not success");
    assert!(
        out.is_empty(),
        "no bare robot prefix: {:?}",
        String::from_utf8_lossy(&out)
    );
}

#[test]
fn output_is_streamed_not_buffered_until_exit() {
    use std::time::Instant;
    // fake-agent-slow prints, sleeps 3s, prints again. If output were buffered
    // until exit, nothing would be readable before the sleep completes.
    let (tx, rx) = std::sync::mpsc::channel::<std::time::Duration>();
    let started = Instant::now();
    let handle = std::thread::spawn(move || {
        struct Probe(std::sync::mpsc::Sender<std::time::Duration>, Instant, bool);
        impl std::io::Write for Probe {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                if !self.2 {
                    self.2 = true;
                    let _ = self.0.send(self.1.elapsed());
                }
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut probe = Probe(tx, started, false);
        let _ = agent::run(&spec("fake-agent-slow"), "x", &mut probe, None);
    });
    let first = rx
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("first byte must arrive before the agent exits");
    assert!(
        first < std::time::Duration::from_secs(2),
        "first byte took {first:?}"
    );
    // Join rather than leave this loose: agent::run inside the thread does
    // not return until fake-agent-slow's full ~3s lifetime is over (it
    // waits for the child same as always), so without this join both the
    // thread and the still-sleeping agent would outlive the test's own
    // pass/fail. Joining is what makes this test's ~3s wall time a
    // deliberate wait for a clean finish rather than an orphaned leak.
    handle.join().expect("probe thread must not panic");
}

#[test]
fn a_broken_output_stream_kills_the_agent_instead_of_leaking_it() {
    // Simulates ama's own stdout breaking mid-stream (e.g. the user piped
    // `ama` into something that exited early). The agent here prints once
    // (tripping the broken-pipe failure inside Robot on the very first
    // write), sleeps, and -- only if left running to completion -- touches
    // a marker file afterwards.
    //
    // A bare timing assertion is not enough to pin this: the original bug
    // (no cleanup at all on this path) *also* returns promptly, because it
    // skips `wait()` entirely rather than blocking on it. What actually
    // distinguishes "killed and reaped" from "abandoned to run loose" is
    // whether the agent ever reaches the `touch` -- so this test waits
    // past the agent's internal sleep and checks the marker never appears.
    struct BrokenPipe;
    impl std::io::Write for BrokenPipe {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "simulated: ama's own stdout is gone",
            ))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("reached-the-end");
    let script = format!(
        "cat >/dev/null; echo first; sleep 1.5; touch '{}'",
        marker.display()
    );
    let s = AgentSpec {
        argv: vec!["bash".into(), "-c".into(), script],
        prompt_via: PromptVia::Stdin,
    };

    let started = std::time::Instant::now();
    let err = agent::run(&s, "x", &mut BrokenPipe, None).unwrap_err();
    let elapsed = started.elapsed();

    assert!(
        matches!(err, agent::AgentError::Io { .. }),
        "expected AgentError::Io, got: {err}"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(1),
        "run should fail promptly by killing the agent, not wait out its sleep: {elapsed:?}"
    );

    // Give a left-running agent ample time (1s margin) to reach the
    // `touch` on its own before checking it never got there.
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(
        !marker.exists(),
        "agent kept running after `run` returned instead of being killed and reaped"
    );
}

// ---- the moon (0.1.2) -------------------------------------------------------

/// One terminal that the spinner, the agent's routed stderr and the answer
/// all write to -- the only way to see the order they land in.
#[derive(Clone, Default)]
struct Screen(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl Screen {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("screen")).into_owned()
    }
}

impl std::io::Write for Screen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("screen").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Longer than any of these tests should take, so the only frame ever drawn
/// is the one `Spinner::start` draws itself, and the expected bytes are exact.
const NO_SECOND_FRAME: std::time::Duration = std::time::Duration::from_secs(10);

fn bash(script: &str) -> AgentSpec {
    AgentSpec {
        argv: vec!["bash".into(), "-c".into(), script.into()],
        prompt_via: PromptVia::Stdin,
    }
}

#[test]
fn the_moon_is_erased_before_the_answer_starts() {
    // REQ-36, and NFR-11: the erase is prompt, not a wait for the next frame.
    let screen = Screen::default();
    let spinner = Spinner::start(Box::new(screen.clone()), None, NO_SECOND_FRAME);
    let started = std::time::Instant::now();
    let code = agent::run(
        &spec("fake-agent-echo"),
        "hello prompt",
        &mut screen.clone(),
        Some(&spinner),
    )
    .expect("run");
    assert_eq!(code, 0);
    assert_eq!(screen.text(), "\r\x1b[K🤖 🌑\r\x1b[K🤖: hello prompt\n");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the answer waited {:?} for the moon",
        started.elapsed()
    );
}

#[test]
fn a_silent_agent_leaves_no_moon_behind() {
    // `run` prints its own "exited without producing any output" diagnostic,
    // so the moon has to be gone before `run` returns -- the caller's drop
    // comes too late for that line.
    let screen = Screen::default();
    let spinner = Spinner::start(Box::new(screen.clone()), None, NO_SECOND_FRAME);
    let code = agent::run(
        &spec("fake-agent-silent"),
        "x",
        &mut Vec::new(),
        Some(&spinner),
    )
    .expect("run");
    assert_eq!(code, 1, "silence is still not success");
    assert_eq!(screen.text(), "\r\x1b[K🤖 🌑\r\x1b[K");
}

#[test]
fn the_agents_stderr_is_routed_around_the_moon() {
    // REQ-38. The sleep orders the two streams: `warn` is on the screen
    // (and the moon redrawn below it) well before the answer begins.
    let screen = Screen::default();
    let spinner = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        NO_SECOND_FRAME,
    );
    let code = agent::run(
        &bash("cat >/dev/null; echo warn >&2; sleep 0.3; echo answer"),
        "x",
        &mut screen.clone(),
        Some(&spinner),
    )
    .expect("run");
    assert_eq!(code, 0);
    assert_eq!(
        screen.text(),
        concat!(
            "\r\x1b[K🤖 🌑", // drawn before the agent starts
            "\r\x1b[K",      // erased for the stderr line
            "warn\n",
            "\r\x1b[K🤖 🌑", // back, on the line below it
            "\r\x1b[K",      // erased for the answer
            "🤖: answer\n",
        )
    );
}

#[test]
fn a_grandchild_holding_the_stderr_pipe_does_not_hold_up_the_turn() {
    // RISK-14. The agent exits at once, but leaves behind a process that
    // inherited its stderr -- here, the pipe `ama` reads -- and keeps it open
    // for 5 s. Waiting for that pipe to close would hang the user's shell
    // for as long as the grandchild lives.
    let screen = Screen::default();
    let spinner = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        NO_SECOND_FRAME,
    );
    let started = std::time::Instant::now();
    let code = agent::run(
        &bash("cat >/dev/null; sleep 5 >/dev/null & echo answer"),
        "x",
        &mut Vec::new(),
        Some(&spinner),
    )
    .expect("run");
    assert_eq!(code, 0);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "the turn waited {:?} on a grandchild's pipe",
        started.elapsed()
    );
}

#[test]
fn a_spawn_failure_leaves_no_moon_behind() {
    // Review finding M10: `run` must leave the moon erased on *every* path,
    // not rely on its caller to remember -- the caller is about to print
    // "could not start ...", and that line must not start beside a frame.
    let screen = Screen::default();
    let spinner = Spinner::start(Box::new(screen.clone()), None, NO_SECOND_FRAME);
    let s = AgentSpec {
        argv: vec!["definitely-not-installed".into()],
        prompt_via: PromptVia::Stdin,
    };
    assert!(agent::run(&s, "x", &mut Vec::new(), Some(&spinner)).is_err());
    assert_eq!(screen.text(), "\r\x1b[K🤖 🌑\r\x1b[K");
}

#[test]
fn a_slow_terminal_still_gets_the_whole_of_the_agents_stderr() {
    // Review finding M5. When the agent exits, up to a pipe buffer of its
    // stderr can still be on its way to the screen, and on a slow terminal (an
    // ssh session on a poor link) delivering it takes a while. The tail of a
    // big dump is usually where the error is, so `ama` must not give up on it
    // just because a single write is slow. Here every write takes 400 ms --
    // well past the 250 ms `ama` allows a pump that is *idle* -- so when the
    // agent exits, the pump is still busy delivering. Two writes' worth of
    // dump keeps the whole test far inside the 3 s cap, even on a loaded CI
    // runner with a 64 KiB pipe.
    //
    // The answer comes first on purpose. Printed last, it made the moon's
    // `stop` wait on the spinner lock behind a slow write, which silently
    // lengthened the grace period and let this test pass against the very
    // bug it is for.
    struct SlowTerminal(Screen);
    impl std::io::Write for SlowTerminal {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            std::thread::sleep(std::time::Duration::from_millis(400));
            self.0.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let screen = Screen::default();
    let spinner = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(SlowTerminal(screen.clone()))),
        NO_SECOND_FRAME,
    );
    // An answer, ~11 KB of stack trace, then the line that says what went wrong.
    let dump = "cat >/dev/null; \
                echo answer; \
                for i in $(seq 1 120); do \
                  echo \"  at frame $i of a very deep stack .............................................\"; \
                done >&2; \
                echo 'error: THE REAL CAUSE' >&2";
    let code = agent::run(&bash(dump), "x", &mut Vec::new(), Some(&spinner)).expect("run");
    assert_eq!(code, 0);
    assert!(
        screen.text().contains("error: THE REAL CAUSE"),
        "the tail of the agent's stderr was cut off"
    );
}

#[test]
fn a_grandchild_that_never_stops_writing_cannot_hold_the_turn_forever() {
    // Re-review finding N1. A grandchild that writes to the inherited stderr
    // pipe every 50 ms never lets the pump sit idle for 250 ms, so the idle
    // rule never fires; only the 3 s cap ends the wait. (This one gives up by
    // itself after 10 s, so nothing outlives the test run for long.)
    let screen = Screen::default();
    let spinner = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        NO_SECOND_FRAME,
    );
    let started = std::time::Instant::now();
    let code = agent::run(
        &bash(
            "cat >/dev/null; \
             (for i in $(seq 1 200); do echo tick >&2; sleep 0.05; done) >/dev/null & \
             echo answer",
        ),
        "x",
        &mut Vec::new(),
        Some(&spinner),
    )
    .expect("run");
    assert_eq!(code, 0);
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_secs(5),
        "the turn waited {took:?} on a grandchild that never stops writing"
    );
}
