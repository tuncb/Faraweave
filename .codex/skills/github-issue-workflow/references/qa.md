# Queue-head QA role

Own the sole complete local validation matrix for the exact queue-head commit. Do not implement or review the change.

## Prepare isolated QA

1. Fetch current `origin/main`.
2. Create a uniquely named QA branch and temporary worktree from that exact commit.
3. Merge the issue branch into the QA worktree and record base, issue tip, QA merge, parents, and tree.
4. Confirm the merge contains the reviewed commits and the worktree is clean.
5. Inspect the complete diff, lockfile, fixtures, specifications, traceability, decision record, and tracked-artifact absence.
6. Record OS, architecture, Rust 1.97.1, Python, external tools, and host limitations.

## Exercise acceptance criteria

Classify every criterion as app-executable or lower-level-only.

For each app-executable criterion, run the exact QA merge's built application through its user-facing CLI, executable, or generated native artifact. Exercise the criterion with realistic inputs and verify exact stdout, stderr, exit code, files, diagnostics, spans, cleanup, resource behavior, and performance thresholds. Automated tests or direct library calls do not replace this application journey.

Use public Rust API or test-only evidence only when the application has no surface for the criterion, and record why.

## Run the complete local matrix once

Run these gates in order for the exact QA merge:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets --all-features -- -D warnings`
3. `cargo build --workspace --all-targets --all-features --release`
4. `cargo test --workspace --all-targets --all-features`
5. `cargo test --workspace --all-targets --all-features --release`
6. `python tools/validation/contracts.py full`
7. The host package contract:
   - Windows x64: `python tools/validation/contracts.py package windows-x64`
   - Linux x64: `python tools/validation/contracts.py package linux-x64`
   - macOS arm64: `python tools/validation/contracts.py package macos-arm64`

Confirm the subchecks that `contracts.py full` actually executed; never infer them from its name. On Linux, confirm sanitizer and `/dev/full` paths where applicable. On Windows, confirm PE identity and long-path checks. On macOS arm64, confirm native execution and archive checks. Assign other platforms to GitHub CI.

Do not rerun this matrix for the same QA merge. Reuse its exact evidence unless the commit, merge tree, environment validity, or concrete failure changes.

## Classify failures

Preserve full output and reproduce a required failure once. If it appears unrelated, run the same command in a separate clean worktree at the recorded `origin/main`. Classify it as regression, baseline failure, missing prerequisite, or infrastructure/flaky failure.

Never modify the issue branch. Return a failure to the controller for an issue-scoped fix agent, fresh review, and new QA merge.

## Finish

Return the compact handoff from `SKILL.md` with exact identities, gate exits, application-journey commands and results, lower-level-only justifications, host exclusions, and evidence path. QA passes only when every applicable criterion and gate passes.

Remove the temporary QA worktree and branch only after resolving their absolute paths and confirming no needed uncommitted evidence remains.
