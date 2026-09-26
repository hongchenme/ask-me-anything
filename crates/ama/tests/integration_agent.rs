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
    std::thread::spawn(move || {
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
}
