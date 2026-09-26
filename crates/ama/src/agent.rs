//! Spawn the user's agent, feed it the prompt, stream its answer back.

use crate::config::{AgentSpec, PromptVia};
use crate::render::Robot;
use crate::spinner::{Spinner, StderrSink};
use std::io::{Read, Write};
use std::process::{ChildStderr, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Once the agent has exited, how long its stderr pump may sit *waiting* on
/// the pipe before `ama` stops waiting for it. By then everything the agent
/// wrote is already in the pipe, so a pump still waiting after this is
/// waiting on a grandchild that inherited the pipe and outlives the agent --
/// which would hang the user's shell for the grandchild's lifetime (RISK-14).
const STDERR_IDLE: Duration = Duration::from_millis(250);

/// The most `ama` waits for stderr once the agent has exited, however
/// steadily it keeps arriving.
const STDERR_CAP: Duration = Duration::from_secs(3);

/// Stops the moon when `run` returns, by whichever path. `run` stops it
/// sooner wherever it prints something itself; this covers the returns after
/// which its caller will.
struct EraseOnReturn<'a>(Option<&'a Spinner>);

impl Drop for EraseOnReturn<'_> {
    fn drop(&mut self) {
        if let Some(s) = self.0 {
            s.stop();
        }
    }
}

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
///
/// The agent's stderr is inherited so its diagnostics reach the user
/// directly -- except while `spinner` is drawing on the same terminal, when
/// it is routed through the spinner instead (ADR-012), so that no stderr
/// line can land on the moon's line.
///
/// `spinner` is stopped the moment the answer starts, before any diagnostic
/// `run` prints itself, and in every case by the time `run` returns -- so a
/// caller can print an error straight away, spawn failures included.
pub fn run(
    spec: &AgentSpec,
    input: &str,
    out: &mut dyn Write,
    spinner: Option<&Spinner>,
) -> Result<i32, AgentError> {
    let _erase = EraseOnReturn(spinner);
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

    let stderr_sink = spinner.and_then(Spinner::stderr_sink);
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(if feed_stdin {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(if stderr_sink.is_some() {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .spawn()
        .map_err(|source| AgentError::Spawn {
            program: program.clone(),
            source,
        })?;

    // Copy the agent's stderr through the spinner for as long as the agent
    // -- or anything it left holding the pipe -- keeps writing.
    let stderr_pump = stderr_sink.and_then(|sink| {
        let pipe = child.stderr.take()?;
        let (tell, status) = mpsc::channel();
        let pump = std::thread::spawn(move || pump_stderr(pipe, sink, &tell));
        Some((pump, status))
    });

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
    // Taken on the first chunk, so the rest of the answer never waits on the
    // spinner's lock behind a stderr write.
    let mut moon = spinner;
    let stream_result: Result<(), AgentError> = (|| {
        if let Some(mut stdout) = child.stdout.take() {
            let mut buf = [0u8; 8192];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        // REQ-36: the moon goes the instant the answer starts.
                        if let Some(s) = moon.take() {
                            s.stop();
                        }
                        robot
                            .write_all(&buf[..n])
                            .map_err(|source| AgentError::Io {
                                program: program.clone(),
                                source,
                            })?
                    }
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

    // Streaming is over: the agent closed stdout, or our own output broke.
    // Either way the moon must be gone before anything below -- or the
    // caller -- prints a diagnostic.
    if let Some(s) = spinner {
        s.stop();
    }

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
    if let Some((pump, status)) = stderr_pump {
        finish_stderr(pump, &status);
    }

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

/// What the stderr pump is doing. It reports every change, so that
/// `finish_stderr` can tell a pump busy delivering to a slow terminal from
/// one waiting on a pipe that nothing will write to again.
enum Pump {
    Reading,
    Writing,
}

fn pump_stderr(mut pipe: ChildStderr, mut sink: StderrSink, tell: &mpsc::Sender<Pump>) {
    let mut buf = [0u8; 8192];
    loop {
        let _ = tell.send(Pump::Reading);
        let n = match pipe.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return,
        };
        let _ = tell.send(Pump::Writing);
        // Never fails: `StderrSink` swallows a broken terminal, so the pipe
        // is always drained to the end.
        let _ = sink.write_all(&buf[..n]);
    }
}

/// Wait for the pump to deliver what the agent, now exited, left in the pipe.
///
/// A pump that is writing is waited for: a slow terminal can take a while,
/// and the tail of a big stderr dump is usually where the error is. A pump
/// that has sat reading for `STDERR_IDLE` is left to end with the process,
/// because the pipe is being held open by a grandchild (RISK-14).
/// `STDERR_CAP` bounds the whole wait either way.
///
/// Every state change since the agent started is still queued -- two small
/// messages per `read`, which usually means per line of stderr -- and is
/// replayed here in order, so the last one says what the pump is doing now.
/// The queue grows with the agent's stderr (some 16 bytes a message) and is
/// freed when the turn ends.
fn finish_stderr(pump: JoinHandle<()>, status: &mpsc::Receiver<Pump>) {
    let deadline = Instant::now() + STDERR_CAP;
    let mut reading = false;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        match status.recv_timeout(if reading { left.min(STDERR_IDLE) } else { left }) {
            Ok(Pump::Reading) => reading = true,
            Ok(Pump::Writing) => reading = false,
            // The pipe closed and the pump returned: every byte was delivered.
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = pump.join();
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => return,
        }
    }
}
