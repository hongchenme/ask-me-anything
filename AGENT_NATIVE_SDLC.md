---
document: agent-native-sdlc
process_version: 1.0.0
status: approved
owner: solo-founder
created: 2026-09-19
last_updated: 2026-09-19
approved: 2026-09-19
---

# Agent-Native Software Development Lifecycle

## 1. Purpose

This document defines the repository's versioned software development lifecycle. It is optimized for a solo founder who delegates bounded work to coding agents while remaining accountable for product, legal, security, architecture, and release decisions.

The process treats version-controlled artifacts as the interface between people and agents. Every stage reads approved inputs, produces inspectable outputs, records evidence, and ends at an explicit gate. Agents may accelerate research, design, implementation, testing, review, and operations; they may not approve their own work or silently resolve decisions that require human judgment.

## 2. Process versioning

The SDLC uses Semantic Versioning independently of the software product version:

- **Major:** Changes stage structure, approval authority, mandatory controls, or evidence required for release.
- **Minor:** Adds a required artifact, role, gate check, or backward-compatible control.
- **Patch:** Clarifies wording, templates, examples, or non-semantic guidance.

Every software cycle records the exact `process_version` it follows. An active cycle may adopt a newer compatible process version by recording the change and its impact in the cycle index. A released cycle remains linked to the process version under which it was approved.

### Process changelog

| Version | Date | Status | Change |
|---|---|---|---|
| 1.0.0 | 2026-09-19 | Approved | Initial agent-native lifecycle for a solo founder, including risk discovery, artifact gates, independent verification, legal review, and operational feedback. |

## 3. Source and adaptation

This lifecycle adapts Anthropic's [AI-native SDLC playbook](https://academy.claude.com/courses/ai-native-sdlc-playbook). The playbook's central ideas are retained:

- Plan, design, build, test, deploy, and maintain form a loop rather than a one-way handoff.
- Every stage ends with a committed artifact that the next stage can read.
- Intent, specification, plan, code, tests, review findings, release evidence, and incident records form an audit chain.
- Repository instructions and reusable skills hold institutional knowledge.
- Agents receive fast feedback through tests, builds, screenshots, and continuous evaluations.
- Independent review and deterministic gates keep human accountability where judgment or material risk exists.
- Production evidence feeds new intent back into the next cycle.

This repository adds two explicit concerns that the product requires:

1. **Discovery and risk precede implementation planning.** Privacy, employment, payments, public content, document processing, and machine access require threat, data, legal, and abuse analysis before build authorization.
2. **Product release gates are independent of feature flags.** A flag may control rollout inside an open gate; it cannot open a gate or waive evidence.

The process is tool-neutral. `AGENTS.md`, `CLAUDE.md`, skills, hooks, CI rules, or equivalent agent controls may implement it, but this document is the source of truth for lifecycle semantics.

## 4. Core principles

1. **Intent before output.** State the problem, affected users, desired outcome, constraints, success measure, and exclusions before selecting implementation details.
2. **Evidence before claims.** A test, command output, review record, measurement, or approved artifact supports every completion claim.
3. **Risk determines autonomy.** The more irreversible, public, regulated, security-sensitive, or financially material an action is, the more independent evidence and human approval it requires.
4. **Artifacts carry context.** Decisions must survive outside a chat transcript. Approved context lives in the repository.
5. **Agents do not self-approve.** The agent that creates a material output cannot be its final approver.
6. **Deterministic controls back advisory guidance.** Instructions and skills explain desired behavior; tests, policies, hooks, branch protection, schemas, and runtime checks enforce non-negotiable behavior.
7. **Small, reversible increments.** Work is partitioned into reviewable vertical slices with rollback or disablement paths.
8. **One policy path.** Web, native, APIs, MCP, workers, and operator tools use the same domain rules for authorization, pricing, lifecycle, confirmation, and audit.
9. **No hidden scope changes.** An agent records deviations and obtains approval rather than silently dropping or adding functionality.
10. **Legal analysis is qualified.** Agents may research and structure legal risks; qualified counsel or the accountable owner decides the launch position.
11. **Production closes the loop.** Incidents, support patterns, abuse, model failures, and metric breaches create new versioned intent and regression evidence.

## 5. Roles and accountability

One person may hold several roles, but the responsibilities remain distinct.

| Role | Accountabilities | Cannot delegate to an agent |
|---|---|---|
| Founder / product owner | Intent, scope, success metrics, pricing, decision status, product acceptance | Final product judgment and scope approval |
| Architecture authority | System boundaries, data ownership, trust boundaries, technical exceptions | Acceptance of material architecture risk |
| Security and privacy owner | Threat model, data classification, controls, incident readiness | Risk acceptance |
| Legal counsel or qualified reviewer | Jurisdiction-specific legal conclusions and launch advice | Legal sign-off where required |
| Implementer agent | Work within an approved packet, tests, documentation, recorded deviations | Approval of its own output |
| Reviewer agent or human | Independent review against intent, specification, plan, security, and policy | Product-owner or release-authority decision |
| Verifier | Reproduce acceptance evidence without changing the implementation under review | Waiver of failed evidence |
| Release authority | Go/no-go, production change authorization, rollback decision | Production approval |
| CI/CD system | Deterministic tests, policy checks, artifact creation, deployment mechanics | Judgment that is not encoded as a deterministic rule |

For this repository, the solo founder initially owns the founder, architecture, privacy, and release roles. That concentration of responsibility does not remove independent review or qualified legal advice where a gate requires it.

## 6. Risk tiers

Each intent and work packet declares a risk tier. The highest applicable tier controls.

| Tier | Typical work | Minimum review and evidence |
|---|---|---|
| R0 — Documentation | Copy edits, non-semantic documentation | Author review; link and formatting checks |
| R1 — Reversible internal | Tooling or internal changes with no production data | Automated checks and owner merge approval |
| R2 — User-facing | Reversible product behavior, public UI, ordinary data changes | Independent review, acceptance tests, accessibility checks, rollout and rollback plan |
| R3 — Critical | Identity, payments, private documents, public indexing, moderation, deletion, MCP authority, legal controls, migrations, production access | Threat and privacy review, independent adversarial review, failure and recovery tests, explicit owner approval, and qualified policy/legal review where applicable |

An agent may recommend a tier increase. Only the accountable owner may accept a lower tier, and the rationale must be recorded.

## 7. Artifact contract

Every cycle lives in a directory named by the target software version, such as `0.1.0/`. The directory contains the execution record, findings, decisions, and evidence for that version.

Every cycle document begins with or clearly records:

```yaml
software_version: 0.1.0
process_version: 1.0.0
stage: S0-intent
status: draft
owner: solo-founder
approver: unassigned
risk_tier: R3
inputs: []
updated: 2026-09-19
```

Allowed artifact states are `draft`, `in-review`, `accepted`, `blocked`, `superseded`, and `complete`. A change from `accepted` back to editable work records why reapproval is required.

### Required cycle record

File names may differ when the cycle index maps them unambiguously, but each cycle must contain these artifact classes:

| Artifact class | Recommended file | Purpose |
|---|---|---|
| Cycle index | `README.md` | Current stage, gates, owners, artifact map, blockers, and next action |
| Intent | `01-intent.md` | Problem, outcome, users, constraints, success, exclusions, open questions |
| Discovery and risk | `02-discovery-and-risk.md` | Research, alternatives, assumptions, legal/security/data/abuse risks |
| Requirements | `03-product-requirements.md` | Functional and non-functional requirements with acceptance criteria |
| Design | `04-design.md` plus diagrams/ADRs | UX flows, domain model, architecture, trust boundaries, lifecycle, decisions |
| Implementation plan | `05-implementation-plan.md` | Vertical slices, exact boundaries, tests, migrations, rollout, rollback |
| Build record | `06-build-record.md` | Implemented slices, commits, deviations, generated artifacts, local evidence |
| Verification | `07-verification.md` | Test matrix, results, reviews, evals, accessibility, security, privacy, performance |
| Release record | `08-release.md` | Release candidate, approvals, migrations, configuration, flags, deployment, rollback |
| Operations record | `09-operations.md` | SLOs, telemetry, incidents, feedback, post-release decision, cycle closure |

Supporting material belongs in the same version directory, optionally under `decisions/`, `evidence/`, `research/`, or `runbooks/`. Secrets, raw production personal data, private-link tokens, and unredacted user documents never belong in lifecycle artifacts.

## 8. Lifecycle overview

| Stage | Name | Primary output | Stage gate |
|---|---|---|---|
| S0 | Capture intent | Accepted intent | SG0 — Intent accepted |
| S1 | Discover and classify risk | Evidence-backed discovery and risk record | SG1 — Discovery accepted |
| S2 | Specify requirements and design | Accepted product and technical specification | SG2 — Specification accepted |
| S3 | Plan and partition | Approved implementation plan and work packets | SG3 — Plan approved |
| S4 | Build and integrate | Reviewed implementation candidate | SG4 — Build complete |
| S5 | Verify and evaluate | Reproducible verification evidence | SG5 — Verification accepted |
| S6 | Release and deploy | Authorized, observable release | SG6 — Release authorized |
| S7 | Operate and learn | Operational review or new intent | SG7 — Cycle closed or loop restarted |

Stages may iterate, but gates cannot be skipped. Work discovered late returns to the earliest affected stage. Product-specific gates, such as v0.1.0 G0–G6, are additional constraints and do not replace these lifecycle gates.

## 9. Stage procedures

### S0 — Capture intent

**Entry:** A founder idea, user problem, audit finding, support issue, incident, metric breach, or legal/policy change.

**Agent work:** Ask focused questions; restate the intended outcome; separate stated facts from assumptions; identify affected users and systems; capture exclusions and uncertainty.

**Required output:** An intent artifact containing the problem, why it matters, target users, proposed outcome, constraints, non-goals, primary success measure, known risks, and open questions.

**SG0 exit:** The founder accepts, rejects, or returns the intent. Acceptance means the problem is worth discovery; it does not approve a solution.

### S1 — Discover and classify risk

**Entry:** Accepted intent.

**Agent work:** Inspect the repository and current behavior; gather user, market, data, security, privacy, legal, accessibility, operational, and commercial context; compare credible approaches; build the assumption and risk registers; classify the work R0–R3.

For legal research, record jurisdiction, source, publication date, retrieved date, exact question, and whether the conclusion is law, regulator guidance, contract/platform policy, interpretation, or product choice. Prefer primary sources. Mark counsel decisions explicitly.

**Required output:** Discovery record, alternatives and trade-offs, risk register, data classification, initial threat scenarios, legal questions, and proposed success/guardrail metrics.

**SG1 exit:** The founder accepts the problem framing, approach direction, risk tier, and named decision owners. Every unresolved issue has an identifier, owner, and blocking effect.

### S2 — Specify requirements and design

**Entry:** Accepted discovery or an explicitly recorded reason for overlapping discovery and design.

**Agent work:** Define user journeys, functional and non-functional requirements, domain terms, states, error paths, accessibility behavior, data model, trust boundaries, APIs, lifecycle, observability, abuse controls, pricing rules, and rollout constraints. Produce alternatives before locking material architecture choices.

**Required output:** Requirements with stable identifiers and acceptance criteria; UX flows; architecture context and diagram; decision records; legal and security control mapping; data lifecycle; release-gate mapping.

**SG2 exit:** The founder approves the written specification. R3 areas also have the required security, privacy, legal, or platform-policy disposition. Approval establishes the target, not permission to code past unresolved blockers.

### S3 — Plan and partition

**Entry:** Accepted specification and all planning-blocking decisions closed.

**Agent work:** Read the actual repository; map exact files and interfaces; split work into vertical slices small enough for independent review; name dependencies; define failing tests first; specify migrations, fixtures, feature flags, telemetry, rollout, rollback, and documentation changes. Parallelize only tasks that do not share files, schemas, or mutable state.

Each work packet records: task ID, objective, inputs, permitted files and systems, forbidden actions, risk tier, interfaces produced and consumed, tests, evidence, rollback, reviewer, and completion criteria.

**Required output:** Implementation plan, dependency graph, work packets, requirements-to-tests traceability, and cost/time uncertainty where useful.

**SG3 exit:** The founder or architecture authority approves the plan. The plan is understandable without the planning conversation. No agent begins implementation while the relevant product release gate is closed.

### S4 — Build and integrate

**Entry:** Approved work packet and implementation authorization for its gate.

**Agent work:** Use an isolated branch or worktree when concurrent work exists; write a failing test or other reproducible check first; implement the smallest change that passes; run the local feedback loop; update documentation and generated artifacts; record deviations before continuing. Keep commits small and attributable.

Agents use least privilege. They do not expose secrets, widen permissions, perform destructive operations, contact external people, spend money, publish publicly, or change production state without the authority and confirmation defined by the work packet.

**Required output:** Code/configuration changes, tests, migration artifacts, documentation, build record, command output, and plan-deviation record.

**SG4 exit:** The implementer has fresh evidence that the slice meets its checks. An independent reviewer finds no unresolved blocking issue. Build completion is not release approval.

### S5 — Verify and evaluate

**Entry:** Integrated implementation candidate.

**Agent work:** Reproduce evidence from a clean context; verify requirements traceability; run unit, integration, contract, end-to-end, migration, rollback, accessibility, localization, security, privacy, abuse, performance, and recovery checks in proportion to risk. Compare visual work with its approved source. Test agent prompts, models, skills, hooks, or extraction behavior with versioned evals.

The verifier reports commands, environment, inputs, outputs, failures, exclusions, and residual risk. A failed check is not converted to a pass through explanation. Waivers require an owner, rationale, expiry, and follow-up.

**Required output:** Verification matrix, independent review findings and dispositions, eval results, residual-risk record, and release recommendation.

**SG5 exit:** All blocking checks pass or have an explicitly approved waiver. The founder accepts the evidence and residual risk.

### S6 — Release and deploy

**Entry:** Accepted verification and every applicable product release gate open.

**Agent work:** Build an immutable release candidate; verify configuration and database migrations; rehearse rollback; produce release notes and support material; stage rollout behind approved flags where helpful; confirm telemetry and alerts; prepare the go/no-go record.

Production credentials are short-lived and scoped. An agent may prepare or execute a pre-approved deployment step, but it cannot grant its own production approval. OpenFeature or another flag system controls exposure only after gate approval.

**Required output:** Release record, artifact identifiers, approvals, change window, flags and defaults, migration evidence, deployment output, smoke tests, rollback evidence, and known issues.

**SG6 exit:** The release authority records go/no-go. A successful deployment includes verified health and an executable rollback path.

### S7 — Operate and learn

**Entry:** Released software or a monitored beta cohort.

**Agent work:** Observe SLOs, product success and guardrail metrics, cost, abuse, support patterns, feature-flag exposure, extraction quality, and incidents. Detection is deterministic where possible. Agents may diagnose and prepare a patch, rollback, runbook execution, or new intent only within pre-approved authority.

Incidents and repeated review findings become regression tests, eval cases, repository instructions, or new intent. When a metric breaches a control band, record the evidence and route it through the normal lifecycle rather than editing production ad hoc.

**Required output:** Operations record, metric review, incident/post-incident evidence, flag cleanup, unresolved debt, and recommendation to continue, roll back, patch, or start a new cycle.

**SG7 exit:** The founder closes the cycle after the observation window or opens a new SemVer cycle from recorded intent.

## 10. Agent operating protocol

Before acting, every agent follows this order:

1. Read this SDLC standard and the active cycle index.
2. Read repository instructions and only the artifacts relevant to the assigned work packet.
3. Inspect current files, Git state, and existing patterns before proposing changes.
4. Restate scope, risk tier, permitted changes, blockers, and proof of completion.
5. Stop if an unresolved choice would materially change scope, rights, money, security, public behavior, or data handling.
6. Make the smallest in-scope change.
7. Run the defined feedback loop and capture fresh evidence.
8. Review the diff for unrelated changes, secrets, personal data, and plan drift.
9. Hand off with results, residual risks, and the exact next gate.

Chat history is working memory, not the system of record. Material decisions and durable corrections go into cycle artifacts, repository instructions, skills, tests, or decision records.

## 11. Review and separation of duties

- The implementing agent performs self-checks but cannot supply final independent approval.
- An independent reviewer checks behavior, security, privacy, specification compliance, plan compliance, and unnecessary complexity.
- The founder reviews intent and risk rather than every generated line mechanically.
- R3 changes require explicit human approval at specification, plan, verification, and release gates.
- Legal conclusions, risk acceptance, production release, destructive data changes, public publishing rules, and payment policy always retain named human accountability.
- Review findings are ranked by severity, tied to evidence, and dispositioned. Repeated findings update instructions, skills, hooks, or evals.

## 12. Continuous feedback and agent evaluations

Every implementation task has a feedback mechanism the agent can run without guessing: tests, build, linter, schema validation, browser/screenshot comparison, security scanner, migration rehearsal, or deterministic probe.

Agent configuration is treated as production behavior. Changes to repository instructions, skills, hooks, prompts, model/provider choices, extraction logic, or agent permissions run a versioned evaluation suite. Evaluation cases use synthetic or separately consented data, never silently copied production documents.

Each incident or escaped defect adds a regression test or eval when a repeatable check is possible.

## 13. Traceability

Use stable identifiers and links so an agent or reviewer can traverse:

```text
INT intent
  -> REQ requirement
    -> RISK / ADR / PD decision
      -> TASK work packet
        -> TEST / EVAL evidence
          -> REL release record
            -> METRIC / INC operational result
```

The cycle index summarizes coverage. A requirement with no owning task, test, gate, or explicit deferral is incomplete.

## 14. Change, exception, and emergency policy

- A plan deviation is recorded before or with the code that introduces it and returns to the earliest affected gate.
- An exception names the control, scope, owner, reason, compensating control, evidence, expiry, and removal task.
- A production emergency may shorten documentation but not identity, authorization, evidence, backup, rollback, or release approval controls.
- Emergency work creates a patch-version cycle unless it is a rollback with no product change.
- After stabilization, the incident record updates the risk register, tests/evals, runbook, and new intent as applicable.

## 15. Metrics

Measure the system, not agent activity for its own sake:

- Time from intent draft to acceptance and from accepted specification to verified release.
- Gate wait time and the decisions causing it.
- First-pass build and CI success.
- Plan-to-diff divergence and specification rework after implementation begins.
- Independent-review findings by severity and repeated-finding rate.
- Escaped defects, change failure rate, rollback rate, and repeat incidents.
- Eval pass rate and time from incident to permanent regression case.
- Verification cost, model/tool cost, and founder review load per accepted slice.
- Product success and guardrail metrics defined by the active cycle.

Speed is not a success metric when rework, incidents, privacy failures, or founder review load increase.

## 16. Definition of cycle completion

A software-version cycle is complete only when:

- All intended requirements are released, explicitly deferred, or rejected with owner approval.
- Applicable lifecycle and product release gates have recorded evidence and approval.
- Verification evidence is reproducible and residual risks are owned.
- Release and rollback records identify the deployed artifacts.
- Operational metrics have been observed for the defined window.
- Incidents and material findings have been converted into tests, evals, instructions, or new intent.
- The cycle index is accurate and the next action is either closure or a new SemVer cycle.

## 17. Reference links

- [Claude Academy: AI-native SDLC introduction](https://academy.claude.com/courses/ai-native-sdlc-playbook/introduction)
- [Capture intent](https://academy.claude.com/courses/ai-native-sdlc-playbook/capture-intent)
- [Requirements and design](https://academy.claude.com/courses/ai-native-sdlc-playbook/requirements-and-design)
- [Plan mode](https://academy.claude.com/courses/ai-native-sdlc-playbook/plan-mode)
- [Repository instructions as institutional knowledge](https://academy.claude.com/courses/ai-native-sdlc-playbook/claude-md)
- [Skills as institutional knowledge](https://academy.claude.com/courses/ai-native-sdlc-playbook/skills-as-institutional-knowledge)
- [Parallel sessions and subagents](https://academy.claude.com/courses/ai-native-sdlc-playbook/parallel-sessions-and-subagents)
- [Give the agent a feedback loop](https://academy.claude.com/courses/ai-native-sdlc-playbook/give-claude-a-feedback-loop)
- [Continuous evals in CI](https://academy.claude.com/courses/ai-native-sdlc-playbook/continuous-evals-in-ci)
- [AI in the PR review loop](https://academy.claude.com/courses/ai-native-sdlc-playbook/ai-in-the-pr-review-loop)
- [Hooks as approval gates](https://academy.claude.com/courses/ai-native-sdlc-playbook/hooks-as-approval-gates)
- [CI/CD integration and deployment](https://academy.claude.com/courses/ai-native-sdlc-playbook/ci-cd-integration-and-deployment)
- [Closing the loop on metrics](https://academy.claude.com/courses/ai-native-sdlc-playbook/closing-the-loop-on-metrics)
