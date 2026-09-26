//! Prefix streamed agent output with the robot marker.

use crate::context::{ANSWER_PREFIX, CONT_INDENT};
use std::io::{self, Write};

/// Writes `🤖: ` before the first byte and indents every later line, so a
/// multi-line answer reads as one block and `context`'s slicing can tell
/// answers from commands -- both to find the conversation's start and to
/// count how many real triggers are on the pane (A-04).
pub struct Robot<W: Write> {
    inner: W,
    started: bool,
    at_line_start: bool,
}

impl<W: Write> Robot<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            started: false,
            at_line_start: true,
        }
    }

    /// False when the agent produced no output at all (Review Focus 5).
    pub fn wrote_anything(&self) -> bool {
        self.started
    }

    /// Terminate a final line that arrived without a newline.
    pub fn finish(&mut self) -> io::Result<()> {
        if self.started && !self.at_line_start {
            self.inner.write_all(b"\n")?;
            self.at_line_start = true;
        }
        self.inner.flush()
    }
}

impl<W: Write> Write for Robot<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        for &b in buf {
            if self.at_line_start {
                if !self.started {
                    self.inner.write_all(ANSWER_PREFIX.as_bytes())?;
                    self.started = true;
                } else {
                    self.inner.write_all(CONT_INDENT.as_bytes())?;
                }
                self.at_line_start = false;
            }
            self.inner.write_all(&[b])?;
            if b == b'\n' {
                self.at_line_start = true;
                // Flush per line so the user sees the answer as it arrives.
                self.inner.flush()?;
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.finish()
    }
}
