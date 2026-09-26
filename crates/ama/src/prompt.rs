//! Compose the text handed to the agent on stdin.

const PREAMBLE: &str = "\
You are answering inside a terminal, inline, while the user works. Be brief and \
concrete: a few lines, plain prose, no headings and no preamble. If the terminal \
transcript below is relevant to the question, use it; if it is not, ignore it.";

pub fn compose(context: &[String], question: &str) -> String {
    let mut s = String::with_capacity(PREAMBLE.len() + question.len() + 256);
    s.push_str(PREAMBLE);
    if !context.is_empty() {
        s.push_str("\n\n## Terminal\n\n```\n");
        for line in context {
            s.push_str(line);
            s.push('\n');
        }
        s.push_str("```\n");
    }
    s.push_str("\n## Question\n\n");
    s.push_str(question);
    s.push('\n');
    s
}
