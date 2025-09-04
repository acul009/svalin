
# Primitives for Svalin

This is just supposed to be a glossary of svalins most important building blocks.
If you want to learn about svalins internals, this is the place to start.

## Certificates

This isn't really part of svalin per se - when talking about certificates I just mean normal X509 certificates.
As of writing this I'm basically just using ed25519. That might change in the future.
Svalin uses the SPKI hash (basically just a hash of the public key) as the subject.

Every time a certificate is parsed, svalin will execute some checks to ensure the certificate follows the convenstions.

## Credential

A credential is just the combination of a certificate and it's private key.
It's used to identify and authenticate an entity

## Entity

An entity in svalin is just one of the following

- the root user
- a user
- a users session
- a server (currently just one)
- an agent

## Users and the Root

The root user is just the root certificate of the whole certificate structure.
The root has special priveliges and can circumvent all permission checks and security measures.

That's quite risky, but it's there as a last resort to restore access
to a controlled device or repair a damaged system in case it's somehow bugged or corrupted.

Users are just a credential that's a CA.
They have a password and username to retrieve their credential via a classic login system.
That's not technically neccesary, but just makes svalin way easier to use for normal people.

In theory the login via password could be skipped for one or all users.
I'll only investigate that if there are actually requests for that though.

## The server

The server (as of writing just one) is the central hub for communication and storage.
It does support a lot of security systems, but doesn't have any authority to actually meaningfully interact with agents.

## Agents

The agent is just the software that runs on a device you want to control.
Usually it's just going to be a system service, though there's nothing stopping me from building a quick support I guess.
