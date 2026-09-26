use ama::render::Robot;
use std::io::Write;

fn render(chunks: &[&str]) -> String {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut r = Robot::new(&mut buf);
        for c in chunks {
            r.write_all(c.as_bytes()).expect("write");
        }
        r.flush().expect("flush");
    }
    String::from_utf8(buf).expect("utf8")
}

#[test]
fn the_first_line_gets_the_robot_prefix() {
    assert_eq!(render(&["hello\n"]), "🤖: hello\n");
}

#[test]
fn continuation_lines_are_indented_not_re_prefixed() {
    assert_eq!(render(&["one\ntwo\n"]), "🤖: one\n   two\n");
}

#[test]
fn the_prefix_is_written_once_across_chunk_boundaries() {
    // Streaming means a line can arrive in pieces; the prefix must not repeat.
    assert_eq!(render(&["hel", "lo\nwor", "ld\n"]), "🤖: hello\n   world\n");
}

#[test]
fn output_without_a_trailing_newline_still_ends_the_line() {
    assert_eq!(render(&["no newline"]), "🤖: no newline\n");
}

#[test]
fn nothing_in_means_nothing_out() {
    assert_eq!(render(&[]), "");
}
