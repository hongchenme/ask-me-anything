---
software_version: 0.1.1
process_version: 1.0.0
stage: S6-release
status: draft
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [07-verification.md]
updated: 2026-09-26
---

# Release record — 0.1.1

**Prepared, not authorized.** SG5 is still pending, and SG6 cannot open
before it. An agent cannot grant its own production approval (SDLC §5,
§11), so everything below is a plan the release authority executes or
rejects — nothing here has been published.

## Release candidate

| | |
|---|---|
| Version | 0.1.1 (`crates/ama/Cargo.toml`, `Cargo.lock`) |
| Branch | `cycle/0.1.1-fixes` |
| Change | 11 files, +632 / −56 |
| Artifacts | `cargo-dist` 0.33.0 → `ama-installer.sh` + tarballs |
| Targets | `{aarch64,x86_64}-apple-darwin`, `{aarch64,x86_64}-unknown-linux-gnu` |
| Trigger | push of tag `v0.1.1` runs `.github/workflows/release.yml` |

Windows is excluded, unchanged from 0.1.0: the headline feature is a
bash/zsh Enter hook.

## Release notes (draft)

> **0.1.1 — two fixes for questions that did not work**
>
> - **Your agent can search the web again.** A one-shot agent has nobody to
>   approve a permission prompt, so web search was silently denied and the
>   agent told you it had no internet access. `ama` now grants search — and
>   only search — and asks the agent to actually use it. `tools: none` opts
>   out; page fetching is deliberately not granted.
> - **Return works on every terminal.** The `@@` trigger hooked only one of
>   the two keys that mean "accept this line". On terminals that send LF for
>   Return (iTerm2 can be configured this way; macOS Terminal is not), the
>   hook was skipped entirely — a question containing an apostrophe left the
>   shell sitting at a continuation prompt with no output. Both keys are
>   hooked now.
>
> Upgrading: re-run `ama setup` (idempotent), then open a new shell so the
> updated hook loads. Existing `~/.ama/config.yml` files keep working.

## Upgrade path and compatibility

| Concern | Assessment |
|---|---|
| Config compatibility | Every 0.1.0 config loads unchanged; `tools:` defaults to `research`. Pinned by test |
| Behaviour change | `claude` and `codex` gain one flag. `ama doctor` prints the argv that will run |
| Shell rc | No rc change needed; `ama setup` remains idempotent |
| **Requires a new shell** | Yes. The hook is loaded by `eval "$(ama init bash)"` at shell start, so a shell already open keeps the old bindings — the defect-2 fix does not apply until a new shell starts |

## Rollback

| Level | Action |
|---|---|
| Whole release | Re-tag/install 0.1.0; no state migration exists, nothing to undo |
| Search grant only | `tools: none` in `~/.ama/config.yml` — restores 0.1.0 argv exactly, pinned by `tools_none_reproduces_the_0_1_0_invocation` |
| All argv handling | `adapter: false`, unchanged from 0.1.0 |
| Shell hook only | Remove the `eval` line from the rc; `ama ask -- <question>` still works |

No database, no migration, no server-side state, no feature flag. Rollback
is reinstall-or-reconfigure, which is why this is R2 rather than R3.

## Pre-authorization checklist

| Item | State |
|---|---|
| SG5 accepted by owner | ☐ pending |
| `./check.sh` green on the branch | ☑ |
| CI green on the branch | ☐ not yet pushed |
| Both defects verified against a live agent | ☑ (07 §1) |
| Rollback path tested | ☑ `tools: none` pinned by test |
| Release notes reviewed | ☐ pending |
| Tag `v0.1.1` pushed | ☐ owner action |

## SG6 — Release authorized

**Not reached.** Blocked on SG5.
