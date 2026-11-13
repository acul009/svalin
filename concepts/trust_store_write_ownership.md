> **Notice**
>
> This document was written with Codex.

# Trust Store Write Ownership

**Status:** Open; architecture improvement requested by the project owner.

The shared `Arc<RwLock<TrustStore>>` lets multiple tasks acquire write access. The lock serializes those acquisitions, but the API does not enforce a single designated writer task.

Introduce a wrapper that exposes cloneable read handles and a single, non-cloneable writer handle, analogous to an MPSC channel's single receiver. Keep the underlying lock private so read handles cannot acquire write access. Move the writer handle into the task responsible for trust-store mutations; other tasks should submit update requests to that owner when needed.

This ownership constraint is separate from [SEC-010](security_issues.md#sec-010-possible-trust-store-writer-starvation-under-sustained-reads): a single writer handle alone does not prevent readers from starving that writer.
