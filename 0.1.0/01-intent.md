---
software_version: 0.1.0
process_version: 1.0.0
stage: S0-intent
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R1
inputs: [README.md]
updated: 2026-09-26
---

# INT — Capture intent

## INT-01 Problem

Answering a small question mid-task costs a context switch. The developer leaves the
shell, starts an agent CLI, waits for it to boot, re-establishes what directory and
project they are in, asks, reads, and quits. The question took five seconds to think
of and a minute to ask. The cost is not the tokens; it is the interruption, so the
question often goes unasked.

## INT-02 Why it matters

The shell is where the work already is. A question asked *in* the shell inherits the
working directory, the command that just failed, and the output still on screen. An
agent invoked from a separate CLI starts blind and must be re-told all of it.

## INT-03 Target users

The repository owner first: a developer working in bash on Linux, frequently inside a
terminal multiplexer, who already has at least one agent CLI installed and configured.
Secondarily, developers who have their own preferred agent and will not adopt a tool
that hardwires someone else's.

## INT-04 Proposed outcome

A prompt prefix, `@@`, that turns any shell line into a question. The configured agent
answers inline, below the prompt, with the visible terminal view as context. Follow-up
questions continue the conversation. Clearing the screen clears the conversation.

## INT-05 Constraints

- **C-01** Written in Rust.
- **C-02** Trigger is `@@` followed by a space, at the start of the line.
- **C-03** The agent is user-supplied and configured in `~/.ama/config.yml`. No agent
  is hardwired and no API key is ever handled by this tool.
- **C-04** Conversation context is bounded by the current terminal view, resetting when
  the screen is cleared.
- **C-05** Answers are prefixed `🤖: `.
- **C-06** Perceived latency is a product feature, not an optimization. The README
  advises low effort settings for speed; the tool must not add meaningful overhead of
  its own.

## INT-06 Non-goals for 0.1.0

- Shells other than bash and zsh.
- Windows.
- A resident daemon or pre-warmed agent process.
- Agent-native session resume (`claude --resume`) or provider-specific conversation state.
- Parsing structured agent output (`stream-json`); stdout text is the contract.
- Filtering, redacting, or classifying terminal content before it reaches the agent.
  Explicitly cut from this cycle — see RISK-01.

## INT-07 Primary success measure

A question that the owner would previously not have bothered to ask gets asked and
answered without leaving the shell, and the answer is good because the agent could see
the terminal. Operationally: `@@` overhead before the agent process starts stays under
50 ms, and normal shell usage is indistinguishable from an uninstrumented shell.

## INT-08 Known risks at intake

- The trigger must survive the shell's own parser. Shell metacharacters in a natural
  language question are the norm, not the exception.
- Reading the terminal view is not portable.
- Re-injecting user text into the shell for execution is a correctness-critical
  operation; a flawed escape turns a typed question into executed code.

## INT-09 Open questions at intake

All four were resolved before SG0 and are recorded as decisions in
[04-design.md](04-design.md): capture mechanism (ADR-001), context source (ADR-002),
agent invocation contract (ADR-003), and canonical naming (ADR-005).

## SG0 — Intent accepted

Accepted 2026-09-26 by solo-founder. Acceptance authorizes discovery, not a solution.
