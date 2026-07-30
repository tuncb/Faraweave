# Make GitHub issue orchestration token-efficient

The issue workflow uses fresh minimal-context agents, a controller-owned slot ledger, and state-driven 60-second waits instead of cross-issue reuse and frequent polling. Implementers and reviewers run focused evidence while one queue-head QA agent owns the complete local matrix for each exact commit. Role-specific references and compact external evidence keep the controller context small without weakening merge or closure guarantees.
