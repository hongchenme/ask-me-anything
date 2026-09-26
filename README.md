# ama -- ask me anything

ama is a tool to let you BYOA -- bring your own agent -- to fastly get answer or task completed in terminal inline, without switching to an agent CLI. To use it at a terminal, start with `@@`+space+your prompt

## install

    git clone <this repo> && cd question-mark-x2 && ./install.sh

Then open a new shell. `ama doctor` reports what it found.

## example use cases

1. just ask
```
user@host:~/project-x$ @@ what's this project all about?
🤖: This is a toy project that builds a quantum core to find ALL analytical solutions for Navier-Stokes Equation under any given boundary conditions!
```

2. make it conversational
```
user@host:~/project-x$ @@ what's this project all about?
🤖: This is a toy project that builds a quantum core to find ALL analytical solutions for Navier-Stokes Equation under any given boundary conditions!
user@host:~/project-x$ @@ show me the steps how it is designed
🤖: Let me read the repo first...
```

3. clear a conversation to start fresh. Context is feed up to the first `@@` of the current terminal view.
```
user@host:~/project-x$ @@ what's this project all about?
🤖: This is a toy project that builds a quantum core to find ALL analytical solutions for Navier-Stokes Equation under any given boundary conditions!
user@host:~/project-x$ @@ show me the steps how it is designed
🤖: Let me read the repo first...
user@host:~/project-x$ clear && @@ show me the joke of the day
🤖: I have no sense of humor.
```

> The shell integration rewrites your line into a correctly quoted command before
> running it, so `@@ what's this project all about?` executes as
> `@@ 'what'\''s this project all about?'`. Your question is passed through
> byte-for-byte: no globbing, no variable expansion, no quote handling.

4. configure your agent, model, effort (prefer small effort for fast response) in `~/.qmx2/config.yml`
```
agent: claude --model opus --effort high
```

`ama` adds the one-shot flag a known agent needs, so the line above runs as
`claude -p --model opus --effort high`. For full control, give an explicit command:

    agent:
      command: [my-bot, --ask, "{prompt}"]

    max_context_lines: 200

## what it sends

Inside tmux or screen, `ama` sends your agent what is on the visible pane --
including the output of other commands, which is what lets it answer "why did that
build fail?".

- Your **first** `@@` in a pane sends the whole visible screen, so the command you
  are asking about is in there.
- Once a conversation is on screen, each later `@@` sends from the **first earlier
  `@@`** down, so `clear` really does start a fresh conversation.

Outside a multiplexer it sends only its own previous questions and answers.

It does not filter that content. If a secret is visible on your screen, it goes to
your agent. Clear the screen first, or ask without any context:

    @@ --no-context what is this
    ama ask --no-context -- what is this

`ama` never reads or forwards your agent's credentials.
