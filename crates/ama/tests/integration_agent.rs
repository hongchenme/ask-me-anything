use ama::agent;
use ama::config::{AgentSpec, PromptVia};

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
    let code = agent::run(&spec("fake-agent-echo"), "hello prompt", &mut out).expect("run");
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
    agent::run(&s, "substituted", &mut out).expect("run");
    assert_eq!(String::from_utf8_lossy(&out), "🤖: substituted\n");
}

#[test]
fn a_failing_agent_reports_its_exit_code() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-fail"), "x", &mut out).expect("run");
    assert_eq!(code, 7);
}

#[test]
fn a_missing_agent_is_a_typed_error_naming_the_program() {
    let s = AgentSpec {
        argv: vec!["definitely-not-installed".into()],
        prompt_via: PromptVia::Stdin,
    };
    let err = agent::run(&s, "x", &mut Vec::new()).unwrap_err();
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
    assert!(agent::run(&s, "x", &mut Vec::new()).is_err());
}

#[test]
fn the_agent_inherits_the_working_directory() {
    let mut out: Vec<u8> = Vec::new();
    agent::run(&spec("fake-agent-cwd"), "x", &mut out).expect("run");
    let cwd = std::env::current_dir().expect("cwd");
    assert!(String::from_utf8_lossy(&out).contains(&cwd.to_string_lossy().to_string()));
}

#[test]
fn a_silent_agent_is_reported_rather_than_looking_like_success() {
    let mut out: Vec<u8> = Vec::new();
    let code = agent::run(&spec("fake-agent-silent"), "x", &mut out).expect("run");
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
        let _ = agent::run(&spec("fake-agent-slow"), "x", &mut probe);
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
    let err = agent::run(&s, "x", &mut BrokenPipe).unwrap_err();
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
