//! The moon shown while the agent thinks (REQ-35…REQ-39, ADR-010…ADR-013).
//!
//! From the moment a question is sent until the agent's first byte arrives,
//! the line the answer will take over shows `🤖` and a moon going through its
//! phases. Without it, the seconds an agent spends thinking look exactly like
//! a hung command: `claude -p` is silent for 6.1 s before answering the single
//! word "ok" (F-13).
//!
//! Three things make this more than a loop that prints glyphs, and all three
//! follow from the screen being the next turn's prompt (ADR-002):
//!
//! - **Nothing may be left behind.** Every frame is drawn as `\r\x1b[K` then
//!   `🤖 <moon>`, and `stop` erases with the same `\r\x1b[K`. That was measured
//!   in a real tmux pane to leave no cell behind for `capture-pane` (F-16).
//!   Residue would be sent to the agent as if the user had typed it.
//! - **The answer waits for the erase, not for the clock.** `stop` wakes the
//!   drawing thread instead of letting it finish its 100 ms, and joins it, so
//!   no frame can follow the erase (NFR-11).
//! - **The agent's stderr shares this line** (ADR-012). codex writes thirty
//!   lines of progress while it thinks (F-14). [`StderrSink`] erases the moon
//!   before each chunk and redraws it below a finished line, and holds it
//!   while a line is unfinished. A stderr line never gets a moon glued on,
//!   and a moon is never drawn over half a line.
//!
//! The cursor is deliberately never hidden. `ama` has no SIGINT handler, so a
//! Ctrl-C between hide and show would leave the user's terminal without a
//! cursor (RISK-15). An interrupted moon leaves one visible frame instead.

use std::io::{self, IsTerminal, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

/// New moon to waning crescent (REQ-35).
const PHASES: [&str; 8] = ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘"];

/// A whole lunar cycle every 0.8 s.
const EVERY: Duration = Duration::from_millis(100);

/// Carriage return, then erase to the end of the line.
const ERASE: &str = "\r\x1b[K";

/// The robot of `context::ANSWER_PREFIX`. The answer's own `🤖: ` lands in the
/// same cells, so on screen the moon simply turns into the answer.
const ROBOT: &str = "🤖";

type Out = Box<dyn Write + Send>;

pub struct Spinner {
    shared: Arc<Shared>,
    drawing: Mutex<Option<JoinHandle<()>>>,
    /// Set once the moon is erased for good, and only then -- so a `stop`
    /// that finds it set can return without taking the screen lock, which a
    /// stderr write to a slow terminal may be holding.
    erased: AtomicBool,
}

/// Where the agent's stderr goes while the moon is up (ADR-012). Always
/// reports success, so the pump feeding it drains the agent's pipe to
/// end-of-file even if the terminal is gone: a pipe nobody drains fills up,
/// and the agent then blocks forever on its next write.
pub struct StderrSink {
    shared: Arc<Shared>,
}

struct Shared {
    screen: Mutex<Screen>,
    wake: Condvar,
}

/// Both writers and the moon's state, behind one lock. The single lock is
/// what stops a frame landing between an erase and the stderr chunk that
/// erase made room for.
struct Screen {
    term: Out,
    stderr: Option<Out>,
    phase: usize,
    /// The moon is on screen, and must be erased before anything else is written.
    drawn: bool,
    /// The last stderr chunk ended mid-line, so a frame now would overwrite it.
    held: bool,
    stopped: bool,
}

impl Screen {
    // Every write result is ignored: the moon is decoration, and a terminal
    // that refuses it must not fail the turn (NFR-12).
    fn draw(&mut self) {
        let frame = format!("{ERASE}{ROBOT} {}", PHASES[self.phase]);
        let _ = self.term.write_all(frame.as_bytes());
        let _ = self.term.flush();
        self.drawn = true;
    }

    fn erase(&mut self) {
        if self.drawn {
            let _ = self.term.write_all(ERASE.as_bytes());
            let _ = self.term.flush();
            self.drawn = false;
        }
    }
}

impl Shared {
    fn screen(&self) -> MutexGuard<'_, Screen> {
        self.screen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Spinner {
    /// The moon for this process's terminal, or `None` when there is nobody to
    /// show it to (REQ-37).
    pub fn for_terminal() -> Option<Spinner> {
        let out = io::stdout();
        if !wanted(out.is_terminal(), std::env::var("TERM").ok().as_deref()) {
            return None;
        }
        // Route the agent's stderr only when it would land on this same
        // screen. Otherwise it is inherited, exactly as in 0.1.1 (ADR-012).
        let err = io::stderr();
        let stderr: Option<Out> = if err.is_terminal() {
            Some(Box::new(err))
        } else {
            None
        };
        Some(Spinner::start(Box::new(out), stderr, EVERY))
    }

    /// Draw the first frame on `term` before returning, then advance one phase
    /// every `every`. `stderr`, when given, is where [`StderrSink`] writes.
    pub fn start(term: Out, stderr: Option<Out>, every: Duration) -> Spinner {
        let shared = Arc::new(Shared {
            screen: Mutex::new(Screen {
                term,
                stderr,
                phase: 0,
                drawn: false,
                held: false,
                stopped: false,
            }),
            wake: Condvar::new(),
        });
        // REQ-35: on screen now, which is before the agent is even spawned.
        shared.screen().draw();
        let drawing = {
            let shared = Arc::clone(&shared);
            std::thread::Builder::new()
                .name("ama-spinner".into())
                .spawn(move || animate(&shared, every))
                // No thread means no animation: the first frame just stays
                // up until it is erased (NFR-12).
                .ok()
        };
        Spinner {
            shared,
            drawing: Mutex::new(drawing),
            erased: AtomicBool::new(false),
        }
    }

    /// Erase the moon and stop drawing it. Idempotent, and returns only once
    /// no further frame can be drawn. After the first call it costs nothing:
    /// the rest of the answer must never wait behind the agent's stderr.
    pub fn stop(&self) {
        if self.erased.load(Ordering::Acquire) {
            return;
        }
        {
            let mut screen = self.shared.screen();
            screen.stopped = true;
            screen.erase();
        }
        // Only now: `stopped` is set under the lock, so no frame can follow.
        self.erased.store(true, Ordering::Release);
        self.shared.wake.notify_all();
        let drawing = self
            .drawing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = drawing {
            let _ = thread.join();
        }
    }

    /// A writer for the agent's stderr, or `None` when this spinner has no
    /// stderr writer. `agent::run` pipes the agent's stderr only on `Some`.
    pub fn stderr_sink(&self) -> Option<StderrSink> {
        self.shared.screen().stderr.is_some().then(|| StderrSink {
            shared: Arc::clone(&self.shared),
        })
    }
}

impl Drop for Spinner {
    /// Every early return in `ama` must still leave a clean line behind.
    fn drop(&mut self) {
        self.stop();
    }
}

fn animate(shared: &Shared, every: Duration) {
    let mut screen = shared.screen();
    while !screen.stopped {
        screen = match shared.wake.wait_timeout(screen, every) {
            Ok((screen, _)) => screen,
            Err(poisoned) => poisoned.into_inner().0,
        };
        if !screen.stopped && !screen.held {
            screen.phase = (screen.phase + 1) % PHASES.len();
            screen.draw();
        }
    }
}

impl Write for StderrSink {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let mut screen = self.shared.screen();
        screen.erase();
        if let Some(err) = screen.stderr.as_mut() {
            let _ = err.write_all(buf);
            let _ = err.flush();
        }
        if !screen.stopped {
            screen.held = !buf.ends_with(b"\n");
            if !screen.held {
                screen.draw();
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// What is left of a captured screen line once a leading moon frame is taken
/// off, or `None` when the line does not start with one.
///
/// Normally a frame is erased before anything else is written, but the
/// user's own keys can strand one. Enter pressed mid-think is echoed by the
/// terminal and moves the cursor down beneath the frame, and Ctrl-C leaves
/// `🤖 🌓^C` behind (RISK-15, RISK-18). This lives here, beside the frame
/// format it has to match, so the two cannot drift apart.
pub fn strip_frame(line: &str) -> Option<&str> {
    let rest = line.strip_prefix(ROBOT)?.strip_prefix(' ')?;
    PHASES.iter().find_map(|phase| rest.strip_prefix(phase))
}

/// REQ-37: a terminal to draw on, and one that can erase a line.
fn wanted(stdout_is_terminal: bool, term: Option<&str>) -> bool {
    stdout_is_terminal && term != Some("dumb")
}

#[cfg(test)]
mod tests {
    use super::wanted;

    #[test]
    fn the_moon_needs_a_terminal_that_can_erase_a_line() {
        assert!(wanted(true, Some("xterm-256color")));
        assert!(wanted(true, Some("tmux-256color")));
        // Most terminals erase fine with TERM unset; only `dumb` says it cannot.
        assert!(wanted(true, None));
        // Emacs `M-x shell` and friends: `\033[K` would print literally.
        assert!(!wanted(true, Some("dumb")));
        // A pipe, a file, `$(...)`: nobody is watching, and the bytes are data.
        assert!(!wanted(false, Some("xterm-256color")));
        assert!(!wanted(false, None));
    }
}
