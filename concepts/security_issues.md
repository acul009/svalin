> **Notice**
>
> This document was written with Codex.

# Known Security Issues

This document tracks confirmed security gaps and intended fixes. It is not a complete security audit.

## SEC-001: Session registration and revocation are not enforced

**Status:** Open

**Affected code:** [`TrustStoreVerifier::verify_known_certificate`](../svalin_pki/src/verifier/trust_store.rs), used for incoming client authentication in [`Server::start`](../svalin/src/server.rs).

For a `UserSession` certificate, the verifier checks that its issuer is trusted and valid, then verifies the session certificate's signature and validity. It does not check whether the session is registered in the server's session store or has been revoked.

Consequently, removing a session from the database would not prevent it from reconnecting. A holder of a user's signing credential can also create a session certificate that authenticates without registering through the login flow. Possession of a session certificate alone is insufficient: authentication still requires its private key.

This affects commands available to authenticated sessions, including listing the issuing user's sessions. The list-sessions command derives ownership from the authenticated certificate's issuer; this gap does not by itself allow a caller to select another user's sessions.

### Intended fix

Distribute session revocations through the trust store and enforce them wherever session certificates are accepted. The server must additionally require each session to be registered in its session store before allowing it to authenticate.

## SEC-002: Independent fork and freeze detection is missing

**Status:** Open

**Affected code:** [`UpdateTrustStore`](../svalin/src/shared/commands/update_trust_store.rs), which compares the local trust-store digest with the server's digest but not with independent peers.

A malicious or malfunctioning server can withhold updates or present different histories to different members. Comparing only with that server cannot reliably detect these forks or freezes.

### Intended fix

Run a guardian background task in both agents and clients. It maintains and rotates connections to other clients and compares checksums, including trust-store checksums, to detect inconsistent views independently of the server. Detection requires reaching a peer with a different view; agreement among equally stale peers does not establish freshness.

Future versions might add a gossip protocol to exchange data around a malfunctioning server. This is a possible extension, not part of the committed guardian design.

## SEC-003: Trust-store revocation support is missing

**Status:** Open

**Affected code:** [`trust_store::Transaction`](../svalin_pki/src/trust_store.rs) currently supports only adding certificates and removing expired certificates.

Compromised identities cannot be revoked through the trust store while their certificates remain valid. This affects users, agents and other certificate types, beyond the session-specific enforcement gap in SEC-001.

### Intended fix

Add revocation transactions to the trust store and enforce the resulting revocation state during certificate verification.

## SEC-004: Possible local update-file substitution

**Status:** Open; exploit not yet reproduced.

**Affected code:** [`Location::system_temp_dir` and directory creation](../svalin/src/util/location.rs), [agent temporary-directory handling](../svalin/src/agent.rs), and [update preparation and execution](../svalin/src/installer/agent/update.rs).

The agent prepares an executable at a predictable path beneath `std::env::temp_dir()/svalin/svalin-agent`, then executes it by pathname. Existing temporary parent directories are accepted without ownership or permission checks.

A local unprivileged actor who controls the temporary parent directory may be able to replace the prepared executable before a legitimate administrator-triggered update runs. The substituted executable would run with the agent's privileges, normally root for the generated Linux service. This requires local filesystem access and a legitimate update; it is not an unauthenticated network attack.

### Intended fix

To be determined.

## SEC-005: Device and device-meta group memberships can diverge

**Status:** Open; known issue reported by the project owner.

The MLS device group and its corresponding device-meta group can have different member sets. Membership changes can leave access to device data and metadata inconsistent between the two groups.

### Intended fix

To be determined.

## SEC-006: Returning devices can be kept on stale trust state

**Status:** Open; known issue reported by the project owner.

A device returning after being offline can be denied trust-store updates by a malicious server. Withholding changes, including revocations, can leave the device trusting old or compromised certificates that should no longer grant access. Certificate expiry still applies; this does not extend a certificate's validity period.

This is a specific freeze scenario related to SEC-002: the returning device needs an independent way to establish whether its trust state is current.

### Intended fix

The solution will include the guardian task described in SEC-002. The exact implementation is to be determined.

## SEC-007: Live trust synchronization can stop without withdrawing access

**Status:** Open; confirmed by source inspection.

**Affected code:** [`update_trust_store`](../svalin/src/util/trust_store.rs) and [`UpdateTrustStore`](../svalin/src/shared/commands/update_trust_store.rs).

After initial synchronization, a live update error is logged and the background task exits. The client or agent continues operating with its existing trust state, without requiring synchronization to recover. Later trust changes would no longer be received through that task.

### Intended fix

To be determined.

## SEC-008: Established connections are not revalidated for each command

**Status:** Open; confirmed by source inspection.

**Affected code:** [RPC connection session handling](../svalin_rpc/src/rpc/connection/mod.rs) and [command permission checks](../svalin_rpc/src/rpc/command/handler.rs).

New RPC streams reuse the connection's authenticated peer identity. Command permission checks do not revalidate certificate expiry or current trust state. An established connection can therefore retain access after its certificate expires. Implementing revocation at authentication alone would likewise leave already established connections authorized.

### Intended fix

To be determined.

## SEC-009: Certificate verification does not consistently validate ancestors

**Status:** Open; confirmed by source inspection.

**Affected code:** [`TrustStoreVerifier::verify_spki_hash`](../svalin_pki/src/verifier/trust_store.rs) checks the requested certificate's validity without walking its issuer chain. Session verification checks the immediate issuer, but not further ancestors.

A certificate can remain accepted after an ancestor expires. Revocation must also invalidate descendants rather than leaving their certificates independently trusted.

### Intended fix

Replace temporary apply-and-rollback validation in the secure chain with validation against a cloned state. Apply the transaction to the clone and verify its resulting state hash; discard it on failure. This removes the need for transaction-specific rollback information.

When revoking a certificate, also remove all of its descendants from the trust store. Ancestor expiry must likewise invalidate descendants during verification; revocation-driven removal alone does not address expiry.

## SEC-010: Possible trust-store writer starvation under sustained reads

**Status:** Open; theoretical risk reported by the project owner; starvation has not been reproduced.

**Affected code:** [`TrustStoreVerifier`](../svalin_pki/src/verifier/trust_store.rs) acquires read access to the shared `std::sync::RwLock<TrustStore>`, while [`apply_block`](../svalin/src/shared/commands/update_trust_store.rs) requires write access to validate and apply trust-store updates.

Rust's [`std::sync::RwLock` documentation](https://doc.rust-lang.org/std/sync/struct.RwLock.html) does not guarantee a reader/writer priority policy; the policy depends on the underlying platform implementation. With a reader-preferring implementation, such as a reader-preferring POSIX lock, continuous overlapping read acquisitions could starve a waiting writer. This is platform-dependent, not a confirmed property of every Rust or POSIX implementation.

If this occurs, trust-store updates could be delayed indefinitely while readers continue using the existing trust state. This could delay future revocation enforcement once SEC-003 is implemented. Practical exploitability and the required reader load have not been established.

### Intended fix

Use a synchronization strategy with documented protection against writer starvation, or restructure trust-store access so readers cannot indefinitely prevent updates. Select the implementation and verify writer progress under sustained concurrent reads on supported platforms.

## SEC-011: Secrets are distributed too widely in memory

**Status:** Open; known issue reported by the project owner.

Secret values are copied and passed around too widely in memory, increasing the number of places that hold sensitive material and making its lifetime harder to control.

### Intended fix

Use `BoxedSecret` and `BoxedSlice` for secret storage and handling to reduce unnecessary copies and keep secret ownership and lifetimes explicit.

## SEC-012: Trust-store state digest lacks an explicit multihash construction

**Status:** Open; construction gap confirmed by source inspection; exploit not demonstrated.

**Affected code:** [`State::digest`](../svalin_pki/src/trust_store.rs), consumed by [`Chain::digest`](../svalin_pki/src/secure_chain.rs).

The trust-store state digest feeds the root certificate's DER bytes followed by the stored certificates' DER bytes, sorted by SPKI hash, directly into one hash context. It does not use a structured multi-input hashing construction with explicit domain separation, field roles, and sequence framing. Separate hash-update calls do not encode boundaries between inputs.

The current certificates are DER-encoded, which supplies internal length information, so missing explicit framing alone does not establish that two valid trust-store states can produce the same hash input. Nevertheless, the construction relies on those encoding constraints rather than an explicit multi-input hashing contract, making future changes harder to assess safely. This concerns hashing multiple values, not algorithm-identifying multihash formats.

### Intended fix

Use a reviewed multi-input hashing construction, such as SequenceHash with SHA-2, and explicitly distinguish the trust-store state domain, root certificate, and ordered certificate sequence. Preserve deterministic ordering and add tests for field and sequence boundaries. Changing this construction changes trust-store digests and requires a compatibility strategy for existing peers.
