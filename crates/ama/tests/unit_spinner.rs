use ama::spinner::Spinner;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Carriage return, then erase to the end of the line.
const ERASE: &str = "\r\x1b[K";

/// A frame interval no test lives long enough to reach, so exactly one frame
/// -- the one `start` draws itself -- is ever on screen.
const NEVER: Duration = Duration::from_secs(10);

/// One terminal that every writer in a test shares. The stderr routing is
/// all about the *order* of writes across two streams, and only a single
/// shared buffer can show it.
#[derive(Clone, Default)]
struct Screen(Arc<Mutex<Vec<u8>>>);

impl Screen {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("screen")).into_owned()
    }
}

impl Write for Screen {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("screen").extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn the_first_frame_is_on_screen_before_start_returns() {
    let screen = Screen::default();
    let s = Spinner::start(Box::new(screen.clone()), None, NEVER);
    assert_eq!(screen.text(), "\r\x1b[K🤖 🌑");
    s.stop();
}

#[test]
fn frames_run_through_the_eight_moon_phases_and_wrap() {
    let screen = Screen::default();
    let s = Spinner::start(Box::new(screen.clone()), None, Duration::from_millis(2));
    wait_until("ten frames", || screen.text().matches('🤖').count() >= 10);
    s.stop();

    let text = screen.text();
    let moons: Vec<&str> = text
        .split(ERASE)
        .filter(|f| !f.is_empty())
        .map(|f| {
            f.strip_prefix("🤖 ")
                .expect("every frame is the robot, a space, a moon")
        })
        .collect();
    assert_eq!(
        moons[..10],
        ["🌑", "🌒", "🌓", "🌔", "🌕", "🌖", "🌗", "🌘", "🌑", "🌒"]
    );
}

#[test]
fn stop_erases_the_moon_and_nothing_is_drawn_after_it() {
    let screen = Screen::default();
    let s = Spinner::start(Box::new(screen.clone()), None, Duration::from_millis(2));
    std::thread::sleep(Duration::from_millis(20));
    s.stop();

    let at_stop = screen.text();
    assert!(at_stop.ends_with(ERASE), "not erased: {at_stop:?}");
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(screen.text(), at_stop, "a frame was drawn after stop");
}

#[test]
fn dropping_the_spinner_erases_the_moon() {
    // `ama` relies on this on every early-return path: whatever happens, the
    // moon must be gone before a diagnostic can print next to it.
    let screen = Screen::default();
    drop(Spinner::start(Box::new(screen.clone()), None, NEVER));
    assert_eq!(screen.text(), "\r\x1b[K🤖 🌑\r\x1b[K");
}

#[test]
fn stop_does_not_wait_out_the_frame_interval() {
    // NFR-11: the answer is held back until `stop` returns.
    let s = Spinner::start(Box::new(Screen::default()), None, NEVER);
    // Let the drawing thread get into its 10 s wait first. Stopped any
    // sooner, it sees the flag before it ever sleeps and exits without
    // needing to be woken -- which is how this test once passed with the
    // wake-up deleted.
    std::thread::sleep(Duration::from_millis(50));
    let started = Instant::now();
    s.stop();
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "stop took {:?}",
        started.elapsed()
    );
}

#[test]
fn a_stderr_line_lands_on_a_cleared_line_and_the_moon_redraws_below_it() {
    let screen = Screen::default();
    let s = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        NEVER,
    );
    let mut err = s
        .stderr_sink()
        .expect("a spinner given a stderr writer routes stderr");
    err.write_all(b"progress one\n").expect("write");
    s.stop();

    assert_eq!(
        screen.text(),
        concat!(
            "\r\x1b[K🤖 🌑", // start
            "\r\x1b[K",      // erased for the stderr line
            "progress one\n",
            "\r\x1b[K🤖 🌑", // redrawn on the line below it
            "\r\x1b[K",      // stop
        )
    );
}

#[test]
fn a_partial_stderr_line_holds_the_moon_until_the_line_completes() {
    let screen = Screen::default();
    let s = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        Duration::from_millis(2),
    );
    let mut err = s.stderr_sink().expect("routes stderr");

    err.write_all(b"partial").expect("write");
    let held = screen.text();
    assert!(held.ends_with("partial"), "{held:?}");
    std::thread::sleep(Duration::from_millis(40)); // ~20 frame intervals
    assert_eq!(
        screen.text(),
        held,
        "a frame was drawn over an unfinished stderr line"
    );

    err.write_all(b" line\n").expect("write");
    let after = screen.text();
    let tail = after.strip_prefix(held.as_str()).expect("appended");
    assert!(
        tail.starts_with(" line\n\r\x1b[K🤖 "),
        "the moon did not come back below the finished line: {tail:?}"
    );
    s.stop();
}

#[test]
fn after_stop_stderr_passes_through_untouched() {
    let screen = Screen::default();
    let s = Spinner::start(
        Box::new(screen.clone()),
        Some(Box::new(screen.clone())),
        NEVER,
    );
    let mut err = s.stderr_sink().expect("routes stderr");
    s.stop();

    let before = screen.text();
    err.write_all(b"late\n").expect("write");
    assert_eq!(screen.text(), format!("{before}late\n"));
}

#[test]
fn without_a_stderr_writer_there_is_nothing_to_route() {
    // `agent::run` pipes the agent's stderr only when this is `Some`. A sink
    // with nowhere to write would swallow the agent's diagnostics whole.
    let s = Spinner::start(Box::new(Screen::default()), None, NEVER);
    assert!(s.stderr_sink().is_none());
}

#[test]
fn a_terminal_that_refuses_every_write_does_not_stop_stderr_draining() {
    // NFR-12. The stderr pump copies the agent's pipe until end-of-file; if
    // the sink reported the terminal's failure, the copy would stop, the
    // pipe would fill, and the agent would block forever on its next write.
    struct Refuses;
    impl Write for Refuses {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("terminal gone"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Err(std::io::Error::other("terminal gone"))
        }
    }

    let s = Spinner::start(
        Box::new(Refuses),
        Some(Box::new(Refuses)),
        Duration::from_millis(2),
    );
    let mut err = s.stderr_sink().expect("routes stderr");
    err.write_all(b"diagnostic\n")
        .expect("the sink must swallow the terminal's failure");
    std::thread::sleep(Duration::from_millis(10));
    s.stop();
}
