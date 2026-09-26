# ama — ask me anything

Ask your own agent a question without leaving the shell. Type `@@`, a space, and
your question.

```
user@host:~/project-x$ @@ what's this project all about?
🤖: A toy project that builds a quantum core to find ALL analytical solutions
   for the Navier-Stokes equations under any boundary conditions.
```

BYOA — bring your own agent. `ama` runs whatever CLI you already have
(`claude`, `codex`, `agy`, `ollama`, or anything that reads a prompt on stdin).

## Install

```sh
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/hongchenme/ask-me-anything/releases/latest/download/ama-installer.sh | sh
~/.local/bin/ama setup
```

The second line uses the full path because the installer has only just put
`ama` there — your current shell does not know about it yet.

`ama setup` creates the `@@` alias, writes a starter config, and wires up your
`.bashrc` or `.zshrc`. Open a new shell; `ama doctor` tells you what it found.
Re-run it any time — it is idempotent — and `ama setup --dry-run` shows the plan
without touching anything.

From a clone instead: `./install.sh`.

The `@@` trigger is bash and zsh only: it is a readline/ZLE hook, and other
shells have no equivalent. `ama ask -- <question>` works in any shell.

## Use

```
user@host:~/project-x$ @@ what's this project all about?
🤖: A toy project that builds a quantum core ...

user@host:~/project-x$ @@ show me the steps how it is designed
🤖: Let me read the repo first...

user@host:~/project-x$ clear && @@ show me the joke of the day
🤖: I have no sense of humor.
```

It is conversational: each `@@` sees the ones before it. `clear` starts fresh,
because the conversation *is* what is on your screen.

Agents take a few seconds to answer. Until the answer starts, the line it will
appear on shows a moon cycling through its phases, so you can tell thinking
from stuck:

```
user@host:~/project-x$ @@ whats weather?
🤖 🌔
```

When the answer starts it takes over that line. The moon is never sent to your
agent as context, not even when a frame stays on screen, which happens if you
press Enter, or interrupt `ama`, while the moon is showing.

It only shows on a terminal. `ama ask -- … | less` and `$(ama ask -- …)` get
exactly the answer. (Typed after `@@`, a `| less` is part of the question.)

Your question is passed through byte-for-byte — no globbing, no variable
expansion, no quote handling. The integration rewrites the line into a correctly
quoted command first, so `@@ what's this?` runs as `@@ 'what'\''s this?'`.

## Configure

`~/.ama/config.yml`:

```yaml
agent:
  command: [claude, --model, opus]

max_context_lines: 200
```

`command` is a plain argv list. `ama` inserts what an agent needs to run
one-shot *and* to answer questions about the world, so you do not have to
remember either:

| You write | It runs | Why |
|---|---|---|
| `[claude]` | `claude -p --allowedTools WebSearch` | `-p` prints and exits; the grant is explained below |
| `[codex]` | `codex --search exec` | one-shot is a *subcommand*; `codex exec -p` means `--profile`, and `--search` is rejected after `exec` |
| `[ollama, llama3]` | `ollama run llama3` | |
| `[agy]` | `agy -p {prompt}` | agy's `-p` takes the prompt as its value and does not read stdin |

`ama doctor` prints the argv that will actually run.

### Web search

A one-shot agent has nobody to answer a permission prompt, so anything needing
approval is denied — and it will tell you it has no internet access rather than
that it was not allowed to look. `ama` therefore grants web search, and only
web search:

```yaml
agent:
  command: [claude]
  tools: research   # the default — let the agent search the web
  # tools: none     # grant nothing
```

Page fetching is deliberately *not* granted. Your terminal screen is part of
every prompt, so anything on it can influence the agent; a search goes to a
search engine, while fetching a URL chosen by text on your screen is a way for
that text to send data somewhere. Reading files is unaffected either way —
agents already allow that without asking.

Anything else runs exactly as written. Add `adapter: false` to stop `ama`
touching your argv at all:

```yaml
agent:
  command: [my-bot, --ask, "{prompt}"]
  adapter: false
```

`{prompt}` puts the question in that argument instead of on stdin.

### System prompts

Pass them through `command` — tested against each agent:

```yaml
# claude: --system-prompt replaces, --append-system-prompt adds to the default
agent:
  command: [claude, --append-system-prompt, "Answer in one sentence."]

# codex has no --system-prompt; use a config override
agent:
  command: [codex, -c, 'instructions="Answer in one sentence."']
```

`agy` has no system-prompt flag.

### The moon

To turn off the moon shown while the agent thinks:

```yaml
spinner: false
```

While the moon is showing, anything your agent prints to stderr is passed
through `ama`, so a progress line never lands on the moon's line — each keeps
its own line, and the moon carries on below it. `spinner: false` undoes that
as well: the agent writes to your terminal directly, exactly as before.

## What it sends

Inside tmux or screen, `ama` sends what is on the visible pane — including other
commands' output, which is what lets it answer *"why did that build fail?"*.

- Your **first** `@@` in a pane sends the whole visible screen, so the command
  you are asking about is included.
- After that, each `@@` sends from the **first earlier `@@`** down — which is why
  `clear` really does start a new conversation.

Outside a multiplexer it sends only its own previous questions and answers.

It does not filter any of it. **If a secret is on your screen, it goes to your
agent.** Clear the screen first, or ask without context:

```
@@ --no-context what is this
```

`ama` never reads or forwards your agent's credentials.

## Develop

```sh
./check.sh   # fmt, clippy, shell syntax, and all tests
```

The design record lives in one directory per version —
[`0.1.0/`](0.1.0/), [`0.1.1/`](0.1.1/), [`0.1.2/`](0.1.2/) — intent, risks,
requirements, decisions, and verification.

Apache-2.0.
