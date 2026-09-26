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
ama setup
```

`ama setup` creates the `@@` alias, writes a starter config, and wires up your
`.bashrc` or `.zshrc`. Open a new shell; `ama doctor` tells you what it found.
Run it again any time — it is idempotent. `ama setup --dry-run` shows the plan
without touching anything.

From a clone instead: `./install.sh`.

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

`command` is a plain argv list. `ama` inserts the one-shot token an agent needs,
so you do not have to remember it:

| You write | It runs | Why |
|---|---|---|
| `[claude]` | `claude -p` | `-p` prints and exits |
| `[codex]` | `codex exec` | one-shot is a *subcommand*; `codex exec -p` means `--profile` |
| `[ollama, llama3]` | `ollama run llama3` | |
| `[agy]` | `agy -p {prompt}` | agy's `-p` takes the prompt as its value and does not read stdin |

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

The design record lives in [`0.1.0/`](0.1.0/) — intent, risks, requirements,
decisions, and verification.

Apache-2.0.
