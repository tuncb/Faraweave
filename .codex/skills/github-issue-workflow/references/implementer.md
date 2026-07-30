# Implementer role

Use this role for the issue lead, an implementation child, or a fix agent. Work only in the assigned issue worktree.

## Before editing

1. Read the issue packet, relevant specifications, existing issue decision, production modules, tests, fixtures, traceability rows, and applicable validation-ladder section.
2. Trace affected behavior through the Rust API/evaluator, CLI, emitted C, native executable, specifications, and packaging where applicable. State intentional exclusions.
3. Run the narrowest existing regression on the untouched branch and record baseline failures.
4. Map every acceptance criterion to a focused success, boundary, or failure test, including exact diagnostics, spans, ordering, cleanup, or resource behavior.

## Implement

1. Make only issue-scoped production, test, fixture, specification, traceability, and decision changes.
2. Preserve parse → resolution → analysis → execution separation.
3. Use checked sizing and `try_reserve` through existing allocation/resource seams. Preserve admission, charging, release, failure precedence, and transactional publication.
4. Preserve binary64 behavior, structured errors, byte spans, formatting, paths, aliases, cleanup, and transactional contracts.
5. Add a regression that fails at the merge base and passes at the tip. Preserve stable contract identifiers when extending existing evidence.
6. Update applicable specs, porting manifest, README, fixtures, and traceability only when their contract or evidence mapping changes.

## Validate and hand off

1. Run `cargo fmt --all`.
2. Run the focused tests mapped to the acceptance criteria.
3. Run `python tools/validation/contracts.py focused` for specification, traceability, workflow, documentation, packaging, or release changes.
4. Run one complete debug suite only when the change crosses modules and focused evidence cannot prove it.
5. Do not run the complete release/contracts/package matrix; queue-head QA owns it. Run a release-specific command only when the issue changes that surface or a concrete failure requires diagnosis.
6. Inspect `git diff --check`, the complete merge-base diff, and `git status --short`.
7. Commit intentional changes and leave the worktree clean.
8. Return the compact handoff from `SKILL.md`, including changed files, exact focused commands, decisions, risks, and the temporary evidence-log path.

A fix agent receives only the blocking findings, updated tip, affected files, and required focused tests. It must not revisit unrelated design or validation.
