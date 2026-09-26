use ama::context;
use ama::prompt;

fn lines(s: &str) -> Vec<String> {
    s.lines().map(String::from).collect()
}

#[test]
fn context_is_everything_from_the_first_trigger_line_down() {
    let pane = lines(
        "some earlier output\n\
         user@host:~$ ls\n\
         a.rs  b.rs\n\
         user@host:~$ @@ 'what is here'\n\
         🤖: two rust files\n\
         user@host:~$ @@ 'and now'",
    );
    let got = context::slice_from_first_trigger(&pane);
    assert_eq!(got.len(), 3);
    assert!(got[0].contains("@@ 'what is here'"));
    assert!(got[2].contains("and now"));
}

#[test]
fn an_empty_pane_yields_no_context() {
    assert!(context::slice_from_first_trigger(&[]).is_empty());
}

#[test]
fn a_pane_with_no_trigger_yields_no_context() {
    // After `clear` there is no trigger on screen, so the conversation is new.
    assert!(context::slice_from_first_trigger(&lines("user@host:~$ ")).is_empty());
}

#[test]
fn the_whole_conversation_from_the_first_question_down_is_context() {
    // The agent explained the tool, so its answer contains "@@ ". This pins
    // the slice's start and length when the real question is already the
    // first line, but that placement means `position()`'s leftmost match
    // lands there regardless of the `is_answer_line` guard -- it does NOT
    // exercise the guard. `a_stray_answer_fragment_before_the_real_trigger_does_not_pollute_the_start`
    // below is the test that does.
    let pane = lines(
        "user@host:~$ @@ 'how do i use this'\n\
         🤖: type @@ followed by a space, like\n   \
         @@ what is this project\n\
         user@host:~$ @@ 'thanks'",
    );
    let got = context::slice_from_first_trigger(&pane);
    assert!(
        got[0].contains("how do i use this"),
        "context must start at the real first question, got {:?}",
        got.first()
    );
    assert_eq!(got.len(), 4);
}

// Beyond the brief: the test above places the real first question at pane
// index 0, so a naive `position(|l| l.contains("@@ "))` with no
// `is_answer_line` guard at all would return exactly the same slice --
// leftmost-match already lands on index 0 either way, so that test alone does
// not prove the guard is doing anything (verified: it still passes with the
// guard removed). This fixture puts a stray answer fragment *before* the only
// reachable trigger, which does distinguish the two.
#[test]
fn a_stray_answer_fragment_before_the_real_trigger_does_not_pollute_the_start() {
    let pane = lines("🤖: like @@ this\nuser@host:~$ @@ 'real question'");
    let got = context::slice_from_first_trigger(&pane);
    assert_eq!(
        got.len(),
        1,
        "the stray fragment must not be included, got {got:?}"
    );
    assert!(got[0].contains("real question"));
}

#[test]
fn trailing_blank_rows_below_the_cursor_are_dropped() {
    // What `tmux capture-pane -p` returns for a pane where only the first
    // two rows have ever been written to: the real content, then padding
    // out to the pane's full height.
    let mut pane = lines("user@host:~$ @@ 'hi'\n🤖: hello");
    pane.extend((0..20).map(|_| String::new()));
    assert_eq!(context::trim_trailing_blank(&pane).len(), 2);
}

#[test]
fn a_wholly_blank_pane_trims_to_nothing() {
    let pane = lines("\n\n\n");
    assert!(context::trim_trailing_blank(&pane).is_empty());
}

#[test]
fn trim_trailing_blank_leaves_an_already_full_pane_untouched() {
    let pane = lines("user@host:~$ @@ 'hi'\n🤖: hello");
    assert_eq!(context::trim_trailing_blank(&pane), pane.as_slice());
}

#[test]
fn a_blank_line_in_the_middle_is_not_trimmed_only_the_trailing_run_is() {
    // A blank line the user's own command legitimately produced (or a blank
    // line inside a multi-line pasted question) must survive; only the
    // padding run at the very end is padding.
    let pane = lines("user@host:~$ @@ 'hi'\n\n🤖: hello");
    let got = context::trim_trailing_blank(&pane);
    assert_eq!(got.len(), 3, "got {got:?}");
}

#[test]
fn trimming_before_slicing_is_the_order_gather_uses_and_it_matters() {
    // `gather` calls `trim_trailing_blank` before `slice_from_first_trigger`
    // (not after): trimming first shortens the slice's search space so
    // padding can never survive past the trigger line either. Padding after
    // the trigger must not reach the agent as bloat, and -- echoed back by a
    // verification fixture in a real terminal -- is exactly what pushed
    // genuine content off the visible pane before Task 7's tmux suite ever
    // observed it.
    let mut pane = lines("user@host:~$ @@ 'hi'");
    pane.extend((0..20).map(|_| String::new()));
    let got = context::slice_from_first_trigger(context::trim_trailing_blank(&pane));
    assert_eq!(got.len(), 1, "padding leaked into context: {got:?}");
}

#[test]
fn the_cap_keeps_the_most_recent_lines() {
    let many: Vec<String> = (0..1000).map(|i| format!("line {i}")).collect();
    let got = context::cap(&many, 200);
    assert_eq!(got.len(), 200);
    assert_eq!(got[0], "line 800");
    assert_eq!(got[199], "line 999");
}

#[test]
fn the_cap_is_a_no_op_below_the_limit() {
    let few: Vec<String> = (0..5).map(|i| format!("line {i}")).collect();
    assert_eq!(context::cap(&few, 200).len(), 5);
}

#[test]
fn multibyte_content_survives_slicing_and_capping() {
    let pane = lines("$ @@ 'なにこれ 🤖'\n🤖: 日本語です");
    let got = context::slice_from_first_trigger(&pane);
    assert!(got[0].contains("なにこれ 🤖"));
    assert!(got[1].contains("日本語です"));
}

#[test]
fn a_composed_prompt_carries_the_question_and_the_terminal() {
    let out = prompt::compose(&lines("$ ls\na.rs"), "what is here?");
    assert!(out.contains("what is here?"));
    assert!(out.contains("$ ls"));
    assert!(out.contains("a.rs"));
    assert!(out.contains("## Terminal"));
}

#[test]
fn an_empty_context_omits_the_terminal_section_entirely() {
    let out = prompt::compose(&[], "hello");
    assert!(!out.contains("## Terminal"), "no empty section: {out}");
    assert!(out.contains("hello"));
}
