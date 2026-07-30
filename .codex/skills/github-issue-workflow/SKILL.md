---
name: github-issue-workflow
description: Implement or fix one or more GitHub issues in the Faraweave repository using dependency-gated worktrees, isolated role agents, independent review, sequential QA, and verified PR merge and issue closure. Use when the user asks Codex to implement, fix, resolve, complete, or work through GitHub issue numbers or URLs and expects tested, reviewed, merged results.
---

# GitHub Issue Workflow

Own the complete lifecycle through verified merge and issue closure unless user input, permissions, or an external failure makes that impossible.

## Load only the current role

Keep this file as the controller contract. Read a role reference only when performing that role; do not load every reference speculatively.

- Issue lead, implementer, or fix agent: [references/implementer.md](references/implementer.md)
- Independent reviewer: [references/reviewer.md](references/reviewer.md)
- Queue-head QA agent: [references/qa.md](references/qa.md)

Do not reread this skill or a role reference after it is loaded unless the file changed or a compaction omitted a required rule.

## Preserve these invariants

1. Preserve unrelated primary-checkout changes. Perform issue work only in issue-specific worktrees.
2. Give each issue one branch, worktree, and agent tree. Never let another issue's agent edit that worktree.
3. Keep implementation, review, fixes, and QA attributable to the same issue and exact commit.
4. Continue through push, required checks, merge, and verified issue closure.

## Establish scope before spawning

Read every requested issue, linked issue or PR, current repository state, and the specifications or decisions needed to classify dependencies. Convert each issue into explicit acceptance criteria and expected focused/full validation.

For multiple issues, build the dependency graph before any implementation agent starts:

1. Add `A -> B` when B requires A's behavior, API, data format, decision, generated output, or merged files.
2. Treat file overlap as integration risk rather than an automatic hard dependency.
3. Identify issues that remove or supersede a subsystem and prioritize them when they simplify downstream work.
4. Resolve cycles by redefining boundaries or ask the user when the correct boundary is not evident.
5. Produce topological implementation waves, projected merge order, and a deterministic ready queue.

The graph and issue packets are a hard spawn gate. Do not start an issue lead before they exist. Ask the user only when ambiguity materially changes behavior, compatibility, data integrity, public APIs, or issue boundaries.

Use the full workflow for code, runtime behavior, bugs, performance, dependencies, or uncertain scope. Use the lightweight path for clearly trivial documentation, comments, spelling, or formatting; it still requires a dedicated worktree, independent review, queue position, and focused validation.

## Control slots centrally

The controller owns one global slot ledger. Assume four concurrent slots including the controller unless the active tool reports another limit.

Use this default allocation:

- one controller;
- up to two issue leads;
- one reserved reviewer, QA, fix, or diagnostic lane.

Track agent path, issue, role, state, and slot grant locally. Use `list_agents` only when the ledger is uncertain after a completion, steering event, or spawn failure; do not poll it after every wait.

An issue lead may spawn a child only after the controller grants the reserved lane. The lead spawns the child so the reviewer, fix, and QA threads remain inside the issue tree. Attempt the spawn once. If the thread limit is reached, refresh the ledger once, wait for a slot, and retry only after a new grant.

Release a slot only after the agent reaches a terminal state. Never probe capacity with speculative spawns.

## Isolate context and choose the role model

Never assign an unrelated issue or role to a completed agent. Use `followup_task` only for the same issue and continuing role, such as a requested clarification or a fix round. Start a fresh agent for another issue.

Spawn with `fork_turns: "none"` by default. Use a small recent-turn fork only when an immediate steering exchange cannot be represented in the task packet. Never use `fork_turns: "all"`.

Every task packet must contain only:

- issue number, URL, role, and acceptance criteria;
- exact worktree, branch, base commit, and reviewed tip when applicable;
- dependency and queue state;
- affected areas or changed files;
- prior findings required for a fix or re-review;
- validation tier and host limitations;
- the compact handoff contract below.

Use these defaults when the environment permits model selection; an explicit user, workspace, or admin model choice wins.

| Role | Model | Reasoning |
| --- | --- | --- |
| Dependency mapping, queue watching, log triage | `gpt-5.6-terra` | `high` |
| Issue lead, implementer, fix agent | `gpt-5.6-sol` | `high` |
| Independent reviewer | `gpt-5.6-sol` | `high` |
| Deterministic QA execution | `gpt-5.6-terra` | `high` |

Escalate a QA failure to a fresh issue-scoped diagnostic agent using `gpt-5.6-sol` with `high` reasoning only when failure classification needs deeper analysis. Model overrides require a minimal-context fork.

## Create isolated issue trees

For each ready issue:

1. Fetch `origin` without modifying the primary checkout.
2. Derive a short lowercase hyphenated name.
3. Use branch `GH-<issue-id>-<brief-name>`.
4. Use worktree `{worktree-root-folder}\Faraweave-GH-<issue-id>-<brief-name>`.
5. Create it from current `origin/main`; inspect and safely resume an existing branch or worktree instead of duplicating it.
6. Create one issue lead with the implementer role packet. The lead implements directly unless a separate implementation child adds clear value and a slot is granted.
7. Require every descendant to run Git and file commands against the exact issue worktree.

## Assign validation once

Validation evidence belongs to an exact commit and becomes stale when that commit or effective diff changes.

- The implementer owns baseline and focused regression evidence. It runs one debug suite only when focused tests cannot prove a cross-module change.
- The reviewer owns independent diff and risk analysis plus the narrow tests needed to validate findings. It does not repeat the complete local matrix.
- The queue-head QA agent is the sole owner of the complete local debug, release, contract, and host-package matrix.
- GitHub CI owns the required cross-platform matrix.

Do not rerun a passed gate on the same commit merely to recreate evidence. Reuse the recorded command, exit status, environment, and log path. Run a gate again only when the commit changed, an integration merge changed the effective diff, the previous environment was invalid, or a concrete failure requires reproduction.

After a fix, require focused implementer evidence, a fresh independent review of the complete updated diff, and then one new QA matrix.

## Batch work and keep evidence compact

Before validation, write the ordered role gate list once. Prefer an existing repository validation entrypoint or role-gate wrapper over separate ad hoc commands. Keep each long-running command attached to its yielded process and resume that process instead of issuing `Get-Process`, `git status`, or duplicate command polls.

When no wrapper covers the role, run required gates in order and batch only independent read-only discovery calls. Do not concatenate unrelated shell commands merely to reduce tool calls.

Store complete command output in an untracked temporary log directory outside the issue worktree. Return concise evidence to the controller and read full logs only for a failure, review dispute, PR description, or final audit.

Use this handoff shape and keep it under 800 tokens:

```text
issue/role:
base/tip:
status:
changed files:
gates: command -> exit
findings or fixes:
residual risks/host exclusions:
evidence log:
```

Put detailed acceptance matrices and long logs at the evidence path; do not paste them into inter-agent messages. Send one consolidated instruction or handoff per state transition rather than several incremental messages.

## Wait on state, not a timer loop

Use `wait_agent` with a 60-second timeout. It returns early on an update, so do not shorten the interval preemptively. After an unchanged wait, wait again without calling `list_agents` or narrating an unchanged snapshot.

For a yielded process, use its wait mechanism with a timeout up to 60 seconds. Inspect the process table only when the wait handle is unavailable, the process appears stalled, or failure classification requires it.

Poll GitHub checks no more than once per minute. Prefer a bounded watch operation that returns on state change. Report state changes immediately and provide at most one user-visible progress update per minute while state is unchanged.

## Maintain the sequential queue

After implementation and review pass, mark the issue ready. Do not create its PR until it reaches the queue head.

For the queue head:

1. Fetch current `origin/main`, including every earlier queue merge.
2. Update the issue branch in its worktree and rerun focused checks.
3. Obtain another review when conflict resolution or integration changes the effective diff.
4. Grant the reserved lane to a fresh QA agent inside the issue tree.
5. Do not begin the next issue's QA until the current issue merges and closure is verified.

If an earlier merge invalidates a waiting issue's assumptions or acceptance criteria, return it to implementation and review. After each merge, start newly unblocked implementation work while preserving the reserved child lane.

## Publish, watch, and merge

After QA passes:

1. Confirm the issue branch is clean and the recorded QA tip is current.
2. Push it and create a ready PR targeting `main`.
3. Include `Closes #<issue-id>`, acceptance criteria, implementation summary, compact review/QA evidence, compatibility notes, decisions, performance results when relevant, and residual risks.
4. Require successful Ubuntu `linux-x64`, Windows `windows-x64`, macOS `macos-arm64`, and unconditional `PR Gate` checks unless the current workflow names differ.
5. On a branch failure, fix in the issue worktree, repeat focused validation, fresh review, QA, push, and check watching.
6. On confirmed infrastructure or flaky failure, use the supported rerun path.
7. Never weaken tests, checks, or branch protection.
8. If GitHub reports the branch out of date, update it and repeat review/QA when the effective diff changes.
9. Use the required merge method, verify the PR merged into `main`, and verify the issue closed.

After merge, remove only clean, confirmed-merged local issue worktrees and branches. Preserve remote branches according to repository policy.

## Report

Report dependency waves, active issue trees, queue position, actionable review/QA findings, PR/check state, and genuine blockers. At completion, list each issue's PR, merge commit, closure state, validation tier, and intentionally retained resources.
