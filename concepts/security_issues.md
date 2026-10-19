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
