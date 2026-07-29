> **Notice**
>
> This document has been written with ChatGPT (GPT-5.5).

# Central Trust Store and Transaction Chain

## Purpose

The system maintains a shared trust state for certificates and revocations without giving the central server authority over that state.

The server provides storage, ordering, synchronization and communication between members. Trust decisions are derived from signed transactions and verified locally by every member.

Security is preferred over availability. Missing, conflicting or unverifiable data should therefore cause the affected operation to fail closed.

## Identity and trust

Every member has a public/private key pair and a certificate. The member is identified by the hash of its Subject Public Key Info (SPKI), rather than by a conventional certificate name.

Users may act as certificate authorities. Other members, such as servers or controlled computers, do not.

A certificate is not trusted merely because it was signed. Its SPKI hash must also be present in the shared trust state. Revocations are stored against the same SPKI hash and therefore affect every certificate using that key.

Initialization is the trust bootstrap. A trusted user provisions a new member with the known root, its certificate, the current trust state and the latest chain position. The exchange is end-to-end encrypted and authenticated through the local initialization code and AuCPace.

## Transaction chain

Changes to the trust state are represented by signed transactions. Transactions form an append-only hash chain.

Each transaction contains at least:

- protocol version
- sequence number
- timestamp
- hash of the previous transaction
- hash of the resulting trust state
- signer SPKI hash
- generic transaction content
- signature

The sequence number is redundant for integrity, but useful for indexing, synchronization and diagnostics. It must increase by exactly one and be covered by the signature.

The previous transaction hash commits the transaction to one specific history. The next transaction should reference the hash of the complete accepted transaction, including its signature. This prevents an alternative signature representation from being substituted later without changing the chain.

The resulting state hash commits to the state after applying the transaction. State serialization must therefore be canonical and protocol-defined.

## Transaction processing

The state implementation exposes three operations:

- `check` verifies whether the signer may perform the transaction at the supplied timestamp.
- `apply` mutates the state and returns an opaque rollback value.
- `rollback` restores the exact previous state.

The chain processes a transaction as follows:

1. Verify its structure, signature, sequence number and previous transaction hash.
2. Call `check` with the transaction content, signer SPKI hash and timestamp.
3. Call `apply` and retain the rollback value.
4. Calculate the resulting state hash.
5. Compare it with the hash committed to by the transaction.
6. Roll back on mismatch; otherwise accept the transaction.

`apply` must either return a complete rollback value or leave the state unchanged. `rollback` should be infallible and restore a hash-equivalent state.

## Chain state

The core `Chain` does not need to retain the full history. It only needs:

- the current trust state
- the current sequence number
- the last accepted transaction hash

Historical transactions may be stored separately for synchronization, auditing and fork investigation. Nodes may keep the complete history, only recent transactions, or prune everything before a trusted checkpoint.

A newly initialized node only needs a trusted snapshot of the current state and the latest chain position. Older transactions are optional.

## Comparison and fork detection

Two members can compare their current chain position using:

- sequence number
- last transaction hash
- resulting state hash

Matching sequence numbers and transaction hashes imply matching accepted histories. A different state hash despite an identical history indicates an implementation or canonical-serialization error.

A malicious server may maintain separate but internally valid histories. Such a split view becomes detectable once members from different views compare their chain positions.

Persistent local state prevents unnoticed rollback. Sequence gaps, conflicting transactions at the same position and invalid chain links are treated as failures.

## Merkle tree

A Merkle tree is not required for the basic transaction chain. The previous transaction hash already provides append-only integrity and transitively commits to the complete preceding history.

A Merkle tree may later be added as a derived index over transaction hashes. Its main benefit is efficient comparison, inclusion proofs and locating the first difference between two histories.

The Merkle root should supplement rather than replace the last transaction hash. The core chain should remain independently verifiable without the Merkle index.

## Server authority

The server may serialize submissions and reject transaction collisions, but it cannot create valid trust changes without an authorized signature.

It can still omit, delay, freeze or selectively present transactions. Local persistence and peer comparison make these attacks detectable once an independent view becomes reachable.

Witness or guardian nodes may later strengthen this model, but they are optional. The base system must remain functional without them.

## Open design questions

The following rules still need to be defined precisely:

- which users may issue or revoke which certificates
- whether and how revoked identities may ever be reinstated
- exact transaction content types
- canonical encoding and domain separation
- snapshot and checkpoint formats
- retention and pruning policy
- optional witness or guardian semantics
