Ok, for the permission store I'd actually like all agents to take part in verification.

The problem is, that I'd expose who has which permission.

But I might still be able to make it work, by anonymizing the identifiers so agents don't know which member a permission belongs to.
If they want to know what permissions a member has, they'd need to be able to generate the id from the members certificate or public id.

Since assinging permissions is usually done by someone with higher permissions of the same kind,
it might also be possible to use a different id for each granted permission.
In theory, this would work by introducing a salt before hashing the certificate and when requesting to do something,
the requesting member could send the salt with it, allowing the permission verifying party to find the assignment in the permission store.

But in that case, how would I sign the assignment?
I can't request the public key of the member who assigned the permission because then I'd just deanonymize them.

The certificate verification store could maybe be split into two parts:
- The server stores all certificates and allows requesting a certificate chain (either via full cert, fingerprint or spki_hash)
- All agents store a log of additions and revocations using an anonymized id (e.g. the fingerprint) but not the certificate itself.
  Obviously, the id or data to request the certificate chain cannot be the same used to request the chain from the server.
  It should also be a different one than the one used to assing permissions
