# Faraweave per-issue decision records

Material design and implementation decisions are recorded here in one append-only Markdown file per GitHub issue. This layout replaces the shared diary tail so independent issue branches do not edit the same file.

## Ownership and naming

- Each GitHub issue owns at most one decision file.
- Create the file when the issue first makes a material decision. Use lowercase ASCII `issue-<number>-<stable-kebab-slug>.md`.
- The GitHub issue number is authoritative. Select the kebab-case slug when the file is created and never rename it to follow later issue-title edits.
- For example, Issue #65 owns `issue-65-per-issue-decision-records.md`. A hypothetical Issue #123 uses `issue-123-<stable-kebab-slug>.md`, with the slug selected from that issue when its record is created.
- Copy [`TEMPLATE.md`](TEMPLATE.md) for the initial file. Keep the issue heading, issue URL, and active append-only status at the top.

Material decisions include syntax and semantics, data layout, ownership, dependencies, error representation, parsing or execution strategy, testing and benchmark policy, and selection or rejection of credible alternatives. Routine keystrokes and mechanically implied edits do not require a decision section.

## Append-only rules

- Add each material decision as a new dated `##` section in the issue's existing file.
- Multiple decisions for the same issue append to that one file; never create a second record for the issue.
- After a record is created, do not rewrite, reorder, remove, or reflow its existing text. Its path and slug are also stable.
- Correct an earlier decision by appending a new section to the same issue file. Identify the earlier file and heading in `Supersedes`.
- When one issue supersedes a decision owned by another issue, append the new section only to the later issue's file and identify the earlier file and heading in `Supersedes`. Do not edit the superseded issue's file merely to add a backlink.
- Use `Supersedes: None` when the decision does not replace an earlier recorded decision.

Reviewers verify append-only history by comparing the issue record with its merge-base version. Existing bytes must remain an exact prefix; only new decision sections may be appended.

## Discovery

Find a record by its authoritative issue number and filename, for example with `git ls-files 'doc/decisions/issue-65-*.md'`. Repository filename search and Git history provide cross-issue discovery.

Do not add a tracked central index, generated catalog, or shared issue list. Such a file would recreate the write hotspot this directory removes. This README defines policy; it is not an index of issue records.

## Protected shared files

Routine issue work edits only its own `issue-<number>-<stable-kebab-slug>.md` record. It must not edit:

- this policy README;
- [`TEMPLATE.md`](TEMPLATE.md);
- the old-path compatibility pointer at [`doc/decision-diary.md`](../decision-diary.md);
- [`legacy-decision-diary.md`](legacy-decision-diary.md); or
- another issue's decision record.

A dedicated workflow-policy or migration issue may deliberately change a shared policy file when that change is its stated scope.

## Historical diary

[`legacy-decision-diary.md`](legacy-decision-diary.md) is the byte-for-byte historical diary from the implementation base of Issue #65. It contains all decisions recorded through accepted Issue #42 work. It is archival and must never be split, normalized, reformatted, or appended.

The old path, [`doc/decision-diary.md`](../decision-diary.md), is only a compatibility pointer to this policy and the preserved legacy file. No new decision belongs there.
