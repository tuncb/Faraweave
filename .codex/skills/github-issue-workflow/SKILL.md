---
name: github-issue-workflow
description: Implement or fix one or more GitHub issues in the Faraweave repository using a dedicated Git worktree and agent tree per issue, dependency-aware parallel implementation and review, and a sequential QA/PR merge queue. Use when the user asks to implement, fix, resolve, complete, or work through GitHub issue numbers or issue URLs and expects the work to be tested, reviewed, pushed, merged after required GitHub checks pass, and the issues closed.
---

# GitHub Issue Workflow

Own the complete issue lifecycle. Continue through merge and verified issue closure unless user input, permissions, or an external failure makes that impossible.

## Establish scope

1. Read the repository `AGENTS.md`, the requested issues, linked issues or PRs, the current repository state, and relevant code, tests, specifications, and decision records.
2. Preserve unrelated changes in the primary checkout. Perform issue work only in issue-specific worktrees.
3. Convert each issue into explicit, verifiable acceptance criteria. Record assumptions and the focused/full validation expected.
4. Ask the user only when an ambiguity materially changes behavior, compatibility, data integrity, public APIs, or issue boundaries. Otherwise state the assumption and proceed.
5. Classify each issue:
   - Use the full workflow for code, language/runtime behavior, bugs, performance, dependencies, or uncertain scope.
   - Use the lightweight path for clearly trivial documentation, comments, spelling, or formatting. Still use a dedicated worktree, issue agent tree, independent review, sequential queue position, and focused validation.

## Plan multiple issues

For more than one issue, build a dependency graph before implementation.

1. Add an edge `A -> B` when B requires A's behavior, API, data format, decision, or merged files. Consider explicit issue links, semantic dependencies, overlapping modules, generated artifacts, and likely merge conflicts.
2. Distinguish hard dependencies from mere file overlap. Treat overlap as a scheduling risk, not automatically as a semantic dependency.
3. Detect cycles. Resolve them by redefining issue boundaries or ask the user when the correct boundary is not evident.
4. Produce:
   - topological implementation waves for issues that may proceed in parallel;
   - a projected merge order and a deterministic ready-queue policy.
5. Do not start a hard-dependent issue until all predecessors have merged. Allow independent issue trees to implement and review concurrently.
6. At each queue decision, consider only issues whose implementation and review are complete. Prioritize an issue that unlocks the most downstream work, then lower integration risk, then issue number. Never leave the QA slot idle for an unready issue when another reviewed issue can advance.

## Create isolated issue trees

For each ready issue:

1. Fetch `origin` without modifying the primary checkout.
2. Derive a short lowercase hyphenated name and use:
   - branch: `GH-<issue-id>-<brief-name>`;
   - worktree: `{worktree-root-folder}\Faraweave-GH-<issue-id>-<brief-name>`.
3. Create the branch from current `origin/main`. If the branch or worktree already exists, inspect and safely resume it instead of duplicating or overwriting it.
4. Create one issue-lead agent for that issue. Give it the issue, acceptance criteria, worktree path, branch, dependency context, repository constraints, and validation expectations. Treat the lead as that issue's implementation agent when agent slots are constrained.
5. Require every descendant in that issue's agent tree to run Git and file commands against the exact issue worktree. Do not let agents from another issue tree edit it.
6. Reserve enough concurrency for review, fix, and QA children. Have ready issue leads implement concurrently when possible. When a lead spawns a child, release or pause the lead's active slot while the child works, then resume the same lead to preserve tree ownership.

The issue lead owns the following child-agent lifecycle. Keep implementation, review, fixes, and QA within that issue's agent tree.

## Use the repository map

Give every role the issue worktree path and require it to inspect the current files rather than relying only on this summary.

| Area | Production files | Primary validation |
| --- | --- | --- |
| Parsing, syntax, spans, parameters, fan-out | `src/parser.rs`, `src/error.rs` | parser unit tests, `tests/golden_corpus.rs`, `tests/parity_contracts.rs`, `tests/cli_contracts.rs` |
| Types, values, primitives, lifting, IEEE behavior | `src/value.rs`, `src/primitive.rs`, `src/strict_float.rs` | `tests/parity_contracts.rs`, golden corpus |
| Evaluation, argument decoding, ordering | `src/evaluator.rs` | parity, CLI, and resource contracts |
| Profiles, allocation, ownership, cleanup | `src/resources.rs`, `src/evaluator.rs` | `tests/resource_contracts.rs` |
| Public Rust API | `src/lib.rs` plus the defining module | parity and resource contracts |
| CLI, paths, atomic publication | `src/main.rs`, `src/native_builder.rs` | `tests/cli_contracts.rs`, C11 journey |
| Generated C and cross-backend parity | `src/c_emitter.rs` | `tools/validation/c11_journey.py`, resource contracts |
| Windows executable metadata | `build.rs`, `src/faraweave.exe.manifest` | Release build and Windows package contract |
| Specs, traceability, CI, packaging, releases | `spec/`, `tests/source-spec-traceability.tsv`, `.github/workflows/`, `tools/validation/`, `tools/release/` | `tools/validation/contracts.py`, provenance test, host package contract |

Treat `doc/validation-ladder.md`, `doc/porting-manifest.md`, `doc/decisions/`, and `spec/` from the issue branch as authoritative when present. Resolve all references from the issue branch and never copy the primary checkout's state into it. If a baseline contract references a missing required file, report baseline breakage instead of silently skipping the contract.

## Implementer duties

Use the issue lead as implementer when this enables parallel issue work. Create an implementation child only when capacity permits and the separation adds value.

Before editing:

1. Read the issue, acceptance criteria, relevant specifications, existing decision record, production modules, tests, fixtures, traceability rows, and validation-ladder section.
2. Trace the behavior through every affected backend: Rust library/evaluator, CLI, emitted C, and native executable. State which paths are affected and which are intentionally unaffected.
3. Run the narrowest existing regression or contract test on the untouched issue branch. Record pre-existing failures rather than silently attributing them to the change.
4. Choose focused tests that prove each acceptance criterion, including failure ordering and exact diagnostic bytes or spans where applicable.

While implementing:

1. Make only issue-scoped production, test, fixture, specification, traceability, and decision-record changes.
2. Preserve the visible parse → resolution → analysis → execution separation. Prefer plain structs, enums, slices, vectors, and module functions; avoid trait-object hierarchies, hidden cloning, unnecessary generics, and abstraction without a measured benefit.
3. Return explicit `Result` for recoverable failures. Never panic, unwrap, or expect on user input, filesystem/process output, C-compiler behavior, resource refusal, allocation refusal, or publication failure.
4. Use checked sizing and `try_reserve`/`try_reserve_exact` through the existing allocation/resource seam. Preserve deterministic admission, work charging, logical release, failure precedence, and all-or-nothing publication.
5. Keep Rust safe. If `unsafe` is essential, isolate it like the existing floating-point and Windows replacement seams, document invariants, and add focused tests for the boundary.
6. Preserve exact binary64, structured error, byte-span, formatting, path, and transactional contracts. Do not change error precedence as an accidental consequence of refactoring.
7. Add a regression test that fails on the merge-base behavior and passes with the fix. Preserve stable identifiers such as `S16`, `PARG`, `TUP`, `FAN`, `SHARED`, and `ISSUE54` when extending an existing contract.
8. Update applicable specs, `doc/porting-manifest.md`, README, and fixtures when the normative contract, evidence mapping, or public behavior changes. Update `tests/source-spec-traceability.tsv` only when its requirement-to-evidence mapping changes; preserve its legacy source-authority rows, fixed contract counts, and ordering. For an ordinary regression under an existing mapped requirement, strengthen the mapped Rust/Python test without rewriting traceability. Keep expected output byte-exact.
9. For a material design, ownership, dependency, error-model, parsing, execution, test-policy, or benchmark decision, append to the issue's `doc/decisions/issue-<id>-<stable-slug>.md`. Create it from `TEMPLATE.md` if absent. Do not edit another issue's record or rewrite existing record bytes.
10. Add a dependency only when necessary. Update both `Cargo.toml` and `Cargo.lock`, and append a brief issue decision explaining alternatives and cost.

Before handoff:

1. Run `cargo fmt --all`.
2. Run focused Rust tests using `cargo test --test <suite> <test-name>` or the narrowest valid Cargo filter.
3. Run `python tools/validation/contracts.py focused` when changing specifications, traceability, workflows, documentation, packaging, release behavior, or other repository contracts.
4. Run `cargo build --workspace --all-targets --all-features --release` and `python tools/validation/c11_journey.py` when changing parsing/lowering semantics, evaluation, strict floating point, emitted C, native building, parameters, resources, or cross-backend behavior.
5. Run the complete debug suite when the change crosses modules or no focused selection proves the acceptance criteria:
   `cargo test --workspace --all-targets --all-features`.
6. Inspect `git diff --check`, the complete diff from the merge base, and `git status --short`. Keep generated `target/` and `artifacts/` output ignored and never commit it.
7. Commit the intentional implementation, tests, and documentation to the issue branch and leave the worktree clean.
8. Hand off an acceptance-criterion-to-test matrix, changed files, exact commands and exit results, environment assumptions, decisions, and residual risks.

## Reviewer duties

Create a separate reviewer agent that did not implement or fix the current diff. Give it the issue, acceptance criteria, implementer handoff, worktree, branch, and complete diff against the merge base. Require read-only review; the reviewer reports findings and does not edit.

The reviewer must:

1. Confirm the branch is based on the intended `origin/main`, the worktree is clean, commits are issue-scoped, and the complete diff contains no generated artifacts, unrelated cleanup, hidden dependency, weakened check, or accidental public API change.
2. Build an independent acceptance-criterion matrix. Trace each criterion to production code and a test that would fail at the merge base.
3. Inspect all affected layers from the repository map, not only the files changed. Check Rust evaluator, CLI, emitted C, and native parity wherever semantics can cross backends.
4. Check exact success and failure behavior: type/shape/domain/resource/argument precedence, one-based byte spans, stable categories, canonical formatting, transactional output, alias protection, cleanup, and deterministic first failure.
5. Check data-oriented design, ownership and borrowing, cloning, allocation count, `try_reserve` coverage, live-byte/work charging, release order, deep-input stack safety, and hidden performance costs.
6. Audit every new or changed `unsafe` block for narrow scope, documented invariants, platform guards, and focused tests. Reject unsafe code that replaces an adequate safe implementation.
7. Check tests for boundaries, negative paths, old-code failure, stable IDs, exact fixture bytes, host guards, and non-weakened assertions. Check traceability, specs, README, porting manifest, and issue decision record when behavior changes.
8. Verify a dependency decision exists when `Cargo.toml` or `Cargo.lock` changes. Verify an existing issue decision record remains an exact prefix with only append-only sections added.
9. Run the review tier exactly:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo test --workspace --all-targets --all-features`
   - `python tools/validation/contracts.py review`
10. When the diff can affect emitted C, evaluation semantics, arguments, resources, strict floats, native builds, or publication, run `cargo build --workspace --all-targets --all-features --release` followed by `python tools/validation/c11_journey.py`.

Report each blocking finding with severity, file and line, violated acceptance criterion or repository contract, concrete impact, reproduction or reasoning, and the validation that would prove the fix. Treat only concrete, reproducible, actionable findings as blocking. Explicitly report no actionable findings only after completing the diff audit and required commands.

If blocking findings exist, have the issue lead create a fix agent, rerun focused tests, commit the fixes, require a clean worktree, and create a fresh reviewer for the entire updated diff. Repeat until no actionable findings remain. If the same finding survives two fix/review rounds, reviewers materially disagree, or progress stalls, summarize the evidence and ask the user instead of looping.

### Lightweight path

For a trivial documentation, comment, spelling, or formatting issue, use the same implementer and independent-review separation but run only the affected contract plus formatting. Escalate immediately to the full duties when behavior, specifications, traceability, CI, packaging, or risk changes.

## Maintain the queue

After implementation and review finish, mark the issue ready for the sequential merge queue. Do not create its PR yet unless it is at the head of the queue.

Process exactly one issue through final QA, PR checks, merge, and closure at a time:

1. Fetch the latest `origin/main`, which must include every earlier queue merge.
2. Update the issue branch with latest `origin/main` in its issue worktree, resolve and commit integration conflicts there, and rerun focused checks.
3. If the integration update changes the effective diff beyond mechanical conflict resolution, obtain another independent review before QA.
4. Do not begin QA for the next issue until the current issue has merged and its issue state has been verified.

If an earlier merge invalidates a waiting issue's assumptions or acceptance criteria, return that issue to implementation/review before its queue turn.

After each merge, immediately start any newly unblocked dependency wave in parallel with QA preparation for the next already-reviewed issue. Recompute the ready queue with the same deterministic policy without violating dependency edges.

## QA duties

At the queue head, have the issue lead create a separate QA agent that did not implement or review the change. Give it the issue, acceptance criteria, implementer and reviewer handoffs, issue branch, and expected host limitations.

The QA agent must:

1. Fetch current `origin/main`. Create a uniquely named local QA branch and temporary worktree from that exact commit, then merge the issue branch into it. Record both commit IDs. Never modify, merge into, or commit on the real `main` branch.
2. Confirm the merged QA tree is clean and contains the exact reviewed commits. Inspect the complete diff, `Cargo.lock`, fixtures, specs, traceability, decision record, and absence of tracked build artifacts.
3. Record the host OS/architecture, `rustc --version` (must be 1.97.1), Python version, and external tools. Require a strict C11 compiler for C/native journeys and Windows SDK `rc.exe` for Windows Release builds; report a missing prerequisite as a limitation, never as a pass.
4. Execute every acceptance criterion through the relevant public surface. Prefer CLI or public Rust API scenarios over private-only probes. Verify exact stdout, stderr, exit code, files, diagnostics, spans, cleanup, and performance threshold when applicable.
5. Run the local equivalent of Main CI in this order:
   - `cargo fmt --all -- --check`
   - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
   - `cargo build --workspace --all-targets --all-features --release`
   - `cargo test --workspace --all-targets --all-features`
   - `cargo test --workspace --all-targets --all-features --release`
   - `python tools/validation/contracts.py full`
6. Recognize that `contracts.py full` validates workflow/spec/traceability contracts, runs the strict C11 evaluator/generated/native journey, and runs release provenance tests. Do not report those subchecks as passed unless the command executed them successfully.
7. Run the host package contract after the Release build:
   - Windows x64: `python tools/validation/contracts.py package windows-x64`
   - Linux x64: `python tools/validation/contracts.py package linux-x64`
   - macOS arm64: `python tools/validation/contracts.py package macos-arm64`
8. On Linux, confirm the C11 journey's ASan/UBSan and `/dev/full` paths ran. On Windows, confirm PE identity and long-path package checks ran. On macOS arm64, confirm native execution and archive checks ran. List other-platform checks as assigned to GitHub CI, not locally passed.
9. If a required command fails, preserve its exact output and reproduce it once. When the failure appears unrelated, run the same command in a separate clean worktree at the recorded `origin/main` commit. Classify it as regression, pre-existing baseline failure, missing host prerequisite, or flaky/infrastructure failure; never hide it or change the issue branch without evidence.
10. Report an acceptance-criterion matrix, exact commands and exit codes, host/tool versions, artifacts inspected, platform exclusions, reproducible failures, and final pass/fail. QA passes only when all criteria and all applicable required commands pass.
11. Remove the temporary QA worktree and QA branch only after resolving their absolute paths, confirming they are under `D:\.worktree`, and confirming no needed uncommitted artifacts remain.

If QA fails, fix and commit the issue-branch changes with a fix agent, rerun focused checks, obtain an independent review of the updated complete diff, update from current `origin/main`, and repeat isolated QA. Do not advance the queue until QA passes or a genuine blocker needs user input.

## Publish, watch, and merge

After QA passes:

1. Confirm the issue branch is clean, its final diff is intentional, and required local checks pass.
2. Push the issue branch and create a ready-for-review PR targeting `main`. Include:
   - `Closes #<issue-id>` and the issue link;
   - acceptance criteria;
   - implementation summary;
   - review and QA evidence with exact commands;
   - performance results when relevant;
   - compatibility notes, decisions, and residual risks.
3. Watch all required GitHub checks and review requirements until they reach a terminal state. Under the current Main CI workflow, require successful Ubuntu `linux-x64`, Windows `windows-x64`, macOS `macos-arm64`, and unconditional `PR Gate` results. Re-read the workflow in case these names change. Use bounded waits or polling so progress can be reported at least once per minute. Never treat pending checks as success.
4. If a check fails:
   - inspect its logs and determine whether it is caused by the branch;
   - for branch failures, fix on the issue branch, review the new diff, repeat isolated QA, push, and watch the new check run;
   - for confirmed infrastructure or flaky failures, use the repository's supported rerun path and keep watching;
   - ask the user only for a genuine permission, policy, external-service, or unresolved product blocker.
5. Do not weaken tests, branch protection, or required checks to make the PR pass.
6. Once checks and required reviews are green, update from `origin/main` again if GitHub reports the branch out of date. Repeat review when the effective diff changes and repeat QA before merging.
7. Use the repository's required merge method. If GitHub requires its merge queue, enqueue the PR and monitor it through merge; otherwise merge the ready PR directly.
8. Verify the PR actually merged into `main` and the linked issue is closed. If the closing keyword did not close it, close the issue explicitly with a reference to the merged PR.
9. Fetch the updated `origin/main`, release newly unblocked implementation work, and promote the next reviewed issue under the ready-queue policy.

After a successful merge, remove the issue worktree and local issue branch only when both are clean, the absolute worktree path is confirmed under `D:\.worktree`, and the branch is confirmed merged. Preserve the remote branch according to repository policy.

## Report

Keep the user informed about dependency waves, active issue trees, queue position, review or QA findings, PR/check status, and blockers. At completion, report each issue's PR, merge commit, closure state, validation performed, and any intentionally retained worktrees or branches.
