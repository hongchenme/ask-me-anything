use ama::session;

#[test]
fn tmux_pane_wins_and_is_made_filesystem_safe() {
    let k = session::key_from(Some("%3"), Some("4711"), Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "tmux-3");
}

#[test]
fn falls_back_to_the_shell_supplied_session_id() {
    let k = session::key_from(None, Some("4711"), Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "sh-4711");
}

#[test]
fn falls_back_to_the_tty_path() {
    let k = session::key_from(None, None, Some("/dev/pts/2"));
    assert_eq!(k.to_string(), "tty-dev-pts-2");
}

#[test]
fn has_a_last_resort_key() {
    assert_eq!(session::key_from(None, None, None).to_string(), "default");
}

#[test]
fn strips_characters_that_are_not_safe_in_a_file_name() {
    let k = session::key_from(None, Some("../../etc/passwd"), None);
    let s = k.to_string();
    assert!(!s.contains('/'), "no separators: {s}");
    assert!(!s.contains(".."), "no traversal: {s}");
}
