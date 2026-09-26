---
software_version: 0.1.2
process_version: 1.0.0
stage: S6-release
status: accepted
owner: solo-founder
approver: solo-founder
risk_tier: R2
inputs: [07-verification.md]
updated: 2026-09-26
---

# Release record — 0.1.2

**Authorized by the owner, 2026-09-26.** The release authority directed:

> when the goal is achieved, push to remote version branch, create a PR to
> merge to main and approve the PR via code owner overwrite. Then git push a
> new tag 0.1.2 to trigger the release

That is the SG6 go decision. An agent cannot grant its own release approval
(SDLC §5, §11), and none is claimed here. The execution plan is below. The
steps after this record's commit (CI, merge, tag, release run) leave their
evidence on GitHub, not in this file.

**Tag name.** The owner wrote "0.1.2". The repository's tags are `v0.1.0`
and `v0.1.1`, and so are its release names, so the tag is **`v0.1.2`**.
The release workflow triggers on `**[0-9]+.[0-9]+.[0-9]+*`, which matches
either.

## Release candidate

| | |
|---|---|
| Version | 0.1.2 (`crates/ama/Cargo.toml`, `Cargo.lock`) |
| Branch | `cycle/0.1.2-moon-spinner`, on `9f81536` (the tip of `main`) |
| Change | tracked files +952/−57, 5 new files (526 lines), `0.1.2/` record |
| Artifacts | `cargo-dist` 0.33.0: `ama-installer.sh` + tarballs |
| Targets | `{aarch64,x86_64}-apple-darwin`, `{aarch64,x86_64}-unknown-linux-gnu` |
| Trigger | pushing annotated tag `v0.1.2` runs `.github/workflows/release.yml` |

## Execution plan

1. Commit on `cycle/0.1.2-moon-spinner` and push it.
2. Open a pull request to `main`.
3. Wait for the required `check`. This cycle was built on macOS, so this is
   the first Linux run of the new tests. A failure stops the release and
   goes back to S4.
4. Merge with a merge commit, using the admin override of the code-owner
   review rule (`gh pr merge --merge --admin`). The owner is the only code
   owner and cannot approve their own PR (see `.github/CODEOWNERS`), which
   is how #1 and #2 were merged.
5. Tag the merge commit `v0.1.2` (annotated, `ama 0.1.2 — …`, like
   `v0.1.1`), push the tag, and watch the release run to completion.

## Release notes (draft)

> **0.1.2 — see that your agent is thinking**
>
> Agents take seconds to answer. Up to now `@@` showed nothing at all in
> that time, and a slow answer looked exactly like a hung command. Now the
> line the answer will appear on shows `🤖` and a moon cycling through its
> phases, 🌑🌒🌓🌔🌕🌖🌗🌘, until the answer takes its place.
>
> - The moon shows only on a terminal. Piped or captured output
>   (`ama ask -- … | less`, `$(ama ask -- …)`) is exactly as before.
> - It never ends up in what `ama` sends your agent, even if you press Enter
>   or Ctrl-C while it is showing and a frame stays on screen.
> - While it shows, anything your agent prints to stderr (codex prints a
>   lot) is passed through `ama` so that it keeps its own lines.
> - `spinner: false` in `~/.ama/config.yml` turns the moon off and gives your
>   agent the terminal directly, as before.
>
> Upgrading: nothing to do. The moon lives in the binary, not the shell
> hook, so there is no need to re-run `ama setup` or open a new shell.

The last line holds because `ama.bash` and `ama.zsh` are unchanged: the
upgrade takes effect on the next `@@`.

## Upgrade path and compatibility

| Concern | Assessment |
|---|---|
| Config | Every 0.1.1 config loads unchanged and gets the moon. Pinned by test |
| Output to pipes and files | Byte-identical to 0.1.1. Pinned by test |
| Shell rc and hook | Unchanged. No new shell needed |
| Agent stderr | Routed through `ama` only while the moon shows on a terminal; otherwise inherited as before |

## Rollback

| Level | Action |
|---|---|
| The moon only | `spinner: false`. Restores 0.1.1's output and stderr handling exactly, pinned by two e2e tests |
| Whole release | Reinstall 0.1.1. **First delete any `spinner:` line**: 0.1.1 rejects keys it does not know and would refuse every turn with exit 2 |

There is no database, migration, server state or feature flag.

## Pre-authorization checklist

| Item | State |
|---|---|
| Release directed by owner (SG5/SG6) | ☑ 2026-09-26, quoted above |
| Owner confirms Q-03, Q-04 and ADR-014 ([07 §7](07-verification.md#7-release-recommendation)) | ☐ open; each is reversible |
| `./check.sh` green | ☑ on this machine (macOS), 193 tests |
| Independent review, and re-review of its fixes | ☑ no blocking issues (07 §5, §8) |
| Behaviour verified against live agents | ☑ claude and codex (07 §1) |
| Rollback path tested | ☑ `spinner: false`, pinned by test |
| CI green on Linux | ☐ runs on the pull request, and gates step 4 |
| Merged, tagged `v0.1.2`, release published | ☐ steps 4–5; evidence on GitHub |

## SG6 — Release authorized

**Authorized 2026-09-26** by the owner's directive, quoted above. It is
executed according to the plan, and step 3's CI result can still stop it.
