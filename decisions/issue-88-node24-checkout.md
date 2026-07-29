# Issue 88: Node 24 checkout action

Use `actions/checkout` v7.0.1 because its official action metadata selects Node 24. Pin its verified commit SHA in every active workflow, and make the offline workflow contract reject both floating refs and any other checkout revision. This keeps the runtime upgrade consistent without weakening the existing immutable-action policy.
