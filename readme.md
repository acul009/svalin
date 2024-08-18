# Svalin

## ⚠️WARNING⚠️

**This repository is not yet ready for production use.**

A lot of this code is still being heavily worked and, is missing security checks and has not been reviewed.

The current focus is getting this behemoth of a project working in any state.

One the programm is usable, it's going to be time to slowly but surely improve everything.

Feel free to browse the codebase and create issues for all problems you see.

## What is this ?

This is my Repository for developing a production ready Svalin.

### Purpose

Svalin is suppoed to be the first open-source end-to-end encrypted remote managment software with a focus on small businesses, individuals and goverment agencies.
This codebase is still far from where I want it to be, but I have to start somewhere.

Svalin has the following guidelines:
- Be easy to setup and use
- Be secure by default
- Allow self-hosting

### Why I'm building Svalin

Before I continue, let me say this: If you're happy with your solution, you don't have to switch.
By it's design Svalin will not be for everyone and if you prefer something else, that's ok.
Nontheless, if you want to try it out, I'd be happy to hear about your experience and what you think is still lacking.

I started Svalin because I was fed up with Teamviewer and because RPort stopped being open source.

I've always been taught and told, that I shouldn't blindly trust a product because I pay for it.
Whereever I can I try to switch to open source projects which allow me to self host my services
and which don't just come with a "Trust be Bro" guarantee.

Teamviewer pros:
- Easy to use
- quick to get started
- once you get your devices registered it's quite reliable

Teamviewer cons:
- It's expensive
- The "RMM" features are not great
- The new group system and client keep acting up
- works on linux, but not particularily great
- no way to just access a remote webinterface directly (at least not easily)

RPort pros:
- great linux support
- nice dashboard informations for connected devices
- easy to connect to devices with webinterface

RPort cons:
- Once the server is compromised, all devices are
- No builtin remote desktop


I started thinking about what I would like my perfect remote software to look like.
- Easy to use, if possible, should not require any terminal knowledge
- End to end encryption that the server cannot break
- Can pierce through NAT, no special configuration on controlled nodes needed
- Can remote control both desktop and webinterfaces
- Self-hostable, but could also be offered as a hosted solution (not everyone might want to self host)
- Focused on small businesses, not big tech
- Open source (obviously)

Obviously I'm far from reaching all of these, but that won't stop me from trying.

### Why not Rustdesk?

Rustdesk follows a different approach and Idea. They much more closely resemble the Teamviewer experience.

Svalin on the other hand is more inspired by RPort. Svalin isn't made for big operations, but rather for small businesses and individuals.
Because of this, svalin was developed with the idea of a central server per organisation.

But even RPort is quite a bit different. Svalin's core isn't actually a remote control software,
but a system to connect different nodes and allow them to communicate with each other.
The remote managment software is just built on top of that.

Thanks to this simple approach, Svalin could be extended for all kind of applications.
One example would be to use Svalin like a Tailscale coordinator and configure Wireguard Mesh networks with just a few clicks.

While the svalin RPC-System is it's own crate, it's still missing a higher level API to build your own network.
If you're interested in that, I'd be happy to hear about it.

If you're based in the EU, you might also be happy to hear, that Svalin is a german project.

TLDR: Rustdesk is primarily a remote software while Svalin is more similar to a generic coordinator. Use whatever fits your use case best.

## Quickstart

WIP - this isn't really complete yet.

### Installing the server

The server does not yet have an official installation method.

You can start the server with the following command: `svalin server 0.0.0.0:<PORT>`
Make sure the client and agents can reach the udp port you specified.

### Setting up the client

The client does not yet have an official installation method.

When starting the client without any existing profiles, you will be asked to create one.
enter the server address with port (e.g. `svalin.example.com:1234`) and follow the stept to create your root user.

**MAKE SURE YOU WRITE DOWN YOUR ROOT PASSWORD - YOU CANNOT RECOVER IT**

On each new start you can select a profile to use or you can create a new one.

**To open a profile you need the corresponding password to decrypt your certificate**

### Setting up the agent

The agent does not yet have an official installation method.

When first using the client, start it with `svalin agent init <HOST:PORT>`.

The agent will print out a join code. Add a new device in the client and enter the join code when asked.

After receiving the join code, the client will establish an encrypted connection to the agent. The agent should now print out it's confirm code.
Enter the confirm code along with the device name in the client and click confirm to add the new device.

## Contributing

At the moment there is not an official way to contribute yet.
If you want to help you can create an issue so we can coordinate our efforts.

## Extending

Currently Svalin isn't really ready to be extended.
If you have a project or some functionality in mind, please create an issue describing what you would like to do.
Then we can work on exposing the svalin API to fit your needs.

# Architecture

## crate svalin

This crate is the actual Svalin library used by the GUI to control the server and devices.

It also builds to the rust standalone which can run as either server or agent.

If you want to build your own UI, you should find everything you need here.
If you don't - just create an issue.

## crate svalin_rpc

This crate contains the inner workings of the RPC system used to send data between devices

Svalins RPC-API is based on these basic primitives.

### The Connection primitive

A connection is a way to communicate with another node.
It allows you to open sessions, which in turn are used to send and receive data, most of the time in form of the very basic RPC-Commands.

A connection could be a direct QUIC connection to the server,
the ability to tunnel through the svalin main server or it could wrap a direct connection to another node.

As a connection is just a trait, svalin can easily be extended to tunnel through other protocols or build direct P2P connections.

### The Session primitive

A session is an open communication channel to another node. A session is usually bound to a context.
So you'd use one session to monitor running processes and another one to connect to a terminal session.

The session already provides the utility to write and read data types which implement Serde's Serialize and Deserialize traits.

Underneath the surface, the session uses a transport, which is just an asynchronous bi-directional byte stream.
This means that a session can run through basically every network connection or protocol.

That very same logic is also used to handle the E2E encryption. To establish an encrypted tunnel,
svalin just replaces the normal QUIC transport with a TLS-stream based on tokio-rustls.
The underlying RPC system doesn't know or care about the encryption.

## svalin_pki

This crate contains the code for certificate generation and encryption.

You'll also find the TBRHL here (Transaction Based Rolling Hash Ledger).
A very simple transaction store inspired by Blockchain traceability.
(Don't worry, it's not a real Blockchain)

## svalin_macros

You'll find svalins macros here

## svalin_sysctl

This crate contains code used by the agent for monitoring and managing a system.

# Todo

## Simple
- [ ] Show Unlock Error when password on profile unlock wrong
- [ ] Make Credentials use an Arc internally
- [ ] Fix as many warnings as possible

## Bigger
- [ ] Add translation to flutter app
- [ ] Add documentation for all public RPC types
- [ ] Add support for IPv6
- [ ] Rate Limiting
- [X] Init Routine
- [ ] Login Routine (make sure the server sent hashing parameters are strong enough)
- [X] Unlock Routine
- [ ] Make Credentials use an Arc internally
- [X] Agent init / join
- [X] Connection forwarding
- [X] E2E Encryption
- [X] List all agents with connection status
- [ ] Show realtime performance ingo
- [ ] Remote Terminal

## Architecture
- [X] crate for local system managment and package management (WIP)
- [ ] Basic Permission System
- [ ] Network Security System and Alerting
- [ ] Replace certificate distribution with better system (possibly TBRHL)
- [ ] Think of way to nicely and securely distribute group and system state
- [ ] Information Subscription System (e.g. CPU utilization, connected clients)

## Milestones
- [ ] Basic Managment (Live)
- [ ] Remote Desktop
- [ ] Network Security System and Alerting
- [ ] IPv6 Support
- [ ] 

## Svalin Network Architecture

1. QUIC with Network Authentication and Path encryption
2. Routing through the Svalin Network
3. P2P Encryption
