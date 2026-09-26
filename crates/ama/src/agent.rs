//! Spawn the user's agent, feed it the prompt, stream its answer back.

use crate::config::{AgentSpec, PromptVia};
use crate::render::Robot;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("no agent command configured")]
    NoCommand,
    #[error(
        "could not start `{program}`: {source}\n\nCheck `agent:` in your config, or run `ama doctor`."
    )]
    Spawn {
        program: String,
        source: std::io::Error,
    },
    #[error("failed while talking to `{program}`: {source}")]
    Io {
        program: String,
        source: std::io::Error,
    },
}

/// Run the agent. Returns its exit code; streams stdout through `out`.
/// The agent's stderr is inherited so its diagnostics reach the user directly.
pub fn run(spec: &AgentSpec, input: &str, out: &mut dyn Write) -> Result<i32, AgentError> {
    let Some(program) = spec.argv.first().cloned() else {
        return Err(AgentError::NoCommand);
    };

    let mut argv = spec.argv.clone();
    let feed_stdin = match spec.prompt_via {
        PromptVia::Arg(i) if i < argv.len() => {
            argv[i] = argv[i].replace("{prompt}", input);
            false
        }
        _ => true,
    };

    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(if feed_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|source| AgentError::Spawn {
            program: program.clone(),
            source,
        })?;

    // Write the prompt on a worker thread: an agent that emits output before
    // draining stdin would otherwise deadlock against a full pipe.
    let writer = feed_stdin.then(|| {
        let mut sink = child.stdin.take();
        let payload = input.to_string();
        std::thread::spawn(move || {
            if let Some(mut s) = sink.take() {
                let _ = s.write_all(payload.as_bytes());
            }
        })
    });

    // Stream stdout through Robot. The result is captured rather than
    // returned immediately: the child and the stdin-writer thread must be
    // cleaned up below on *every* exit from this section, not just the
    // happy one -- otherwise a broken `out` (e.g. our own stdout pipe
    // closing because the user piped `ama` into something that exited
    // early) would abandon a still-running agent instead of reaping it.
    let mut robot = Robot::new(out);
    let stream_result: Result<(), AgentError> = (|| {
        if let Some(mut stdout) = child.stdout.take() {
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => robot
                        .write_all(&buf[..n])
                        .map_err(|source| AgentError::Io {
                            program: program.clone(),
                            source,
                        })?,
                    Err(source) => {
                        return Err(AgentError::Io {
                            program: program.clone(),
                            source,
                        });
                    }
                }
            }
        }
        robot.finish().map_err(|source| AgentError::Io {
            program: program.clone(),
            source,
        })
    })();
    let produced = robot.wrote_anything();

    // On failure the agent may still be alive: blocked writing to a stdout
    // pipe nobody drains any more, or blocked reading stdin because it
    // cannot make progress until that write unblocks. Kill it first so
    // neither the writer thread's join nor `wait()` below can hang on a
    // process that would otherwise never exit on its own.
    if stream_result.is_err() {
        let _ = child.kill();
    }

    if let Some(h) = writer {
        let _ = h.join();
    }
    let wait_result = child.wait();

    // The streaming failure is the more useful diagnostic (e.g. our own
    // stdout broke) than any secondary error from wait() on a process we
    // just killed, but wait() is still called unconditionally above so the
    // child is always reaped rather than leaked as a zombie or orphan.
    let status = match stream_result {
        Err(e) => {
            let _ = wait_result;
            return Err(e);
        }
        Ok(()) => wait_result.map_err(|source| AgentError::Io {
            program: program.clone(),
            source,
        })?,
    };

    // Review Focus 5: a silent, successful agent is reported, not mistaken
    // for a good answer.
    if status.success() && !produced {
        eprintln!("ama: `{program}` exited without producing any output.");
        return Ok(1);
    }
    Ok(status.code().unwrap_or(1))
}
