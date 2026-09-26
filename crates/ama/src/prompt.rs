//! Compose the text handed to the agent on stdin.

/// The second paragraph is ADR-008, and every clause in it answers an
/// observed failure rather than a guess.
///
/// `ama` hands its question to an agent whose own system prompt frames it as
/// a coding assistant for the current directory. Asked `what's the weather`,
/// `claude -p` replied "I don't have access to real-time weather data ...
/// Was there something coding or project-related I can help with instead?" --
/// its default persona showing through, on an agent that *can* search the
/// web. Granting the tool (ADR-007) is necessary and not sufficient: with
/// the grant in place and this paragraph absent, the same question still
/// came back "I don't have live weather access" (F-08).
///
/// So, clause by clause: "general question box" counters the refusal to
/// answer anything non-coding; "use those tools before answering anything
/// you do not already know" turns an available tool into a used one; "never
/// say you lack internet access ... without having tried" names the exact
/// false claim in the defect report; and "ask for just that" covers the
/// reported question, which has no location in it -- without that clause the
/// agent padded an otherwise reasonable "which city?" with an invented
/// incapacity.
///
/// It names capabilities, never a vendor's tool, so it reads correctly to an
/// agent with no web access at all: that agent tries, fails, and says so,
/// which is the honest outcome rather than a pre-emptive refusal.
const PREAMBLE: &str = "\
You are answering inside a terminal, inline, while the user works. Be brief and \
concrete: a few lines, plain prose, no headings and no preamble. If the terminal \
transcript below is relevant to the question, use it; if it is not, ignore it.\n\
\n\
Answer whatever is asked. This is a general question box, not only a coding \
assistant, and the question is often not about the current directory. You can \
search the web and read this machine, so use those tools before answering \
anything you do not already know, and never say you lack internet access or \
tools without having tried them. If the question is missing one thing you need, \
such as a location, ask for just that.";

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
